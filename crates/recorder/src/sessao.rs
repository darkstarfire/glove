//! Sessão de gravação: sobe o `ffmpeg`, alimenta o `stdin` numa thread própria e
//! encerra com timeout. `iniciar` e `parar` bloqueiam (esperam processos), então
//! devem rodar fora do loop de renderização. `enviar` e `viva` nunca bloqueiam.
//!
//! Queda do ffmpeg no meio da gravação: o arquivo continua utilizável (mp4
//! fragmentado ou mkv), `parar` sempre devolve o caminho e o motivo, e o chamador
//! decide se avisa, se retoma por software etc.

use crate::{build_config, detect_encoders, find_render_node, EncoderConfig, VideoBackend};
use glove_config::tr;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Frames na fila. Quando cheia, `enviar` descarta o frame novo.
const FILA_FRAMES: usize = 4;
/// Sem frame novo por esse tempo, o último é reenviado (mantém A/V sincronizados).
const REENVIO: Duration = Duration::from_millis(100);
/// Tempo máximo para o ffmpeg finalizar ao parar (ou ao ser descartado).
const TIMEOUT_PARAR: Duration = Duration::from_secs(15);
/// Se o ffmpeg morrer nesse intervalo após iniciar, conta como falha de inicialização.
const JANELA_INICIO: Duration = Duration::from_millis(500);

/// O que o usuário pediu para gravar.
#[derive(Debug, Clone)]
pub struct RecordingRequest {
    /// Tamanho do quadro em pixels (a região ou janela selecionada).
    pub largura: u32,
    pub altura: u32,
    pub fps: u32,
    pub pasta_saida: PathBuf,
    /// Áudio do sistema (monitor da saída padrão).
    pub audio_sistema: bool,
    /// Microfone (fonte padrão).
    pub microfone: bool,
}

/// Gravação em andamento: o processo `ffmpeg` e a thread que alimenta seu `stdin`.
pub struct ActiveRecording {
    child: Option<Child>,
    frames: Option<SyncSender<Vec<u8>>>,
    /// Status do ffmpeg, depois de observado (ver `observar`).
    saida: Option<ExitStatus>,
    pub caminho: PathBuf,
    pub config: EncoderConfig,
}

/// Status (bruto, como do `waitpid`) dos processos que o compositor recolheu. Quando o
/// compositor faz `waitpid(-1)`, ele tira o ffmpeg antes da gravação ler o status; por
/// isso ele registra aqui, e a gravação lê daqui antes de perguntar ao sistema.
static SAIDAS: Mutex<Vec<(u32, i32)>> = Mutex::new(Vec::new());

/// Chamado pelo compositor ao recolher um filho (`waitpid`) com o status dele.
pub fn registrar_saida(pid: u32, status: i32) {
    if let Ok(mut v) = SAIDAS.lock() {
        v.push((pid, status));
    }
}

/// Status de um filho: primeiro o que o compositor registrou, depois o do sistema.
fn status_de(child: &mut Child) -> std::io::Result<Option<ExitStatus>> {
    let pid = child.id();
    if let Ok(mut v) = SAIDAS.lock() {
        if let Some(i) = v.iter().position(|(p, _)| *p == pid) {
            let (_, st) = v.swap_remove(i);
            return Ok(Some(ExitStatus::from_raw(st)));
        }
    }
    child.try_wait()
}

/// Como a gravação terminou. `erro` é `Some` quando o ffmpeg caiu ou não finalizou
/// direito; `caminho` aponta para o que foi gravado até ali (pode estar parcial).
#[derive(Debug, Clone)]
pub struct Encerramento {
    pub caminho: PathBuf,
    pub erro: Option<String>,
}

/// Argumentos do `ffmpeg`: vídeo RGBA pelo `pipe:0`, áudio do PulseAudio/PipeWire
/// (uma entrada por fonte) e a codificação escolhida.
pub fn montar_args(cfg: &EncoderConfig, req: &RecordingRequest, saida: &Path) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    push_all(&mut a, &["-hide_banner", "-loglevel", "warning", "-y"]);
    a.extend(cfg.pre_input_args.iter().cloned());

    push_all(&mut a, &["-f", "rawvideo", "-pix_fmt", "rgba", "-s"]);
    a.push(format!("{}x{}", req.largura, req.altura));
    // Sem `-r` na entrada: com ele, o ffmpeg conta quadros a `fps` e ignora o tempo de
    // parede, e o vídeo fica curto quando a tela não chega a `fps` (era 10 s para 30 s).
    push_all(&mut a, &["-use_wallclock_as_timestamps", "1", "-i", "pipe:0"]);

    // Áudio só é gravado se há encoder de áudio. Sem ele, a gravação segue sem som.
    let mut fontes = 0;
    if cfg.audio_codec.is_some() {
        if req.audio_sistema {
            push_all(&mut a, &["-f", "pulse", "-use_wallclock_as_timestamps", "1", "-i", "@DEFAULT_MONITOR@"]);
            fontes += 1;
        }
        if req.microfone {
            push_all(&mut a, &["-f", "pulse", "-use_wallclock_as_timestamps", "1", "-i", "@DEFAULT_SOURCE@"]);
            fontes += 1;
        }
    }

    push_all(&mut a, &["-map", "0:v"]);
    match fontes {
        0 => {}
        1 => push_all(&mut a, &["-map", "1:a"]),
        _ => push_all(
            &mut a,
            &["-filter_complex", "[1:a][2:a]amix=inputs=2:duration=longest[a]", "-map", "[a]"],
        ),
    }

    a.push("-c:v".into());
    a.push(cfg.video_codec.into());
    a.extend(cfg.video_args.iter().cloned());
    if let Some(pf) = cfg.pixel_format {
        push_all(&mut a, &["-pix_fmt", pf]);
    }
    if let (Some(ac), true) = (cfg.audio_codec, fontes > 0) {
        push_all(&mut a, &["-c:a", ac, "-b:a", "160k"]);
    }
    if cfg.file_extension == "mp4" {
        // Fragmentado: cada trecho já escrito fica tocável mesmo se o ffmpeg morrer.
        // (Com `faststart`, um arquivo interrompido não teria o índice e não abriria.)
        push_all(&mut a, &["-movflags", "+frag_keyframe+empty_moov+default_base_moof"]);
    }
    a.push(saida.display().to_string());
    a
}

fn push_all(a: &mut Vec<String>, itens: &[&str]) {
    a.extend(itens.iter().map(|s| s.to_string()));
}

/// Inicia a gravação com o melhor encoder disponível. Se um encoder de hardware
/// falhar ao iniciar, registra o erro e tenta de novo com o melhor de software.
pub fn iniciar(req: &RecordingRequest) -> Result<ActiveRecording, String> {
    let encoders = detect_encoders()?;
    let cfg = escolhe_validado(encoders.clone(), find_render_node().as_deref())?;
    match tenta(&cfg, req) {
        Err(erro) if cfg.backend != VideoBackend::Software => {
            eprintln!("[recorder] encoder {} falhou: {erro}", cfg.video_codec);
            let sw = escolhe_validado(encoders.sem_hardware(), None)?;
            tenta(&sw, req)
        }
        resultado => resultado,
    }
}

/// Inicia direto com o melhor encoder de software. Usado para retomar a gravação
/// quando o encoder de hardware cai no meio.
pub fn iniciar_software(req: &RecordingRequest) -> Result<ActiveRecording, String> {
    let encoders = detect_encoders()?;
    let cfg = escolhe_validado(encoders.sem_hardware(), None)?;
    tenta(&cfg, req)
}

/// Escolhe o melhor encoder que de fato funciona nesta máquina. Cada candidato é
/// testado com uma codificação real; o que falha sai da lista e a escolha recomeça.
/// Estar listado pelo `ffmpeg -encoders` não quer dizer que a GPU consegue codificar.
fn escolhe_validado(mut encoders: crate::Encoders, render: Option<&Path>) -> Result<EncoderConfig, String> {
    loop {
        let cfg = build_config(&encoders, render)?;
        if sonda(&cfg) {
            return Ok(cfg);
        }
        eprintln!("[recorder] encoder {} não funciona nesta máquina, tentando outro", cfg.video_codec);
        encoders = encoders.sem_codec(cfg.video_codec);
    }
}

/// Codifica alguns quadros de verdade com a configuração. `true` se o encoder abre e
/// termina sem erro.
fn sonda(cfg: &EncoderConfig) -> bool {
    let mut a: Vec<String> = Vec::new();
    push_all(&mut a, &["-hide_banner", "-loglevel", "error", "-y"]);
    a.extend(cfg.pre_input_args.iter().cloned());
    push_all(&mut a, &["-f", "rawvideo", "-pix_fmt", "rgba", "-s", "320x240", "-r", "10", "-i", "/dev/zero", "-frames:v", "10", "-c:v"]);
    a.push(cfg.video_codec.into());
    a.extend(cfg.video_args.iter().cloned());
    if let Some(pf) = cfg.pixel_format {
        push_all(&mut a, &["-pix_fmt", pf]);
    }
    push_all(&mut a, &["-f", "null", "-"]);
    Command::new("ffmpeg")
        .args(&a)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn tenta(cfg: &EncoderConfig, req: &RecordingRequest) -> Result<ActiveRecording, String> {
    let caminho = caminho_saida(req, cfg.file_extension)?;
    inicia(cfg, req, caminho)
}

/// Pasta padrão das gravações: `Vídeos/Gravações de tela`, na pasta Vídeos do sistema
/// (a do user-dirs, como as capturas vão para a pasta Imagens).
pub fn pasta_padrao() -> PathBuf {
    glove_config::arquivos::screen_recordings_dir()
}

/// Componentes de data e hora locais: (ano, mês, dia, hora, minuto, segundo).
fn agora_local() -> (i32, u32, u32, u32, u32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as libc::time_t)
        .unwrap_or(0);
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&secs, &mut tm) };
    (
        tm.tm_year + 1900,
        tm.tm_mon as u32 + 1,
        tm.tm_mday as u32,
        tm.tm_hour as u32,
        tm.tm_min as u32,
        tm.tm_sec as u32,
    )
}

/// Caminho do arquivo: `<Gravação de tela>-aaaa-mm-dd hh-mm-ss.<ext>` na pasta pedida.
/// Se já existir um arquivo com esse nome (dois cliques no mesmo segundo), acrescenta ` (2)`, ` (3)`...
fn caminho_saida(req: &RecordingRequest, extensao: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(&req.pasta_saida)
        .map_err(|e| format!("{}: {e}", tr!("Não foi possível criar a pasta de gravações")))?;
    let (ano, mes, dia, h, m, s) = agora_local();
    let base = format!(
        "{}-{ano:04}-{mes:02}-{dia:02} {h:02}-{m:02}-{s:02}",
        tr!("Gravação de tela")
    );
    let mut caminho = req.pasta_saida.join(format!("{base}.{extensao}"));
    let mut n = 2;
    while caminho.exists() {
        caminho = req.pasta_saida.join(format!("{base} ({n}).{extensao}"));
        n += 1;
    }
    Ok(caminho)
}

/// Sobe o `ffmpeg` com a configuração dada e grava em `saida`.
fn inicia(cfg: &EncoderConfig, req: &RecordingRequest, saida: PathBuf) -> Result<ActiveRecording, String> {
    let args = montar_args(cfg, req, &saida);
    let mut child = Command::new("ffmpeg")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{}: {e}", tr!("Não foi possível iniciar o FFmpeg")))?;

    // O stderr precisa ser drenado, senão o ffmpeg trava quando o buffer enche.
    if let Some(err) = child.stderr.take() {
        thread::spawn(move || {
            for linha in BufReader::new(err).lines().map_while(Result::ok) {
                eprintln!("[ffmpeg] {linha}");
            }
        });
    }
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| tr!("O FFmpeg não abriu a entrada de vídeo.").to_string())?;

    let (tx, rx) = sync_channel::<Vec<u8>>(FILA_FRAMES);
    // Thread própria: a thread de renderização só faz `try_send`. A alimentação não
    // é esperada, então o handle é descartado.
    thread::spawn(move || alimenta(stdin, rx));

    let mut rec = ActiveRecording {
        child: Some(child),
        saida: None,
        frames: Some(tx),
        caminho: saida,
        config: cfg.clone(),
    };

    // Um encoder que não sobe (device ocupado, driver ausente) morre logo no início.
    thread::sleep(JANELA_INICIO);
    if let Some(child) = rec.child.as_mut() {
        if let Ok(Some(status)) = child.try_wait() {
            if !status.success() {
                return Err(tr!("O FFmpeg encerrou logo após iniciar.").to_string());
            }
        }
    }
    Ok(rec)
}

/// Escreve os frames no `stdin` do ffmpeg. Sem frame novo por `REENVIO`, repete o último.
/// Sai quando a fila fecha (gravação parada) ou o ffmpeg fecha a entrada (caiu).
fn alimenta(mut stdin: ChildStdin, rx: Receiver<Vec<u8>>) {
    let mut ultimo: Option<Vec<u8>> = None;
    loop {
        match rx.recv_timeout(REENVIO) {
            Ok(frame) => {
                if stdin.write_all(&frame).is_err() {
                    break;
                }
                ultimo = Some(frame);
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Some(f) = &ultimo {
                    if stdin.write_all(f).is_err() {
                        break;
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    // Ao sair, `stdin` é fechado: o ffmpeg recebe EOF e finaliza o arquivo.
}

fn motivo_saida(status: ExitStatus) -> String {
    format!("{}: {status}", tr!("O FFmpeg parou inesperadamente"))
}

impl ActiveRecording {
    /// Envia um quadro RGBA (`largura * altura * 4` bytes) sem bloquear.
    /// Retorna `false` se a fila está cheia ou o ffmpeg já caiu; o quadro é descartado.
    pub fn enviar(&self, quadro: Vec<u8>) -> bool {
        self.frames.as_ref().is_some_and(|tx| tx.try_send(quadro).is_ok())
    }

    /// `true` se o encoder usado é de hardware (candidato a retomar por software).
    pub fn e_hardware(&self) -> bool {
        self.config.backend != VideoBackend::Software
    }

    /// Diz se o ffmpeg caiu. `Some(motivo)` se parou; `None` enquanto roda.
    /// Não bloqueia: é chamado a cada tique, no loop de renderização.
    pub fn viva(&mut self) -> Option<String> {
        self.child.as_ref()?;
        match self.observar() {
            Ok(None) => None,
            Ok(Some(status)) => Some(motivo_saida(status)),
            Err(e) => Some(format!("{}: {e}", tr!("Falha ao verificar o FFmpeg"))),
        }
    }

    /// Status do ffmpeg, se já saiu. Guarda o status: depois de observado, não se perde.
    fn observar(&mut self) -> std::io::Result<Option<ExitStatus>> {
        if self.saida.is_some() {
            return Ok(self.saida);
        }
        let Some(child) = self.child.as_mut() else { return Ok(None) };
        let st = status_de(child)?;
        self.saida = st;
        Ok(st)
    }

    /// Para a gravação: fecha a entrada, espera o ffmpeg finalizar (até 15 s) e devolve
    /// sempre o caminho do que foi gravado, com o motivo se algo deu errado. Bloqueia:
    /// deve rodar numa thread própria.
    pub fn parar(mut self) -> Encerramento {
        // Soltar o sender faz a thread de alimentação drenar a fila e fechar o stdin.
        self.frames.take();
        let caminho = self.caminho.clone();
        if self.child.is_none() {
            return Encerramento { caminho, erro: Some(tr!("A gravação já foi encerrada.").to_string()) };
        }

        let prazo = Instant::now() + TIMEOUT_PARAR;
        let erro = loop {
            match self.observar() {
                Ok(Some(status)) if status.success() => break None,
                Ok(Some(status)) => break Some(motivo_saida(status)),
                Ok(None) if Instant::now() < prazo => thread::sleep(Duration::from_millis(50)),
                Ok(None) => {
                    if let Some(c) = self.child.as_mut() {
                        let _ = c.kill();
                        let _ = c.wait();
                    }
                    break Some(tr!("O FFmpeg não finalizou a tempo.").to_string());
                }
                Err(e) => break Some(format!("{}: {e}", tr!("Falha ao aguardar o FFmpeg"))),
            }
        };
        self.child = None;
        // Um arquivo vazio não serve a ninguém: isso também é um erro a mostrar.
        let vazio = fs::metadata(&caminho).map(|m| m.len() == 0).unwrap_or(true);
        let erro = erro.or_else(|| vazio.then(|| tr!("O arquivo da gravação ficou vazio.").to_string()));
        Encerramento { caminho, erro }
    }
}

impl Drop for ActiveRecording {
    /// Se a gravação for descartada sem `parar` (ex.: o compositor fechando), pede o
    /// fim limpo ao ffmpeg, para ele fechar o arquivo, e só mata se ele não responder.
    fn drop(&mut self) {
        self.frames.take();
        if self.child.is_none() {
            return;
        }
        let prazo = Instant::now() + TIMEOUT_PARAR;
        while Instant::now() < prazo {
            match self.observar() {
                Ok(None) => thread::sleep(Duration::from_millis(50)),
                _ => break,
            }
        }
        if let Some(c) = self.child.as_mut() {
            if self.saida.is_none() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
        self.child = None;
    }
}

#[cfg(test)]
mod status_testes {
    use super::*;

    /// O compositor recolhe o filho antes da gravação: o status registrado não se perde.
    #[test]
    fn status_recolhido_pelo_compositor_nao_se_perde() {
        let mut child = Command::new("sh").args(["-c", "exit 3"]).spawn().unwrap();
        let pid = child.id();
        // Simula o `waitpid(-1)` do compositor: recolhe o filho e registra o status.
        let mut status: libc::c_int = 0;
        let r = unsafe { libc::waitpid(pid as libc::pid_t, &mut status, 0) };
        assert_eq!(r, pid as libc::pid_t);
        registrar_saida(pid, status);
        let observado = status_de(&mut child).unwrap().expect("o status deve estar registrado");
        assert_eq!(observado.code(), Some(3));
        // Depois de lido, o sistema já não tem o status: o registro é a única fonte.
        assert!(child.try_wait().is_err());
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pedido(pasta: PathBuf) -> RecordingRequest {
        RecordingRequest {
            largura: 64,
            altura: 48,
            fps: 10,
            pasta_saida: pasta,
            audio_sistema: false,
            microfone: false,
        }
    }

    fn cfg(audio: Option<&'static str>) -> EncoderConfig {
        EncoderConfig {
            backend: VideoBackend::Software,
            video_codec: "mpeg4",
            audio_codec: audio,
            pixel_format: Some("yuv420p"),
            pre_input_args: vec![],
            video_args: vec!["-q:v".into(), "5".into()],
            file_extension: "mp4",
        }
    }

    #[test]
    fn args_com_duas_fontes_de_audio_fazem_mix() {
        let mut r = pedido(PathBuf::from("/tmp"));
        r.audio_sistema = true;
        r.microfone = true;
        let a = montar_args(&cfg(Some("aac")), &r, Path::new("/tmp/x.mp4"));
        assert!(a.contains(&"@DEFAULT_MONITOR@".to_string()));
        assert!(a.contains(&"@DEFAULT_SOURCE@".to_string()));
        assert!(a.contains(&"-filter_complex".to_string()));
        assert!(a.windows(2).any(|w| w == ["-c:a", "aac"]));
        assert!(a.contains(&"pipe:0".to_string()));
    }

    #[test]
    fn mp4_e_fragmentado_para_sobreviver_a_queda() {
        let a = montar_args(&cfg(None), &pedido(PathBuf::from("/tmp")), Path::new("/tmp/x.mp4"));
        assert!(a.contains(&"+frag_keyframe+empty_moov+default_base_moof".to_string()));
        assert!(!a.iter().any(|x| x.contains("faststart")));
    }

    #[test]
    fn nome_segue_o_padrao_com_data_e_hora() {
        let dir = std::env::temp_dir().join(format!("glove-recorder-nome-{}", std::process::id()));
        let caminho = caminho_saida(&pedido(dir.clone()), "mp4").unwrap();
        let nome = caminho.file_name().unwrap().to_string_lossy().to_string();
        let _ = fs::remove_dir_all(&dir);
        // "Gravação de tela-aaaa-mm-dd hh-mm-ss.mp4"
        let prefixo = format!("{}-", tr!("Gravação de tela"));
        assert!(nome.starts_with(&prefixo), "{nome}");
        let data = &nome[prefixo.len()..nome.len() - ".mp4".len()];
        assert_eq!(data.len(), "aaaa-mm-dd hh-mm-ss".len(), "{nome}");
        assert!(nome.ends_with(".mp4"));
    }

    #[test]
    fn pasta_padrao_termina_em_gravacoes_de_tela() {
        let p = pasta_padrao();
        assert!(p.ends_with(tr!("Gravações de tela")), "{}", p.display());
    }

    /// Um encoder que a máquina não consegue abrir não pode ser escolhido, mesmo listado.
    #[test]
    fn encoder_que_nao_abre_e_recusado_pela_sonda() {
        let mut enc = crate::Encoders::default();
        enc.video.insert("encoder_inexistente".to_string());
        assert!(escolhe_validado(enc, None).is_err());
    }

    #[test]
    fn sem_encoder_de_audio_nao_mapeia_audio() {
        let mut r = pedido(PathBuf::from("/tmp"));
        r.microfone = true;
        let a = montar_args(&cfg(None), &r, Path::new("/tmp/x.mp4"));
        assert!(!a.contains(&"-c:a".to_string()));
        assert!(!a.contains(&"@DEFAULT_SOURCE@".to_string()));
    }

    /// Grava quadros de verdade com o `ffmpeg` local (usa `mpeg4`, que existe em qualquer build).
    #[test]
    fn grava_quadros_no_arquivo() {
        if Command::new("ffmpeg").arg("-version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("glove-recorder-teste-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let r = pedido(dir.clone());
        let mut rec = inicia(&cfg(None), &r, dir.join("teste.mp4")).expect("o ffmpeg deve iniciar");

        let aceitos = (0..20)
            .filter(|i| {
                let mut quadro = vec![128u8; 64 * 48 * 4];
                quadro[0] = *i as u8;
                thread::sleep(Duration::from_millis(50));
                rec.enviar(quadro)
            })
            .count();
        assert!(aceitos >= 10, "a fila aceitou só {aceitos} quadros");
        assert!(rec.viva().is_none(), "o ffmpeg deveria estar rodando");

        let fim = rec.parar();
        let tamanho = fs::metadata(&fim.caminho).map(|m| m.len()).unwrap_or(0);
        let _ = fs::remove_dir_all(&dir);
        assert!(fim.erro.is_none(), "{:?}", fim.erro);
        assert!(tamanho > 0, "arquivo vazio: {}", fim.caminho.display());
    }

    /// Se o ffmpeg cai no meio, `viva` avisa e `parar` ainda devolve o que foi gravado.
    #[test]
    fn queda_no_meio_devolve_o_arquivo_parcial() {
        if Command::new("ffmpeg").arg("-version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("glove-recorder-queda-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let r = pedido(dir.clone());
        let mut rec = inicia(&cfg(None), &r, dir.join("queda.mp4")).expect("o ffmpeg deve iniciar");
        for i in 0..10u8 {
            let mut quadro = vec![128u8; 64 * 48 * 4];
            quadro[0] = i;
            rec.enviar(quadro);
            thread::sleep(Duration::from_millis(50));
        }
        // Simula a queda: mata o ffmpeg como se o sistema o tivesse derrubado.
        if let Some(c) = rec.child.as_mut() {
            let _ = c.kill();
        }
        let motivo = (0..100).find_map(|_| {
            thread::sleep(Duration::from_millis(20));
            rec.viva()
        });
        assert!(motivo.is_some(), "viva deveria acusar a queda");

        let fim = rec.parar();
        let tamanho = fs::metadata(&fim.caminho).map(|m| m.len()).unwrap_or(0);
        let _ = fs::remove_dir_all(&dir);
        assert!(fim.erro.is_some(), "a queda deve aparecer no erro");
        assert!(tamanho > 0, "o trecho gravado deve ser salvo");
    }
}
