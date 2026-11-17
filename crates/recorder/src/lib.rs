//! Gravação de tela: escolha do encoder do FFmpeg.
//!
//! Nenhum codec é fixo. `detect_best_ffmpeg_config` consulta o `ffmpeg` instalado
//! (`ffmpeg -hide_banner -encoders`) e escolhe, pela ordem de preferência, um
//! encoder de hardware disponível; se nenhum servir, usa um de software.

use glove_config::tr;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

mod sessao;
pub use sessao::{ActiveRecording, Encerramento, RecordingRequest, iniciar, iniciar_software, montar_args, pasta_padrao, registrar_saida};

/// Como o vídeo é codificado. Define os argumentos de entrada e o filtro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoBackend {
    Vaapi,
    Nvenc,
    Qsv,
    V4l2m2m,
    Software,
}

/// Configuração de codificação escolhida para uma gravação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderConfig {
    pub backend: VideoBackend,
    pub video_codec: &'static str,
    /// Nome do encoder de áudio (`-c:a`). `None` quando não há nenhum: grava-se sem som.
    pub audio_codec: Option<&'static str>,
    /// `-pix_fmt` da saída. `None` quando o filtro (`hwupload`) já entrega o formato.
    pub pixel_format: Option<&'static str>,
    /// Argumentos antes de `-f rawvideo -i pipe:0` (ex.: `-vaapi_device`).
    pub pre_input_args: Vec<String>,
    /// Argumentos de vídeo: filtros, presets e qualidade. Vão depois do codec.
    pub video_args: Vec<String>,
    /// Extensão do arquivo, sem o ponto.
    pub file_extension: &'static str,
}

/// Encoders listados pelo `ffmpeg -encoders`, separados por tipo.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Encoders {
    video: HashSet<String>,
    audio: HashSet<String>,
}

impl Encoders {
    /// Mesma lista, sem os encoders de hardware. Usada no fallback.
    pub fn sem_hardware(&self) -> Encoders {
        let video = self
            .video
            .iter()
            .filter(|c| {
                !CANDIDATOS
                    .iter()
                    .any(|(nome, b)| nome == c && *b != VideoBackend::Software)
            })
            .cloned()
            .collect();
        Encoders { video, audio: self.audio.clone() }
    }

    /// Mesma lista, sem um codec de vídeo (quando a sonda mostra que ele não funciona).
    pub fn sem_codec(&self, codec: &str) -> Encoders {
        let mut e = self.clone();
        e.video.remove(codec);
        e
    }
}

/// Vídeo, na ordem de preferência. O padrão é `libx264` (software, portátil).
/// Se ele não estiver disponível, entram os demais: hardware, depois software.
/// `mpeg4` é o fallback universal.
const CANDIDATOS: [(&str, VideoBackend); 11] = [
    ("libx264", VideoBackend::Software),
    ("h264_vaapi", VideoBackend::Vaapi),
    ("hevc_vaapi", VideoBackend::Vaapi),
    ("h264_nvenc", VideoBackend::Nvenc),
    ("hevc_nvenc", VideoBackend::Nvenc),
    ("h264_qsv", VideoBackend::Qsv),
    ("h264_v4l2m2m", VideoBackend::V4l2m2m),
    ("libx265", VideoBackend::Software),
    ("libvpx-vp9", VideoBackend::Software),
    ("libaom-av1", VideoBackend::Software),
    ("mpeg4", VideoBackend::Software),
];

/// Consulta o `ffmpeg` local e monta a melhor configuração disponível.
///
/// Roda um processo filho, então deve ser chamada fora do loop de renderização.
pub fn detect_best_ffmpeg_config() -> Result<EncoderConfig, String> {
    let encoders = detect_encoders()?;
    build_config(&encoders, find_render_node().as_deref())
}

/// Lista os encoders do `ffmpeg` local (roda um processo filho).
pub fn detect_encoders() -> Result<Encoders, String> {
    let saida = Command::new("ffmpeg")
        .args(["-hide_banner", "-encoders"])
        .output()
        .map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                tr!("O FFmpeg não foi encontrado. Instale-o (ex.: sudo apt install ffmpeg) e tente de novo.").to_string()
            } else {
                format!("{}: {e}", tr!("Não foi possível executar o FFmpeg"))
            }
        })?;

    if !saida.status.success() {
        return Err(tr!("O FFmpeg não conseguiu listar os encoders disponíveis.").to_string());
    }

    let texto = String::from_utf8_lossy(&saida.stdout);
    Ok(parse_encoders(&texto))
}

/// Lê a saída de `ffmpeg -encoders`. Cada linha de encoder começa com seis
/// flags (`V` vídeo, `A` áudio, ...), seguidas do nome. Cabeçalho e legenda são ignorados.
pub fn parse_encoders(saida: &str) -> Encoders {
    let mut encoders = Encoders::default();
    for linha in saida.lines() {
        let mut campos = linha.split_whitespace();
        let (Some(flags), Some(nome)) = (campos.next(), campos.next()) else {
            continue;
        };
        // A legenda tem o formato " V..... = Video": o nome é "=", não um encoder.
        if flags.len() != 6 || nome == "=" {
            continue;
        }
        match flags.as_bytes()[0] {
            b'V' => {
                encoders.video.insert(nome.to_string());
            }
            b'A' => {
                encoders.audio.insert(nome.to_string());
            }
            _ => {}
        }
    }
    encoders
}

/// Primeiro nó de render da GPU (ex.: `/dev/dri/renderD128`), se existir.
pub fn find_render_node() -> Option<PathBuf> {
    let mut nodes: Vec<PathBuf> = std::fs::read_dir("/dev/dri")
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("renderD"))
        })
        .collect();
    nodes.sort();
    nodes.into_iter().next()
}

/// Monta a configuração a partir dos encoders listados. Não toca no sistema,
/// então pode ser testada com saídas simuladas.
pub fn build_config(encoders: &Encoders, render: Option<&Path>) -> Result<EncoderConfig, String> {
    let (video_codec, backend, device) = escolhe_video(encoders, render)?;
    let (pre_input_args, video_args, pixel_format) = monta_video(video_codec, backend, device)?;
    // Áudio correspondente ao vídeo: AAC para H.264/HEVC/MPEG-4; Opus para VP9/AV1.
    // `libmp3lame` é o último recurso (o FFmpeg não tem um encoder chamado `mp3`).
    let vp_av1 = matches!(video_codec, "libvpx-vp9" | "libaom-av1");
    let audio_pref: [&'static str; 3] = if vp_av1 {
        ["libopus", "aac", "libmp3lame"]
    } else {
        ["aac", "libopus", "libmp3lame"]
    };
    let audio_codec = audio_pref.into_iter().find(|c| encoders.audio.contains(*c));
    // VP9/AV1 vão em MKV: o WebM não aceita AAC.
    let file_extension = if vp_av1 { "mkv" } else { "mp4" };

    Ok(EncoderConfig {
        backend,
        video_codec,
        audio_codec,
        pixel_format,
        pre_input_args,
        video_args,
        file_extension,
    })
}

/// Escolhe o primeiro candidato disponível, na ordem de `CANDIDATOS`.
/// VAAPI só entra se houver um nó de render.
fn escolhe_video<'a>(
    encoders: &Encoders,
    render: Option<&'a Path>,
) -> Result<(&'static str, VideoBackend, Option<&'a Path>), String> {
    for (codec, backend) in CANDIDATOS {
        if !encoders.video.contains(codec) {
            continue;
        }
        match backend {
            // VAAPI sem nó de render não funciona: pula para o próximo candidato.
            VideoBackend::Vaapi => {
                if let Some(node) = render {
                    return Ok((codec, backend, Some(node)));
                }
            }
            _ => return Ok((codec, backend, None)),
        }
    }
    Err(tr!("Nenhum encoder de vídeo utilizável foi encontrado no FFmpeg instalado.").to_string())
}

/// (antes da entrada, de vídeo, pix_fmt)
type ArgsVideo = (Vec<String>, Vec<String>, Option<&'static str>);

/// Argumentos específicos de cada encoder.
fn monta_video(
    codec: &'static str,
    backend: VideoBackend,
    device: Option<&Path>,
) -> Result<ArgsVideo, String> {
    let (pre, mut args, pix): ArgsVideo = match backend {
        VideoBackend::Vaapi => {
            let node = device.ok_or_else(|| tr!("VAAPI exige um nó de render da GPU.").to_string())?;
            (
                v(&["-vaapi_device", &node.display().to_string()]),
                v(&["-vf", "format=nv12,hwupload", "-qp", "23"]),
                None,
            )
        }
        VideoBackend::Nvenc => (
            vec![],
            v(&["-preset", "p1", "-rc", "vbr", "-cq", "23", "-b:v", "0"]),
            Some("yuv420p"),
        ),
        VideoBackend::Qsv => (
            v(&["-init_hw_device", "qsv=hw", "-filter_hw_device", "hw"]),
            v(&["-vf", "format=nv12,hwupload=extra_hw_frames=64", "-preset", "veryfast", "-global_quality", "23"]),
            None,
        ),
        VideoBackend::V4l2m2m => (vec![], v(&["-b:v", "4M"]), Some("yuv420p")),
        VideoBackend::Software => {
            let (args, pix) = match codec {
                "libx264" => (v(&["-preset", "veryfast", "-crf", "23"]), "yuv420p"),
                "libx265" => (v(&["-preset", "fast", "-crf", "28"]), "yuv420p"),
                "libvpx-vp9" => (
                    v(&["-deadline", "realtime", "-cpu-used", "8", "-row-mt", "1", "-crf", "32", "-b:v", "0"]),
                    "yuv420p",
                ),
                "libaom-av1" => (
                    v(&["-cpu-used", "8", "-row-mt", "1", "-crf", "35", "-b:v", "0"]),
                    "yuv420p",
                ),
                _ => (v(&["-q:v", "5"]), "yuv420p"), // mpeg4
            };
            (vec![], args, Some(pix))
        }
    };

    // HEVC no MP4 precisa da tag hvc1 para tocar em Apple e em outros players.
    if codec.contains("hevc") || codec == "libx265" {
        args.extend(v(&["-tag:v", "hvc1"]));
    }
    Ok((pre, args, pix))
}

fn v(itens: &[&str]) -> Vec<String> {
    itens.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    const SAIDA: &str = "Encoders:
 V..... = Video
 A..... = Audio
 S..... = Subtitle
 .F.... = Frame-level multithreading
 ------
 V....D h264_vaapi           H.264/AVC (VAAPI) (codec h264)
 V....D h264_nvenc           NVIDIA NVENC H.264 encoder (codec h264)
 V....D libx264              libx264 H.264 / AVC / MPEG-4 AVC (codec h264)
 V....D libvpx-vp9           libvpx VP9 (codec vp9)
 V....D mpeg4                MPEG-4 part 2 (codec mpeg4)
 A....D aac                  AAC (Advanced Audio Coding)
 A....D libopus              libopus Opus
";

    fn enc(saida: &str) -> Encoders {
        parse_encoders(saida)
    }

    #[test]
    fn parse_ignora_cabecalho_e_legenda() {
        let e = enc(SAIDA);
        assert!(e.video.contains("libx264"));
        assert!(e.audio.contains("aac"));
        assert!(!e.video.contains("="));
        assert!(!e.video.contains("Video"));
        assert!(!e.video.contains("Encoders:"));
    }

    #[test]
    fn libx264_com_aac_e_o_padrao() {
        let c = build_config(&enc(SAIDA), None).unwrap();
        assert_eq!(c.video_codec, "libx264");
        assert_eq!(c.backend, VideoBackend::Software);
        assert_eq!(c.audio_codec, Some("aac"));
        assert_eq!(c.file_extension, "mp4");
        assert_eq!(c.pixel_format, Some("yuv420p"));
    }

    #[test]
    fn sem_libx264_cai_para_hardware() {
        let sem_x264 = "V....D h264_nvenc  x\nV....D libvpx-vp9  x\nA....D aac  x\n";
        let c = build_config(&enc(sem_x264), None).unwrap();
        assert_eq!(c.video_codec, "h264_nvenc");
        assert_eq!(c.backend, VideoBackend::Nvenc);
    }

    #[test]
    fn vp9_usa_opus_quando_disponivel() {
        let so_vp9 = "V....D libvpx-vp9  x\nA....D libopus  x\nA....D aac  x\n";
        let c = build_config(&enc(so_vp9), None).unwrap();
        assert_eq!(c.audio_codec, Some("libopus"));
        assert_eq!(c.file_extension, "mkv");
    }

    #[test]
    fn mp3_usa_o_encoder_libmp3lame() {
        let so_mp3 = "V....D libx264  x\nA....D libmp3lame  x\n";
        let c = build_config(&enc(so_mp3), None).unwrap();
        assert_eq!(c.audio_codec, Some("libmp3lame"));
    }

    #[test]
    fn vaapi_sem_render_node_e_pulado() {
        let so_vaapi = "V....D h264_vaapi  x\nV....D libx264  x\n";
        let c = build_config(&enc(so_vaapi), None).unwrap();
        assert_eq!(c.video_codec, "libx264");
        assert_eq!(c.backend, VideoBackend::Software);
        assert_eq!(c.pixel_format, Some("yuv420p"));
    }

    #[test]
    fn vaapi_monta_device_e_filtro_hwupload() {
        let so_vaapi = "V....D h264_vaapi  x\n";
        let node = Path::new("/dev/dri/renderD128");
        let c = build_config(&enc(so_vaapi), Some(node)).unwrap();
        assert_eq!(c.backend, VideoBackend::Vaapi);
        assert_eq!(c.pre_input_args, vec!["-vaapi_device", "/dev/dri/renderD128"]);
        assert!(c.video_args.contains(&"format=nv12,hwupload".to_string()));
        assert_eq!(c.pixel_format, None);
    }

    #[test]
    fn vp9_vai_para_mkv_e_sem_audio_quando_nao_ha_encoder() {
        let so_vp9 = "V....D libvpx-vp9  x\n";
        let c = build_config(&enc(so_vp9), None).unwrap();
        assert_eq!(c.video_codec, "libvpx-vp9");
        assert_eq!(c.file_extension, "mkv");
        assert_eq!(c.audio_codec, None);
    }

    #[test]
    fn sem_encoder_de_video_da_erro() {
        assert!(build_config(&enc("A....D aac  x\n"), None).is_err());
    }
}
