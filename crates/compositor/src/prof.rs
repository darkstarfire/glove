//! `GLOVE_PROF=1`: a cada 2 segundos, o registro diz quantos quadros o
//! compositor fez e quanto tempo gastou preparando (desenhos na CPU: Dock,
//! barra, títulos), montando a lista do quadro e mandando desenhar (GPU).

use std::cell::RefCell;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Acc {
    since: Option<Instant>,
    frames: u32,
    prep: Duration,
    prep_max: Duration,
    build: Duration,
    render: Duration,
    render_max: Duration,
}

thread_local! {
    static ACC: RefCell<Acc> = RefCell::new(Acc::default());
}

pub fn on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("GLOVE_PROF").is_some())
}

/// Tempo de preparar o quadro (conta um quadro).
pub fn prep(d: Duration) {
    ACC.with(|a| {
        let mut a = a.borrow_mut();
        a.frames += 1;
        a.prep += d;
        a.prep_max = a.prep_max.max(d);
    });
    report();
}

thread_local! {
    static LAST_VBLANK: RefCell<Option<Instant>> = const { RefCell::new(None) };
    /// Quadros entregues ao kernel e ainda não confirmados pelo vblank: (quando, elementos).
    static QUEUED: RefCell<std::collections::HashMap<String, (Instant, usize)>> = RefCell::new(Default::default());
}

/// Um quadro foi entregue à tela (queue_frame): começa a contar até o flip.
pub fn queued(tela: &str, elementos: usize) {
    // Mantém o mais antigo ainda não confirmado: o flip atrasado conta desde a entrega dele.
    QUEUED.with(|q| {
        q.borrow_mut().entry(tela.to_string()).or_insert((Instant::now(), elementos));
    });
}

/// Há quanto tempo o quadro mais antigo da tela ainda espera o vblank.
pub fn esperando(tela: &str) -> Option<Duration> {
    QUEUED.with(|q| {
        let q = q.borrow();
        q.get(tela).map(|(t, _)| t.elapsed())
    })
}

/// Um vblank chegou: se passou muito tempo desde o anterior, avisa (com GLOVE_PROF).
pub fn vblank(tela: &str) {
    let now = Instant::now();
    let queued = QUEUED.with(|q| q.borrow_mut().remove(tela));
    if !on() {
        return;
    }
    if let Some((t, n)) = queued {
        let lat = now - t;
        if lat > Duration::from_millis(33) {
            tracing::warn!("{tela}: quadro com {n} elementos levou {:?} para aparecer", lat);
        }
    }
    LAST_VBLANK.with(|l| {
        let mut l = l.borrow_mut();
        if let Some(prev) = *l {
            let gap = now - prev;
            if gap > Duration::from_millis(50) {
                tracing::warn!("{tela}: {:?} sem vblank", gap);
            }
        }
        *l = Some(now);
    });
}

pub fn build(d: Duration) {
    ACC.with(|a| a.borrow_mut().build += d);
}

pub fn render(d: Duration) {
    ACC.with(|a| {
        let mut a = a.borrow_mut();
        a.render += d;
        a.render_max = a.render_max.max(d);
    });
}

fn report() {
    ACC.with(|a| {
        let mut a = a.borrow_mut();
        let since = *a.since.get_or_insert_with(Instant::now);
        let el = since.elapsed();
        if el < Duration::from_secs(2) {
            return;
        }
        let n = a.frames.max(1);
        tracing::info!(
            "perfil: {:.1} quadros/s | preparar {:?} (pior {:?}) | montar {:?} | desenhar {:?} (pior {:?}) | CPU no laço ≈ {:.0}%",
            a.frames as f64 / el.as_secs_f64(),
            a.prep / n,
            a.prep_max,
            a.build / n,
            a.render / n,
            a.render_max,
            (a.prep + a.build + a.render).as_secs_f64() / el.as_secs_f64() * 100.0
        );
        *a = Acc { since: Some(Instant::now()), ..Acc::default() };
    });
}
