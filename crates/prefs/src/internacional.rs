//! Painel Internacional: idioma dos apps e formatos (data, hora, números e
//! moeda). Fica em ~/.config/glove/ambiente.conf (LANG e LC_*), que a sessão
//! do Glove lê ao entrar; vale para os apps abertos depois disso.

use glove_config::tr;
use std::path::PathBuf;

use glove_ui::Ui;

/// (locale, nome do idioma, nome da região).
pub const LOCALES: [(&str, &str, &str); 10] = [
    ("pt_BR", "Português (Brasil)", "Brasil"),
    ("pt_PT", "Português (Portugal)", "Portugal"),
    ("en_US", "Inglês (EUA)", "Estados Unidos"),
    ("en_GB", "Inglês (Reino Unido)", "Reino Unido"),
    ("es_ES", "Espanhol (Espanha)", "Espanha"),
    ("es_AR", "Espanhol (Argentina)", "Argentina"),
    ("fr_FR", "Francês", "França"),
    ("de_DE", "Alemão", "Alemanha"),
    ("it_IT", "Italiano", "Itália"),
    ("ja_JP", "Japonês", "Japão"),
];

/// Gera um idioma no sistema (como administrador): no Debian, Ubuntu e Arch, habilita a
/// linha em /etc/locale.gen e roda o locale-gen; no Fedora, instala o glibc-langpack.
/// `$1` = o locale (ll_CC).
const INSTALAR: &str = r#"set -e
l="$1"
lang=${l%%_*}
if [ -f /etc/locale.gen ] && command -v locale-gen >/dev/null 2>&1; then
    linha="$l.UTF-8 UTF-8"
    if grep -q "^[# ]*$linha *\$" /etc/locale.gen; then
        sed -i "0,/^[# ]*$linha *\$/s//$linha/" /etc/locale.gen
    else
        echo "$linha" >> /etc/locale.gen
    fi
    if [ -f /etc/arch-release ]; then locale-gen; else locale-gen --keep-existing; fi
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y "glibc-langpack-$lang"
else
    echo "Este sistema não tem o locale-gen nem o dnf." >&2
    exit 1
fi
"#;

/// Como administrador (`glove-prefs --install-language ll_CC`): só os idiomas da lista.
pub fn instalar_como_admin(loc: &str) -> Result<(), String> {
    if !LOCALES.iter().any(|l| l.0 == loc) {
        return Err(tr!("Idioma desconhecido: {}", loc));
    }
    let ok = std::process::Command::new("sh").args(["-c", INSTALAR, "sh", loc]).status().map_err(|e| e.to_string())?;
    if ok.success() { Ok(()) } else { Err(tr!("Não deu para instalar o idioma {}.", loc)) }
}

fn file() -> PathBuf {
    glove_config::dir().join("ambiente.conf")
}

/// (LANG, região dos formatos) gravados; vazio = padrão do sistema.
pub fn load() -> (String, String) {
    let t = std::fs::read_to_string(file()).unwrap_or_default();
    let get = |k: &str| t.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(|v| v.trim_matches('"').split('.').next().unwrap_or("").to_string()).unwrap_or_default();
    (get("LANG"), get("LC_TIME"))
}

fn save(lang: &str, region: &str) -> std::io::Result<()> {
    let mut out = String::from("# Idioma e formatos do Glove (painel Internacional). A sessão lê ao entrar.\n");
    if !lang.is_empty() {
        out.push_str(&format!("LANG={lang}.UTF-8\n"));
    }
    if !region.is_empty() {
        for k in ["LC_TIME", "LC_NUMERIC", "LC_MONETARY", "LC_PAPER", "LC_MEASUREMENT"] {
            out.push_str(&format!("{k}={region}.UTF-8\n"));
        }
    }
    std::fs::create_dir_all(glove_config::dir())?;
    std::fs::write(file(), out)
}

/// Locales instalados no sistema (pt_BR, en_US…), pelo `locale -a`.
fn installed() -> Vec<String> {
    std::process::Command::new("locale")
        .arg("-a")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|l| l.split('.').next().unwrap_or("").to_string()).collect())
        .unwrap_or_default()
}

/// Agrupa os milhares e põe a vírgula decimal da região: 1234567.891 → "1.234.567,89".
pub fn number(region: &str, v: f64) -> String {
    let (thousands, decimal) = match region {
        "en_US" | "en_GB" | "ja_JP" => (",", "."),
        "fr_FR" | "pt_PT" => ("\u{202f}", ","),
        "de_DE" | "it_IT" | "es_ES" | "es_AR" | "pt_BR" => (".", ","),
        _ => (".", ","),
    };
    let cents = (v * 100.0).round() as i64;
    let (int, frac) = (cents / 100, (cents % 100).abs());
    let digits = int.abs().to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push_str(thousands);
        }
        grouped.push(c);
    }
    format!("{}{grouped}{decimal}{frac:02}", if int < 0 { "-" } else { "" })
}

pub fn money(region: &str, v: f64) -> String {
    let n = number(region, v);
    match region {
        "pt_BR" => format!("R$ {n}"),
        "en_US" => format!("${n}"),
        "en_GB" => format!("£{n}"),
        "es_AR" => format!("$ {n}"),
        "ja_JP" => format!("¥{}", n.split('.').next().unwrap_or("")),
        _ => format!("{n} €"),
    }
}

/// Data curta e hora de exemplo (5 de novembro de 2026, 14:30).
pub fn date_time(region: &str, (y, m, d, h, mi): (i32, u32, u32, u32, u32)) -> (String, String) {
    let date = match region {
        "en_US" => format!("{m}/{d}/{y}"),
        "ja_JP" => format!("{y}/{m:02}/{d:02}"),
        "de_DE" => format!("{d:02}.{m:02}.{y}"),
        _ => format!("{d:02}/{m:02}/{y}"),
    };
    let time = match region {
        "en_US" | "en_GB" => {
            let h12 = if h % 12 == 0 { 12 } else { h % 12 };
            format!("{h12}:{mi:02} {}", if h < 12 { "AM" } else { "PM" })
        }
        _ => format!("{h:02}:{mi:02}"),
    };
    (date, time)
}

#[derive(Default)]
pub struct Internacional {
    pub tab: usize,
    lang: String,
    region: String,
    installed: Vec<String>,
    loaded: bool,
    message: Option<String>,
    /// Instalando um idioma no sistema: o pedido de senha e o trabalho.
    pub prompt: Option<(Vec<String>, crate::admin::Prompt)>,
    job: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
}

impl Internacional {
    pub fn refresh(&mut self) {
        let (l, r) = load();
        self.lang = l;
        self.region = r;
        self.installed = installed();
        self.loaded = true;
    }

    fn locale_names(&self, region: bool) -> Vec<String> {
        LOCALES
            .iter()
            .map(|(id, lang, reg)| {
                let base = if region { tr(reg).to_string() } else { tr(lang).to_string() };
                if self.installed.iter().any(|i| i == id) { base } else { tr!("{} (não instalado)", base) }
            })
            .collect()
    }

    /// Abas Idioma e Formatos (a do Menu de Entrada é desenhada pelo painel).
    pub fn idioma(&mut self, ui: &mut Ui) {
        if !self.loaded {
            self.refresh();
        }
        ui.small(tr!("Idioma do Glove e dos outros apps:"));
        ui.space(6.0);
        let names = self.locale_names(false);
        let mut sel = LOCALES.iter().position(|l| l.0 == self.lang).or(Some(0));
        if ui.list(&mut sel, &names, ui.available_width(), 200.0 + crate::grow()) {
            if let Some(i) = sel {
                self.lang = LOCALES[i].0.to_string();
                self.write();
            }
        }
        ui.space(6.0);
        // O escolhido ainda não está no sistema: dá para instalar (pede a senha).
        let escolhido = LOCALES.iter().position(|l| l.0 == self.lang).unwrap_or(0);
        let id = LOCALES[escolhido].0;
        if !self.installed.iter().any(|i| i == id) {
            ui.set_enabled(self.job.is_none());
            if ui.button(tr!("Instalar Este Idioma…")) {
                self.instalar(id);
            }
            ui.set_enabled(true);
            ui.space(4.0);
        }
        self.andamento(ui);
        ui.small(tr!("Idiomas marcados “não instalado” precisam ser instalados no sistema (pede a senha de administrador)."));
        ui.small(tr!("Vale para os apps abertos depois da próxima entrada no Glove."));
        if let Some(m) = &self.message {
            let m = m.clone();
            ui.small(&m);
        }
    }

    pub fn formatos(&mut self, ui: &mut Ui) {
        if !self.loaded {
            self.refresh();
        }
        let names = self.locale_names(true);
        let reg = if self.region.is_empty() { if self.lang.is_empty() { "pt_BR".to_string() } else { self.lang.clone() } } else { self.region.clone() };
        let mut cur = LOCALES.iter().position(|l| l.0 == reg).unwrap_or(0);
        ui.form_row(tr!("Região"), 120.0, |ui| {
            if ui.popup(&mut cur, &names, 240.0) {
                self.region = LOCALES[cur].0.to_string();
                self.write();
            }
        });
        ui.space(10.0);
        let r = LOCALES[cur].0;
        let (y, mo, d, _, h, mi, _) = glove_ui::local_time();
        let (date, time) = date_time(r, (y, mo, d, h, mi));
        ui.form_row(tr!("Data"), 120.0, |ui| ui.label(&date));
        ui.form_row(tr!("Hora"), 120.0, |ui| ui.label(&time));
        ui.form_row(tr!("Números"), 120.0, |ui| ui.label(&number(r, 1_234_567.89)));
        ui.form_row(tr!("Moeda"), 120.0, |ui| ui.label(&money(r, 1_234.56)));
        ui.form_row(tr!("Medidas"), 120.0, |ui| ui.label(if matches!(r, "en_US") { tr!("Americanas (polegadas, libras)") } else { tr!("Métricas (centímetros, quilos)") }));
        ui.space(8.0);
        ui.small(tr!("Exemplos de como os apps vão mostrar. Vale depois da próxima entrada."));
    }

    /// Instala o idioma `loc` no sistema pelo próprio glove-prefs, como administrador.
    fn instalar(&mut self, loc: &str) {
        self.message = None;
        let exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("glove-prefs")))
            .filter(|p| p.exists())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "glove-prefs".into());
        let args = vec![exe, "--install-language".into(), loc.to_string()];
        match crate::admin::sudo_state() {
            crate::admin::Sudo::Free => self.job = Some(crate::admin::run(args, None)),
            crate::admin::Sudo::No => self.message = Some(tr!("Instalar um idioma pede uma conta de administrador.").into()),
            crate::admin::Sudo::Password => self.prompt = Some((args, crate::admin::Prompt::new(tr!("Instalar um idioma no sistema.").into()))),
        }
    }

    /// A janela de senha (chamada pelo painel).
    pub fn prompt_sheet(&mut self, ui: &mut Ui) {
        if let Some((args, mut p)) = self.prompt.take() {
            match p.sheet(ui) {
                crate::admin::Answer::Waiting => self.prompt = Some((args, p)),
                crate::admin::Answer::Cancel => {}
                crate::admin::Answer::Password(pw) => self.job = Some(crate::admin::run(args, Some(pw))),
            }
        }
    }

    /// O fim da instalação: confere de verdade (`locale -a`) e avisa.
    fn andamento(&mut self, ui: &mut Ui) {
        let Some(rx) = &self.job else { return };
        match rx.try_recv() {
            Ok(r) => {
                self.job = None;
                self.installed = installed();
                let id = self.lang.clone();
                self.message = Some(match r {
                    Ok(()) if self.installed.iter().any(|i| *i == id) => tr!("Pronto: o idioma foi instalado. Vale depois da próxima entrada.").into(),
                    Ok(()) => tr!("O sistema terminou, mas o idioma ainda não aparece instalado.").into(),
                    Err(e) => tr!("Não deu certo: {}", e),
                });
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ui.small(tr!("Instalando o idioma… (pode levar um minuto)"));
                ui.wake_after(std::time::Duration::from_millis(250));
            }
            Err(_) => self.job = None,
        }
    }

    fn write(&mut self) {
        self.message = save(&self.lang, &self.region).err().map(|e| tr!("Não foi possível salvar: {}", e));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatos_por_regiao() {
        assert_eq!(number("pt_BR", 1_234_567.891), "1.234.567,89");
        assert_eq!(number("en_US", 1_234_567.891), "1,234,567.89");
        assert_eq!(number("pt_BR", 12.5), "12,50");
        assert_eq!(money("pt_BR", 1_234.56), "R$ 1.234,56");
        assert_eq!(money("de_DE", 1_234.56), "1.234,56 €");
        assert_eq!(date_time("en_US", (2026, 11, 5, 14, 30)), ("11/5/2026".to_string(), "2:30 PM".to_string()));
        assert_eq!(date_time("pt_BR", (2026, 11, 5, 9, 5)), ("05/11/2026".to_string(), "09:05".to_string()));
    }
}
