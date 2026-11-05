//! Tocador: decodifica com o symphonia (Rust puro) numa thread e manda o som
//! (PCM de 16 bits) para o pw-cat do PipeWire, ou para o aplay do ALSA.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};

use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::{MetadataOptions, StandardTagKey},
    probe::Hint,
    units::Time,
};

/// Tags e duração de uma música.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub secs: f32,
}

fn open(path: &Path) -> Option<(Box<dyn FormatReader>, Option<symphonia::core::meta::MetadataRevision>)> {
    let file = std::fs::File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(e) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(e);
    }
    let mut probed = symphonia::default::get_probe().format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default()).ok()?;
    // Tags podem vir antes do contêiner (ID3) ou dentro dele.
    let meta = probed.metadata.get().and_then(|m| m.current().cloned());
    let format = probed.format;
    Some((format, meta))
}

/// Lê as tags e a duração (rápido: não decodifica o som).
pub fn read_info(path: &Path) -> Info {
    let mut info = Info { title: path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), ..Default::default() };
    let Some((mut format, meta)) = open(path) else { return info };
    let inner = format.metadata().current().cloned();
    for rev in [meta, inner].into_iter().flatten() {
        for tag in rev.tags() {
            let v = tag.value.to_string();
            match tag.std_key {
                Some(StandardTagKey::TrackTitle) if !v.is_empty() => info.title = v,
                Some(StandardTagKey::Artist) | Some(StandardTagKey::AlbumArtist) if info.artist.is_empty() => info.artist = v,
                Some(StandardTagKey::Album) => info.album = v,
                _ => {}
            }
        }
    }
    if let Some(t) = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL) {
        let p = &t.codec_params;
        if let (Some(n), Some(tb)) = (p.n_frames, p.time_base) {
            let t = tb.calc_time(n);
            info.secs = t.seconds as f32 + t.frac as f32;
        } else if let (Some(n), Some(r)) = (p.n_frames, p.sample_rate) {
            info.secs = n as f32 / r as f32;
        }
    }
    info
}

pub enum Cmd {
    Play(PathBuf),
    Pause,
    Resume,
    Stop,
    Seek(f32),
    Volume(f32),
}

#[derive(Default)]
pub struct Status {
    pub playing: Option<PathBuf>,
    pub paused: bool,
    pub pos: f32,
    pub secs: f32,
    /// A música acabou sozinha (a janela passa para a próxima).
    pub ended: bool,
    pub error: Option<String>,
}

/// Saída de som: o pw-cat (PipeWire) ou, sem ele, o aplay (ALSA).
fn sink(rate: u32, channels: usize) -> Option<Child> {
    Command::new("pw-cat")
        .args(["--playback", "--raw", "--format", "s16", "--rate", &rate.to_string(), "--channels", &channels.to_string(), "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .or_else(|_| Command::new("aplay").args(["-q", "-f", "S16_LE", "-r", &rate.to_string(), "-c", &channels.to_string()]).stdin(Stdio::piped()).stderr(Stdio::null()).spawn())
        .ok()
}

/// A thread do tocador: espera comandos e toca.
pub fn spawn(status: Arc<Mutex<Status>>) -> Sender<Cmd> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || run(rx, status));
    tx
}

fn run(rx: Receiver<Cmd>, status: Arc<Mutex<Status>>) {
    let mut volume = 0.8f32;
    let mut next: Option<Cmd> = None;
    loop {
        let cmd = match next.take() {
            Some(c) => c,
            None => match rx.recv() {
                Ok(c) => c,
                Err(_) => return,
            },
        };
        match cmd {
            Cmd::Volume(v) => volume = v,
            Cmd::Play(path) => next = play(&path, &rx, &status, &mut volume),
            _ => {}
        }
    }
}

/// Toca uma música até acabar ou chegar outro comando que a interrompa
/// (devolvido para o laço principal).
fn play(path: &Path, rx: &Receiver<Cmd>, status: &Arc<Mutex<Status>>, volume: &mut f32) -> Option<Cmd> {
    let fail = |e: &str| {
        let mut s = status.lock().unwrap();
        s.playing = None;
        s.error = Some(e.to_string());
        s.ended = true;
    };
    let Some((mut format, _)) = open(path) else {
        fail("formato não reconhecido");
        return None;
    };
    let Some(track) = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL).cloned() else {
        fail("sem faixa de áudio");
        return None;
    };
    let Ok(mut decoder) = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()) else {
        fail("codec não suportado");
        return None;
    };
    let rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2).max(1);
    let secs = track.codec_params.n_frames.map(|n| n as f32 / rate as f32).unwrap_or(0.0);
    {
        let mut s = status.lock().unwrap();
        *s = Status { playing: Some(path.to_path_buf()), paused: false, pos: 0.0, secs, ended: false, error: None };
    }
    let Some(mut out) = sink(rate, channels) else {
        fail("sem saída de som (pw-cat ou aplay)");
        return None;
    };
    let mut stdin = out.stdin.take();
    let mut frames: u64 = 0;
    let mut paused = false;
    let mut buf: Option<SampleBuffer<i16>> = None;
    let result = loop {
        // Comandos chegando no meio da música.
        let cmd = if paused { rx.recv().ok() } else {
            match rx.try_recv() {
                Ok(c) => Some(c),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => break None,
            }
        };
        match cmd {
            Some(Cmd::Pause) => {
                paused = true;
                status.lock().unwrap().paused = true;
                continue;
            }
            Some(Cmd::Resume) => {
                paused = false;
                status.lock().unwrap().paused = false;
            }
            Some(Cmd::Volume(v)) => *volume = v,
            Some(Cmd::Seek(t)) => {
                if format.seek(SeekMode::Coarse, SeekTo::Time { time: Time::from(t.max(0.0) as f64), track_id: Some(track.id) }).is_ok() {
                    decoder.reset();
                    frames = (t.max(0.0) * rate as f32) as u64;
                }
            }
            Some(c @ (Cmd::Play(_) | Cmd::Stop)) => break Some(c),
            None if paused => break None,
            None => {}
        }
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => {
                // Fim do arquivo.
                status.lock().unwrap().ended = true;
                break None;
            }
        };
        if packet.track_id() != track.id {
            continue;
        }
        let Ok(decoded) = decoder.decode(&packet) else { continue };
        let spec = *decoded.spec();
        let b = buf.get_or_insert_with(|| SampleBuffer::<i16>::new(decoded.capacity() as u64, spec));
        if b.capacity() < decoded.capacity() * spec.channels.count() {
            *b = SampleBuffer::<i16>::new(decoded.capacity() as u64, spec);
        }
        b.copy_interleaved_ref(decoded);
        let v = *volume;
        let bytes: Vec<u8> = b.samples().iter().flat_map(|s| (((*s as f32) * v) as i16).to_le_bytes()).collect();
        if let Some(w) = stdin.as_mut() {
            if w.write_all(&bytes).is_err() {
                break None;
            }
        }
        frames += (b.samples().len() / channels) as u64;
        status.lock().unwrap().pos = frames as f32 / rate as f32;
    };
    // Fecha a saída (o pw-cat toca o que falta e sai) ou corta na hora num comando novo.
    drop(stdin);
    if result.is_some() {
        let _ = out.kill();
    }
    std::thread::spawn(move || out.wait());
    if matches!(result, Some(Cmd::Stop)) {
        let mut s = status.lock().unwrap();
        s.playing = None;
        s.pos = 0.0;
        return None;
    }
    result
}
