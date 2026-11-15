//! glove-bateria: a bateria do Chromebook lida direto do EC, como root (o serviço
//! `glove-bateria.service`). Versão em Rust do `cros-ec-battery` do
//! github.com/fabioaacarneiro/chromebook-linux-fixes, o mesmo que o Mide usa.
//!
//! Em Chromebooks com firmware alternativo (MrChromebox), o firmware diz ao Linux
//! "bateria ausente" quando não reconhece o modelo (comum com baterias paralelas),
//! embora o EC converse com ela. Este serviço lê a memória do EC (`/dev/cros_ec`) a
//! cada 5 s e grava `/run/cros-ec-battery.json`, que o Glove (glove-energia) mostra
//! na barra de menu.
//!
//! Como o Linux não sabe que há bateria, ninguém desligaria o computador quando ela
//! acabasse. Por isso o serviço também a protege:
//!   - na bateria, com carga <= POWEROFF_PCT (ou tensão baixa), desliga;
//!   - suspenso na bateria, o gancho do systemd acorda a cada SUSPEND_CHECK_S e roda
//!     `glove-bateria --resume-check`: desliga se a carga estiver baixa, ou volta a
//!     suspender se a tampa continuar fechada.
//!
//! Ajustes: /etc/default/glove-bateria (ou o /etc/default/chromebook-linux-fixes).
//!
//!   glove-bateria             mostra o estado (precisa de root)
//!   glove-bateria --daemon    o serviço
//!   glove-bateria --resume-check
//!   glove-bateria --raw       os bytes do EC (diagnóstico)

use std::path::Path;
use std::process::Command;
use std::time::Duration;

const DEV: &str = "/dev/cros_ec";
const OUT: &str = "/run/cros-ec-battery.json";
const PRESUSPEND: &str = "/run/cros-ec-battery.presuspend.json";
const INTERVAL: Duration = Duration::from_secs(5);
/// Leituras seguidas abaixo do limite antes de desligar.
const POWEROFF_READS: u32 = 3;

struct Conf {
    poweroff_pct: f64,
    suspend_poweroff_pct: f64,
    poweroff_mv_per_cell: f64,
}

impl Conf {
    fn load() -> Conf {
        let mut c = Conf { poweroff_pct: 4.0, suspend_poweroff_pct: 15.0, poweroff_mv_per_cell: 3400.0 };
        for f in ["/etc/default/chromebook-linux-fixes", "/etc/default/glove-bateria"] {
            for line in std::fs::read_to_string(f).unwrap_or_default().lines() {
                let Some((k, v)) = line.split_once('=') else { continue };
                let Ok(v) = v.trim().trim_matches('"').parse::<f64>() else { continue };
                match k.trim() {
                    "POWEROFF_PCT" => c.poweroff_pct = v,
                    "SUSPEND_POWEROFF_PCT" => c.suspend_poweroff_pct = v,
                    "POWEROFF_MV_PER_CELL" => c.poweroff_mv_per_cell = v,
                    _ => {}
                }
            }
        }
        c
    }
}

fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or_default()
}

/// Carrega o módulo que cria o `/dev/cros_ec`, se faltar.
fn ensure_dev() {
    if !Path::new(DEV).exists() {
        let _ = Command::new("modprobe").arg("cros_ec_chardev").status();
    }
}

fn write_atomic(v: &serde_json::Value) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let tmp = format!("{OUT}.tmp");
    std::fs::write(&tmp, v.to_string())?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o644))?;
    std::fs::rename(&tmp, OUT)
}

fn f(v: &serde_json::Value, k: &str) -> Option<f64> {
    v.get(k).and_then(|x| x.as_f64())
}

/// Tensão abaixo da qual a bateria está no fim (pelas células estimadas, ~3,8 V cada).
fn poweroff_mv(st: &serde_json::Value, c: &Conf) -> f64 {
    let cells = (f(st, "design_mv").unwrap_or(11400.0) / 3800.0).round().max(1.0);
    cells * c.poweroff_mv_per_cell
}

fn battery_low(st: &serde_json::Value, pct_limit: f64, c: &Conf) -> bool {
    if st.get("state").and_then(|s| s.as_str()) != Some("discharging") {
        return false;
    }
    if f(st, "percent").unwrap_or(100.0) <= pct_limit {
        return true;
    }
    f(st, "voltage_mv").is_some_and(|mv| mv > 0.0 && mv < poweroff_mv(st, c))
}

fn poweroff(reason: &str) {
    println!("Desligando para proteger a bateria: {reason}");
    let _ = Command::new("systemctl").arg("poweroff").status();
}

/// A tampa fechada (o interruptor SW_LID de algum dispositivo de entrada).
fn lid_closed() -> bool {
    use std::os::fd::AsRawFd;
    const SW_LID: u32 = 0;
    // EVIOCGSW(8) = _IOR('E', 0x1b, 8 bytes)
    const EVIOCGSW: u64 = (2 << 30) | (8 << 16) | ((b'E' as u64) << 8) | 0x1B;
    let Ok(rd) = std::fs::read_dir("/sys/class/input") else { return false };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with("event") {
            continue;
        }
        let caps = std::fs::read_to_string(e.path().join("device/capabilities/sw")).unwrap_or_default();
        let last = caps.split_whitespace().last().and_then(|h| u64::from_str_radix(h, 16).ok()).unwrap_or(0);
        if last & (1 << SW_LID) == 0 {
            continue;
        }
        let Ok(file) = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(format!("/dev/input/{name}")) else { continue };
        let mut buf = [0u8; 8];
        // SAFETY: buf tem os 8 bytes que o EVIOCGSW pedido escreve.
        if unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGSW as _, buf.as_mut_ptr()) } >= 0 {
            return buf[0] & (1 << SW_LID) != 0;
        }
    }
    false
}

use std::os::unix::fs::OpenOptionsExt;

/// Com monitor externo (o logind diz "Docked"), a tampa fechada não suspende.
fn docked() -> bool {
    Command::new("busctl")
        .args(["get-property", "org.freedesktop.login1", "/org/freedesktop/login1", "org.freedesktop.login1.Manager", "Docked"])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "b true")
}

fn read_state(rate: Option<f64>) -> std::io::Result<serde_json::Value> {
    let mem = glove_energia::read_ec_memory(Path::new(DEV))?;
    glove_energia::ec_json(&mem, glove_energia::ac_online(), rate, now()).ok_or_else(|| std::io::Error::other("o EC não informou a capacidade da bateria"))
}

fn daemon() {
    let c = Conf::load();
    ensure_dev();
    let mut low = 0;
    // Corrente suavizada (média com peso 0,2), recomeçando quando o estado muda.
    let mut smooth: Option<(String, f64)> = None;
    loop {
        // O kernel enxerga a bateria (outro firmware, outra bateria): nada a fazer.
        if glove_energia::kernel_has_battery() {
            let _ = std::fs::remove_file(OUT);
            std::thread::sleep(Duration::from_secs(60));
            continue;
        }
        let r = glove_energia::read_ec_memory(Path::new(DEV)).and_then(|mem| {
            let first = glove_energia::ec_json(&mem, glove_energia::ac_online(), None, now()).ok_or_else(|| std::io::Error::other("o EC não informou a capacidade da bateria"))?;
            let state = first.get("state").and_then(|s| s.as_str()).unwrap_or("").to_string();
            let rate = f(&first, "rate_ma").unwrap_or(0.0);
            let avg = match &smooth {
                Some((s, prev)) if *s == state && rate > 0.0 => 0.2 * rate + 0.8 * prev,
                Some((s, prev)) if *s == state => *prev,
                _ => rate,
            };
            smooth = Some((state, avg));
            glove_energia::ec_json(&mem, glove_energia::ac_online(), Some(avg), now()).ok_or_else(|| std::io::Error::other("EC"))
        });
        match r {
            Ok(st) => {
                let _ = write_atomic(&st);
                low = if battery_low(&st, c.poweroff_pct, &c) { low + 1 } else { 0 };
                if low >= POWEROFF_READS {
                    poweroff(&format!("{:.0}%, {} mV", f(&st, "percent").unwrap_or(0.0), f(&st, "voltage_mv").unwrap_or(0.0)));
                    low = 0;
                }
            }
            Err(e) => {
                ensure_dev();
                let _ = write_atomic(&serde_json::json!({ "ok": false, "ts": now(), "error": e.to_string() }));
            }
        }
        std::thread::sleep(INTERVAL);
    }
}

/// Depois de acordar (chamado pelo gancho do systemd): desliga com a carga baixa, ou
/// volta a suspender se a tampa continua fechada.
fn resume_check() -> std::io::Result<()> {
    let c = Conf::load();
    let st = read_state(None)?;
    let pct = f(&st, "percent").unwrap_or(0.0);
    let mut msg = format!("Depois de acordar: {pct:.0}% ({})", st.get("state").and_then(|s| s.as_str()).unwrap_or("?"));
    if let Ok(before) = std::fs::read_to_string(PRESUSPEND).map(|t| serde_json::from_str::<serde_json::Value>(&t).unwrap_or_default()) {
        if let (Some(ts), Some(p)) = (f(&before, "ts"), f(&before, "percent")) {
            let hours = (now() - ts) / 3600.0;
            if hours > 0.05 {
                msg += &format!(", gastou {:.1}% em {hours:.1} h", p - pct);
            }
        }
    }
    let closed = lid_closed();
    println!("{msg}, tampa {}", if closed { "fechada" } else { "aberta" });
    if !closed || docked() {
        return Ok(());
    }
    if battery_low(&st, c.suspend_poweroff_pct, &c) {
        poweroff(&format!("{pct:.0}% durante a suspensão"));
    } else {
        println!("Tampa ainda fechada: voltando a suspender");
        let _ = Command::new("systemctl").arg("suspend").status();
    }
    Ok(())
}

fn once() -> std::io::Result<()> {
    let st = read_state(None)?;
    let c = Conf::load();
    let name = match st.get("state").and_then(|s| s.as_str()) {
        Some("charging") => "carregando",
        Some("discharging") => "na bateria",
        Some("full") => "carregada",
        _ => "na tomada, sem carregar",
    };
    println!("Bateria:      {:.0}% ({name})", f(&st, "percent").unwrap_or(0.0));
    println!("Capacidade:   {} / {} mAh (projeto {} mAh, saúde {}%)", st["remaining_mah"], st["full_mah"], st["design_mah"], st["health_percent"]);
    println!("Tensão/corr.: {} mV / {} mA (desliga abaixo de {:.0} mV)", st["voltage_mv"], st["rate_ma"], poweroff_mv(&st, &c));
    println!("Ciclos:       {}", st["cycles"]);
    println!("Fabricante:   {} {}", st["manufacturer"].as_str().unwrap_or(""), st["model"].as_str().unwrap_or(""));
    println!("O kernel vê a bateria: {}", if glove_energia::kernel_has_battery() { "sim" } else { "não (por isso este serviço)" });
    Ok(())
}

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    if arg != "--daemon" {
        ensure_dev();
    }
    let r = match arg.as_str() {
        "--daemon" => {
            daemon();
            Ok(())
        }
        "--resume-check" => resume_check(),
        "--raw" => glove_energia::read_ec_memory(Path::new(DEV)).map(|m| println!("{}", m.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "))),
        _ => once(),
    };
    if let Err(e) = r {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => eprintln!("glove-bateria: sem permissão para abrir {DEV}. Rode com sudo."),
            std::io::ErrorKind::NotFound => eprintln!("glove-bateria: {DEV} não existe (não é um Chromebook, ou falta o módulo cros_ec_chardev)."),
            _ => eprintln!("glove-bateria: {e}"),
        }
        std::process::exit(1);
    }
}
