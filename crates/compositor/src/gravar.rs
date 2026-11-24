//! Gravação de tela no compositor. A tela (ou a área/janela escolhida) vai para o
//! `glove-recorder` a cada quadro renderizado. O ffmpeg sobe e para em threads
//! próprias, para a renderização nunca esperar por ele.
//!
//! Fluxo: ⇧⌘5 abre o painel (`glove-screenrec`). Ao clicar em Gravar, o app manda
//! `GRAVAR` e o compositor começa (tela cheia ou seleção de área/janela). Enquanto
//! grava, a barra mostra a bolinha e o tempo; clicando nela, "Parar Gravação".
//!
//! Queda: se o ffmpeg morrer no meio, o que já foi gravado é salvo e o usuário é
//! avisado (com "Abrir"). Se o encoder era de hardware, a gravação continua por
//! software num arquivo novo, sem o usuário precisar fazer nada.

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use glove_config::tr;
use glove_paint::Pixmap;
use glove_recorder::{ActiveRecording, Encerramento, RecordingRequest};
use smithay::{
    backend::renderer::gles::GlesRenderer,
    reexports::calloop::{
        channel::{channel, Event},
        timer::{TimeoutAction, Timer},
    },
    utils::{Logical, Rectangle},
};

use crate::{render::grab, shell::window_info, state::Glove, CalloopData};

/// Quadros por segundo. Com a tela parada, o `glove-recorder` repete o último quadro.
const FPS: u32 = 30;
const INTERVALO: Duration = Duration::from_millis(1000 / FPS as u64);
/// De quanto em quanto tempo a barra é redesenhada e a queda do ffmpeg é checada.
const TIQUE: Duration = Duration::from_millis(500);

/// (x, y, largura, altura) em pixels físicos. Largura e altura são pares (yuv420p).
type Recorte = (usize, usize, usize, usize);

/// Resultado de subir o ffmpeg: a gravação, o recorte, a área lógica e se é uma
/// retomada (depois de queda do hardware).
type Pronta = Result<(ActiveRecording, Recorte, Rectangle<i32, Logical>, bool), String>;

/// Gravação em andamento.
pub struct Gravacao {
    rec: ActiveRecording,
    recorte: Recorte,
    /// Área lógica escolhida: é dela que se retoma se o encoder cair.
    alvo: Rectangle<i32, Logical>,
    ultimo: Instant,
    inicio: Instant,
}

impl Glove {
    /// ⇧⌘3: grava a tela inteira.
    pub fn gravar_tela(&mut self) {
        let (lw, lh) = self.output_size();
        self.iniciar_gravacao(Rectangle::from_size((lw, lh).into()));
    }

    /// Escolhe uma área (ou uma janela, com `janela`) na tela e grava.
    pub fn gravar_area(&mut self, janela: bool) {
        if self.locked() {
            return;
        }
        self.region_gravar = true;
        self.start_region_shot(false, janela);
    }

    /// ⇧⌘5: com uma gravação em andamento, para; sem ela, abre o painel.
    pub fn gravar_alternar(&mut self) {
        if self.gravacao.is_some() {
            self.parar_gravacao();
        } else {
            self.abrir_painel();
        }
    }

    /// Abre o painel de gravação (uma vez só).
    pub fn abrir_painel(&mut self) {
        if self.locked() || self.greeter || self.gravacao.is_some() || self.gravacao_iniciando {
            return;
        }
        let aberto = self.space.elements().any(|w| window_info(w).1.eq_ignore_ascii_case("glove.ScreenRec"));
        if !aberto {
            self.spawn("glove-screenrec");
        }
    }

    /// O painel pediu a gravação: `modo` 0 = tela inteira, 1 = área, 2 = janela.
    pub fn pedido_gravacao(&mut self, modo: usize, sistema: bool, microfone: bool) {
        self.gravacao_audio = (sistema, microfone);
        match modo {
            0 => self.gravar_tela(),
            1 => self.gravar_area(false),
            _ => self.gravar_area(true),
        }
    }

    /// Começa a gravar `alvo` (lógico). Não bloqueia: o ffmpeg sobe numa thread e o
    /// resultado volta pelo loop.
    pub fn iniciar_gravacao(&mut self, alvo: Rectangle<i32, Logical>) {
        self.comecar(alvo, false, false);
    }

    /// Sobe a gravação. `software` força o encoder de software; `retomada` marca que
    /// é a continuação de uma gravação cujo encoder caiu (para avisar o usuário).
    fn comecar(&mut self, alvo: Rectangle<i32, Logical>, software: bool, retomada: bool) {
        if self.locked() || self.gravacao.is_some() || self.gravacao_iniciando {
            return;
        }
        let sc = self.scale();
        let (lw, lh) = self.output_size();
        let (pw, ph) = ((lw as f64 * sc).round() as usize, (lh as f64 * sc).round() as usize);
        // Coordenadas lógicas da área → pixels físicos, dentro da tela.
        let fisico = |v: f64, max: usize| (v.round().max(0.0) as usize).min(max);
        let x0 = fisico(alvo.loc.x as f64 * sc, pw);
        let y0 = fisico(alvo.loc.y as f64 * sc, ph);
        let x1 = fisico((alvo.loc.x + alvo.size.w) as f64 * sc, pw);
        let y1 = fisico((alvo.loc.y + alvo.size.h) as f64 * sc, ph);
        let (w, h) = (x1.saturating_sub(x0) & !1, y1.saturating_sub(y0) & !1);
        if w < 2 || h < 2 {
            tracing::warn!("gravação: área pequena demais");
            return;
        }

        let (sistema, microfone) = self.gravacao_audio;
        let pedido = RecordingRequest {
            largura: w as u32,
            altura: h as u32,
            fps: FPS,
            pasta_saida: glove_recorder::pasta_padrao(),
            audio_sistema: sistema,
            microfone,
        };
        self.gravacao_iniciando = true;
        self.dirty = true;

        let (tx, rx) = channel::<Pronta>();
        let _ = self.loop_handle.insert_source(rx, |ev, _, data: &mut CalloopData| {
            if let Event::Msg(r) = ev {
                data.state.gravacao_chegou(r);
            }
        });
        std::thread::spawn(move || {
            let subiu = if software { glove_recorder::iniciar_software(&pedido) } else { glove_recorder::iniciar(&pedido) };
            let resultado = subiu.map(|rec| (rec, (x0, y0, w, h), alvo, retomada));
            let _ = tx.send(resultado);
        });
    }

    fn gravacao_chegou(&mut self, r: Pronta) {
        self.gravacao_iniciando = false;
        match r {
            Ok((rec, recorte, alvo, retomada)) => {
                tracing::info!("gravação em {}", rec.caminho.display());
                if retomada {
                    notificar_gravacao(rec.caminho.clone(), &tr!("A gravação continua por software em um novo arquivo"), "");
                }
                let agora = Instant::now();
                self.gravacao = Some(Gravacao { rec, recorte, alvo, ultimo: agora, inicio: agora });
                self.dirty = true;
                self.agendar_tique();
            }
            Err(e) => {
                tracing::warn!("gravação não começou: {e}");
                notificar(&tr!("Não foi possível gravar a tela"), &e);
            }
        }
    }

    /// Redesenha a barra a cada `TIQUE` enquanto grava e checa se o ffmpeg caiu.
    fn agendar_tique(&mut self) {
        if self.gravacao_tique {
            return;
        }
        self.gravacao_tique = true;
        let _ = self.loop_handle.insert_source(Timer::from_duration(TIQUE), |_, _, data| {
            let st = &mut data.state;
            if let Some(motivo) = st.gravacao.as_mut().and_then(|g| g.rec.viva()) {
                st.gravacao_caiu(motivo);
            }
            if st.gravacao.is_none() {
                st.gravacao_tique = false;
                return TimeoutAction::Drop;
            }
            st.dirty = true;
            TimeoutAction::ToDuration(TIQUE)
        });
    }

    /// O ffmpeg caiu no meio. Salva o trecho gravado e avisa. Se o encoder era de
    /// hardware, retoma por software num arquivo novo.
    fn gravacao_caiu(&mut self, motivo: String) {
        let Some(g) = self.gravacao.take() else { return };
        self.dirty = true;
        tracing::warn!("gravação interrompida: {motivo}");
        let retomar = g.rec.e_hardware();
        let alvo = g.alvo;
        std::thread::spawn(move || {
            let fim = g.rec.parar();
            avisar_fim(fim, true);
        });
        if retomar {
            // Não há como saber se o hardware volta; a continuação é por software.
            self.comecar(alvo, true, true);
        }
    }

    /// Texto do tempo decorrido (m:ss, ou h:mm:ss) e se a bolinha está acesa (pisca).
    pub fn texto_gravacao(&self) -> (String, bool) {
        let Some(g) = self.gravacao.as_ref() else { return (String::new(), false) };
        let t = g.inicio.elapsed();
        let s = t.as_secs();
        let tempo = if s >= 3600 {
            format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
        } else {
            format!("{:02}:{:02}", s / 60, s % 60)
        };
        (tempo, (t.as_millis() / TIQUE.as_millis()) % 2 == 0)
    }

    /// Para a gravação de propósito. O arquivo é finalizado numa thread (até 15 s) e o
    /// usuário é avisado pelo sistema.
    pub fn parar_gravacao(&mut self) {
        let Some(g) = self.gravacao.take() else { return };
        self.dirty = true;
        std::thread::spawn(move || {
            let fim = g.rec.parar();
            avisar_fim(fim, false);
        });
    }
}

/// Chamado depois de cada quadro renderizado: envia a tela, já recortada, à gravação.
pub fn quadro(state: &mut Glove, renderer: &mut GlesRenderer) {
    let Some(g) = state.gravacao.as_mut() else { return };
    if g.ultimo.elapsed() < INTERVALO {
        return;
    }
    g.ultimo = Instant::now();
    let recorte = g.recorte;
    let Ok((pix, _)) = grab(state, renderer) else { return };
    if let (Some(quadro), Some(g)) = (recorta(&pix, recorte), state.gravacao.as_ref()) {
        g.rec.enviar(quadro);
    }
}

/// Copia o retângulo (RGBA, pixels físicos) de uma tela inteira.
fn recorta(pix: &Pixmap, (x, y, w, h): Recorte) -> Option<Vec<u8>> {
    let (pw, ph) = (pix.width() as usize, pix.height() as usize);
    if x + w > pw || y + h > ph {
        return None;
    }
    let stride = pw * 4;
    let dados = pix.data();
    let mut out = Vec::with_capacity(w * h * 4);
    for linha in y..y + h {
        let a = linha * stride + x * 4;
        out.extend_from_slice(&dados[a..a + w * 4]);
    }
    Some(out)
}

/// Avisa como a gravação terminou. Se há um trecho utilizável, mostra o caminho com
/// "Abrir"; se o arquivo ficou vazio, só a falha.
fn avisar_fim(fim: Encerramento, interrompida: bool) {
    let util = std::fs::metadata(&fim.caminho).map(|m| m.len() > 0).unwrap_or(false);
    if !util {
        let motivo = fim.erro.unwrap_or_default();
        tracing::warn!("gravação falhou: {motivo}");
        notificar(&tr!("Falha ao salvar a gravação de tela"), &motivo);
        return;
    }
    match fim.erro {
        None => {
            tracing::info!("gravação salva em {}", fim.caminho.display());
            notificar_gravacao(fim.caminho, &tr!("Gravação de tela salva"), "");
        }
        Some(motivo) => {
            tracing::warn!("gravação interrompida, trecho salvo em {}: {motivo}", fim.caminho.display());
            let titulo = if interrompida {
                tr!("Gravação interrompida. O que foi gravado foi salvo")
            } else {
                tr!("Gravação salva, mas com erro")
            };
            notificar_gravacao(fim.caminho, &titulo, &motivo);
        }
    }
}

/// Aviso pelo sistema (`notify-send`), numa thread: não espera.
fn notificar(titulo: &str, corpo: &str) {
    let (titulo, corpo) = (titulo.to_string(), corpo.to_string());
    std::thread::spawn(move || {
        let _ = Command::new("notify-send").args(["-a", "Glove", &titulo, &corpo]).status();
    });
}

/// Aviso com o caminho da gravação e o botão "Abrir" (abre no vídeo do Glove). O
/// `notify-send -w` espera o clique e imprime a ação escolhida.
fn notificar_gravacao(caminho: PathBuf, titulo: &str, extra: &str) {
    let titulo = titulo.to_string();
    let extra = extra.to_string();
    std::thread::spawn(move || {
        let mut corpo = caminho.display().to_string();
        if !extra.is_empty() {
            corpo.push('\n');
            corpo.push_str(&extra);
        }
        let acao = format!("abrir={}", tr!("Abrir"));
        let saida = Command::new("notify-send").args(["-a", "Glove", "-w", "-A", &acao, &titulo, &corpo]).output();
        let Ok(saida) = saida else { return };
        if String::from_utf8_lossy(&saida.stdout).lines().any(|l| l.trim() == "abrir") {
            let abriu = Command::new("glove-video").arg(&caminho).spawn();
            if abriu.is_err() {
                let _ = Command::new("mpv").arg(&caminho).spawn();
            }
        }
    });
}
