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
