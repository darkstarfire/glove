//! A instalação: uma lista de passos (comandos de root) executada numa thread.
//!
//! 1. Partições: o disco inteiro (GPT: EFI de 512 MiB + o sistema; sem UEFI, a
//!    partição de 1 MiB do GRUB + o sistema) ou o maior espaço livre (ao lado de
//!    outro sistema, reaproveitando a partição EFI dele).
//! 2. Formata (vfat e ext4) e monta em /mnt/glove.
//! 3. Copia o sistema do Glove ao vivo (`unsquashfs` da imagem do pendrive).
//! 4. Configura: fstab, nome do computador, idioma, fuso, teclado, a conta (no
//!    grupo sudo) e tira o que é só do sistema ao vivo (login automático,
//!    o instalador no início, os pacotes live-*).
//! 5. Carregador de boot (GRUB, UEFI ou BIOS), initramfs e desmonta.
//!
//! No modo simulação (`--simular`) nada é executado: cada comando vai para o
//! registro, para conferir o que seria feito.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glove_config::tr;

use crate::discos::{part_path, Disco};

pub const ALVO: &str = "/mnt/glove";
const IMAGEM: &str = "/run/live/medium/live/filesystem.squashfs";
/// Tipo GPT da partição de boot do GRUB (BIOS).
const BIOS_GRUB: &str = "21686148-6449-6E6F-744E-656564454649";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Modo {
    /// Apaga o disco inteiro.
    Inteiro,
    /// Usa o maior espaço livre (outro sistema continua).
    Livre,
}

#[derive(Clone, Debug)]
pub struct Escolhas {
    pub idioma: String,
    pub layout: String,
    pub variante: String,
    pub fuso: String,
    pub disco: Disco,
    pub modo: Modo,
    pub nome: String,
    pub usuario: String,
    pub senha: String,
    pub computador: String,
    pub uefi: bool,
}

#[derive(Clone, Debug)]
pub enum Acao {
    /// Comando de root; `entrada` vai para a entrada padrão; `opcional` não para se falhar.
    Rodar { args: Vec<String>, entrada: Option<String>, opcional: bool, secreto: bool },
    /// Grava um arquivo (como root).
    Escrever { caminho: String, conteudo: String },
    /// O fstab com os UUIDs das partições (lidos depois de formatar).
    Fstab { raiz: String, esp: Option<String> },
    /// Copia o sistema (com o andamento).
    Copiar,
}

pub struct Passo {
    pub nome: String,
    /// Quanto do total este passo vale (para a barra).
    pub peso: f32,
    pub acoes: Vec<Acao>,
}

fn rodar(args: &[&str]) -> Acao {
    Acao::Rodar { args: args.iter().map(|s| s.to_string()).collect(), entrada: None, opcional: false, secreto: false }
}
fn opcional(args: &[&str]) -> Acao {
    Acao::Rodar { args: args.iter().map(|s| s.to_string()).collect(), entrada: None, opcional: true, secreto: false }
}
fn com_entrada(args: &[&str], entrada: String) -> Acao {
    Acao::Rodar { args: args.iter().map(|s| s.to_string()).collect(), entrada: Some(entrada), opcional: false, secreto: false }
}
fn chroot(args: &[&str]) -> Acao {
    let mut v = vec!["chroot", ALVO];
    v.extend_from_slice(args);
    rodar(&v)
}
fn chroot_opcional(args: &[&str]) -> Acao {
    let mut v = vec!["chroot", ALVO];
    v.extend_from_slice(args);
    opcional(&v)
}
fn escrever(caminho: &str, conteudo: String) -> Acao {
    Acao::Escrever { caminho: format!("{ALVO}{caminho}"), conteudo }
}

/// As partições que a instalação vai usar: (raiz, EFI) e os passos para criá-las.
fn particoes(e: &Escolhas) -> (String, Option<String>, Vec<Acao>) {
    let d = &e.disco;
    let mut v = Vec::new();
    match e.modo {
        Modo::Inteiro => {
            v.push(opcional(&["swapoff", "-a"]));
            for p in &d.particoes {
                v.push(opcional(&["umount", &p.path]));
            }
            v.push(rodar(&["wipefs", "-a", &d.path]));
            let tabela = if e.uefi { "label: gpt\n,512MiB,U\n,,L\n".to_string() } else { format!("label: gpt\n,1MiB,{BIOS_GRUB}\n,,L\n") };
            v.push(com_entrada(&["sfdisk", "--wipe", "always", &d.path], tabela));
            v.push(opcional(&["udevadm", "settle"]));
            let (p1, p2) = (part_path(&d.path, 1), part_path(&d.path, 2));
            if e.uefi {
                v.push(rodar(&["mkfs.vfat", "-F", "32", "-n", "EFI", &p1]));
            }
            v.push(rodar(&["mkfs.ext4", "-F", "-L", "Glove", &p2]));
            (p2, e.uefi.then_some(p1), v)
        }
        Modo::Livre => {
            let (inicio, setores) = d.livre.unwrap_or((2048, 0));
            let n = d.particoes.iter().map(|p| p.numero).max().unwrap_or(0) + 1;
            v.push(com_entrada(&["sfdisk", "--append", &d.path], format!("start={inicio}, size={setores}, type=L\n")));
            v.push(opcional(&["udevadm", "settle"]));
            let raiz = part_path(&d.path, n);
            v.push(rodar(&["mkfs.ext4", "-F", "-L", "Glove", &raiz]));
            // Reaproveita a partição EFI que já existe (a do outro sistema).
            (raiz, d.esp().map(|p| p.path.clone()), v)
        }
    }
}

pub fn plano(e: &Escolhas) -> Vec<Passo> {
    let (raiz, esp, part) = particoes(e);
    let mut passos = vec![Passo { nome: tr!("Preparando o disco").into(), peso: 0.06, acoes: part }];

    let mut monta = vec![rodar(&["mkdir", "-p", ALVO]), rodar(&["mount", &raiz, ALVO])];
    if let Some(esp) = &esp {
        monta.push(rodar(&["mkdir", "-p", &format!("{ALVO}/boot/efi")]));
        monta.push(rodar(&["mount", esp, &format!("{ALVO}/boot/efi")]));
    }
    passos.push(Passo { nome: tr!("Montando").into(), peso: 0.01, acoes: monta });
    passos.push(Passo { nome: tr!("Copiando o sistema").into(), peso: 0.7, acoes: vec![Acao::Copiar] });

    // Configuração do sistema novo.
    let u = &e.usuario;
    let mut cfg = vec![
        Acao::Fstab { raiz: raiz.clone(), esp: esp.clone() },
        escrever("/etc/hostname", format!("{}\n", e.computador)),
        escrever("/etc/hosts", format!("127.0.0.1\tlocalhost\n127.0.1.1\t{}\n::1\t\tlocalhost ip6-localhost ip6-loopback\n", e.computador)),
        escrever("/etc/default/locale", format!("LANG={}\n", e.idioma)),
        rodar(&["sed", "-i", &format!("s/^# *{}/{}/", e.idioma.replace('.', "\\."), e.idioma), &format!("{ALVO}/etc/locale.gen")]),
        chroot_opcional(&["locale-gen"]),
        rodar(&["ln", "-sf", &format!("/usr/share/zoneinfo/{}", e.fuso), &format!("{ALVO}/etc/localtime")]),
        escrever("/etc/timezone", format!("{}\n", e.fuso)),
        escrever("/etc/default/keyboard", format!("XKBMODEL=\"pc105\"\nXKBLAYOUT=\"{}\"\nXKBVARIANT=\"{}\"\nXKBOPTIONS=\"\"\nBACKSPACE=\"guess\"\n", e.layout, e.variante)),
        // O que é só do sistema ao vivo.
        rodar(&["rm", "-f", &format!("{ALVO}/etc/glove/autologin"), &format!("{ALVO}/etc/xdg/autostart/glove-instalador.desktop"), &format!("{ALVO}/etc/skel/.config/glove/config.toml"), &format!("{ALVO}/usr/share/applications/glove-instalador.desktop")]),
        chroot_opcional(&["dpkg", "--purge", "live-boot", "live-boot-initramfs-tools", "live-config", "live-config-systemd", "live-tools"]),
    ];
    // A conta: no grupo sudo (administrador) e nos de áudio, vídeo e rede que existirem.
    cfg.push(chroot(&["useradd", "-m", "-s", "/bin/bash", "-c", &e.nome, "-G", "sudo", u]));
    for g in ["audio", "video", "plugdev", "netdev", "lpadmin", "bluetooth", "input", "render"] {
        cfg.push(chroot_opcional(&["usermod", "-aG", g, u]));
    }
    cfg.push(Acao::Rodar { args: vec!["chroot".into(), ALVO.into(), "chpasswd".into()], entrada: Some(format!("{u}:{}\n", e.senha)), opcional: false, secreto: true });
    // As preferências do Glove já com o teclado e o idioma (sem o assistente de boas-vindas).
    let home = format!("/home/{u}");
    cfg.push(rodar(&["mkdir", "-p", &format!("{ALVO}{home}/.config/glove")]));
    cfg.push(escrever(&format!("{home}/.config/glove/config.toml"), format!("welcome_done = true\n\n[keyboard]\nlayout = \"{}\"\nvariant = \"{}\"\n", e.layout, e.variante)));
    cfg.push(escrever(&format!("{home}/.config/glove/ambiente.conf"), format!("LANG={}\n", e.idioma)));
    cfg.push(chroot(&["chown", "-R", &format!("{u}:{u}"), &format!("{home}/.config")]));
    passos.push(Passo { nome: tr!("Configurando o sistema").into(), peso: 0.08, acoes: cfg });

    // Carregador de boot.
    let mut boot = Vec::new();
    for d in ["dev", "proc", "sys", "run"] {
        boot.push(rodar(&["mount", "--rbind", &format!("/{d}"), &format!("{ALVO}/{d}")]));
        boot.push(opcional(&["mount", "--make-rslave", &format!("{ALVO}/{d}")]));
    }
    boot.push(escrever("/etc/default/grub", "GRUB_DEFAULT=0\nGRUB_TIMEOUT=3\nGRUB_DISTRIBUTOR=\"Glove OS\"\nGRUB_CMDLINE_LINUX_DEFAULT=\"quiet splash\"\nGRUB_CMDLINE_LINUX=\"\"\nGRUB_DISABLE_OS_PROBER=false\n".into()));
    boot.push(chroot(&["update-initramfs", "-u", "-k", "all"]));
    if e.uefi {
        boot.push(chroot(&["grub-install", "--target=x86_64-efi", "--efi-directory=/boot/efi", "--bootloader-id=Glove", "--recheck"]));
        if e.modo == Modo::Inteiro {
            // Cópia no caminho padrão (computadores e máquinas virtuais que não guardam a entrada).
            boot.push(chroot_opcional(&["grub-install", "--target=x86_64-efi", "--efi-directory=/boot/efi", "--removable", "--no-nvram"]));
        }
    } else {
        boot.push(chroot(&["grub-install", "--target=i386-pc", "--recheck", &e.disco.path]));
    }
    boot.push(chroot(&["update-grub"]));
    boot.push(opcional(&["sync"]));
    boot.push(opcional(&["umount", "-R", ALVO]));
    passos.push(Passo { nome: tr!("Instalando o carregador de boot").into(), peso: 0.15, acoes: boot });
    passos
}

/// Andamento lido pela janela.
#[derive(Default)]
pub struct Andamento {
    pub passo: usize,
    pub fracao: f32,
    pub texto: String,
    pub registro: Vec<String>,
    pub fim: Option<Result<(), String>>,
}

pub type Shared = Arc<Mutex<Andamento>>;

fn log(s: &Shared, l: &str) {
    let mut a = s.lock().unwrap();
    a.registro.push(l.to_string());
    if a.registro.len() > 2000 {
        a.registro.drain(..500);
    }
}

fn sou_root() -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").is_ok_and(|m| m.uid() == 0)
}

/// Monta o comando como root: direto, ou pelo sudo (sem senha no Glove ao vivo; com a senha, se dada).
fn comando(args: &[String], senha: &Option<String>) -> Command {
    if sou_root() {
        let mut c = Command::new(&args[0]);
        c.args(&args[1..]);
        c
    } else {
        let mut c = Command::new("sudo");
        if senha.is_some() {
            c.args(["-S", "-p", ""]);
        } else {
            c.arg("-n");
        }
        c.args(args);
        c
    }
}

fn executa(args: &[String], entrada: Option<&str>, senha: &Option<String>, s: &Shared) -> Result<(), String> {
    let mut c = comando(args, senha);
    c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).env("LC_ALL", "C").env("DEBIAN_FRONTEND", "noninteractive");
    let mut child = c.spawn().map_err(|e| format!("{}: {e}", args[0]))?;
    if let Some(mut i) = child.stdin.take() {
        if let Some(p) = senha.as_ref().filter(|_| !sou_root()) {
            let _ = writeln!(i, "{p}");
        }
        if let Some(t) = entrada {
            let _ = i.write_all(t.as_bytes());
        }
    }
    let err = child.stderr.take();
    let s2 = s.clone();
    let h = std::thread::spawn(move || {
        let mut last = String::new();
        if let Some(e) = err {
            for l in BufReader::new(e).lines().map_while(Result::ok) {
                log(&s2, &l);
                last = l;
            }
        }
        last
    });
    if let Some(o) = child.stdout.take() {
        for l in BufReader::new(o).lines().map_while(Result::ok) {
            log(s, &l);
        }
    }
    let last = h.join().unwrap_or_default();
    match child.wait() {
        Ok(st) if st.success() => Ok(()),
        _ => Err(if last.is_empty() { tr!("{} falhou", args.join(" ")) } else { format!("{}: {last}", args[0]) }),
    }
}

/// A cópia do sistema, com o andamento do unsquashfs ("12%").
fn copiar(senha: &Option<String>, s: &Shared, simular: bool) -> Result<(), String> {
    if simular {
        log(s, &format!("$ unsquashfs -f -percentage -d {ALVO} {IMAGEM}"));
        for k in 0..=20 {
            s.lock().unwrap().fracao = k as f32 / 20.0;
            std::thread::sleep(Duration::from_millis(120));
        }
        return Ok(());
    }
    let args: Vec<String> = ["unsquashfs", "-f", "-percentage", "-d", ALVO, IMAGEM].iter().map(|x| x.to_string()).collect();
    let mut c = comando(&args, senha);
    c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().map_err(|e| e.to_string())?;
    if let (Some(p), Some(mut i)) = (senha.as_ref().filter(|_| !sou_root()), child.stdin.take()) {
        let _ = writeln!(i, "{p}");
    }
    if let Some(o) = child.stdout.take() {
        for l in BufReader::new(o).split(b'\n').map_while(Result::ok) {
            let l = String::from_utf8_lossy(&l);
            if let Some(p) = l.trim().strip_suffix('%').and_then(|v| v.trim().parse::<f32>().ok()) {
                s.lock().unwrap().fracao = p / 100.0;
            }
        }
    }
    match child.wait() {
        Ok(st) if st.success() => Ok(()),
        _ => Err(tr!("A cópia do sistema falhou.").into()),
    }
}

fn uuid(part: &str, senha: &Option<String>) -> Result<String, String> {
    let args: Vec<String> = ["blkid", "-s", "UUID", "-o", "value", part].iter().map(|x| x.to_string()).collect();
    let out = comando(&args, senha).stdin(Stdio::null()).output().map_err(|e| e.to_string())?;
    let u = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if u.is_empty() { Err(tr!("Sem UUID para {}", part)) } else { Ok(u) }
}

/// Roda o plano numa thread. `senha`: a de administrador, se o sudo pedir.
pub fn instalar(passos: Vec<Passo>, senha: Option<String>, simular: bool) -> Shared {
    let s: Shared = Arc::new(Mutex::new(Andamento::default()));
    let st = s.clone();
    std::thread::spawn(move || {
        let r = (|| -> Result<(), String> {
            for (i, p) in passos.iter().enumerate() {
                {
                    let mut a = st.lock().unwrap();
                    a.passo = i;
                    a.fracao = 0.0;
                    a.texto = p.nome.clone();
                }
                log(&st, &format!("== {}", p.nome));
                let n = p.acoes.len().max(1);
                for (k, a) in p.acoes.iter().enumerate() {
                    match a {
                        Acao::Rodar { args, entrada, opcional, secreto } => {
                            let shown = if *secreto { format!("$ {} (…)", args.join(" ")) } else { format!("$ {}", args.join(" ")) };
                            log(&st, &shown);
                            if simular {
                                std::thread::sleep(Duration::from_millis(60));
                            } else if let Err(e) = executa(args, entrada.as_deref(), &senha, &st) {
                                if *opcional {
                                    log(&st, &format!("(ignorado) {e}"));
                                } else {
                                    return Err(e);
                                }
                            }
                        }
                        Acao::Escrever { caminho, conteudo } => {
                            log(&st, &format!("> {caminho}"));
                            if !simular {
                                executa(&["tee".to_string(), caminho.clone()], Some(conteudo), &senha, &Arc::new(Mutex::new(Andamento::default())))?;
                            }
                        }
                        Acao::Fstab { raiz, esp } => {
                            log(&st, &format!("> {ALVO}/etc/fstab"));
                            if !simular {
                                let mut t = format!("UUID={} / ext4 errors=remount-ro 0 1\n", uuid(raiz, &senha)?);
                                if let Some(e) = esp {
                                    t.push_str(&format!("UUID={} /boot/efi vfat umask=0077 0 1\n", uuid(e, &senha)?));
                                }
                                executa(&["tee".to_string(), format!("{ALVO}/etc/fstab")], Some(&t), &senha, &Arc::new(Mutex::new(Andamento::default())))?;
                            }
                        }
                        Acao::Copiar => copiar(&senha, &st, simular)?,
                    }
                    if !matches!(a, Acao::Copiar) {
                        st.lock().unwrap().fracao = (k + 1) as f32 / n as f32;
                    }
                }
            }
            Ok(())
        })();
        if let Err(e) = &r {
            log(&st, &format!("ERRO: {e}"));
            // Tenta desmontar o que ficou montado.
            if !simular {
                let _ = executa(&["umount".into(), "-R".into(), ALVO.into()], None, &senha, &st);
            }
        }
        st.lock().unwrap().fim = Some(r);
    });
    s
}

/// O sudo funciona sem senha (o Glove ao vivo)?
pub fn sudo_sem_senha() -> bool {
    sou_root() || Command::new("sudo").args(["-n", "true"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Estamos no Glove ao vivo (a imagem do sistema está no pendrive)?
pub fn ao_vivo() -> bool {
    std::path::Path::new(IMAGEM).exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discos::{Disco, Particao};

    fn esc(modo: Modo, uefi: bool) -> Escolhas {
        let disco = Disco {
            path: "/dev/nvme0n1".into(),
            nome: "Teste".into(),
            tamanho: 500_000_000_000,
            removivel: false,
            tabela: "gpt".into(),
            setor: 512,
            particoes: vec![Particao { path: "/dev/nvme0n1p1".into(), numero: 1, tamanho: 512 << 20, fs: "vfat".into(), rotulo: String::new(), esp: true, inicio: 2048 }],
            livre: Some((1050624, 400_000_000)),
            ao_vivo: false,
        };
        Escolhas { idioma: "pt_BR.UTF-8".into(), layout: "br".into(), variante: String::new(), fuso: "America/Sao_Paulo".into(), disco, modo, nome: "Ana".into(), usuario: "ana".into(), senha: "segredo".into(), computador: "pc-ana".into(), uefi }
    }

    fn cmds(p: &[Passo]) -> Vec<String> {
        p.iter().flat_map(|x| x.acoes.iter()).filter_map(|a| if let Acao::Rodar { args, .. } = a { Some(args.join(" ")) } else { None }).collect()
    }

    #[test]
    fn disco_inteiro_uefi() {
        let p = plano(&esc(Modo::Inteiro, true));
        let c = cmds(&p);
        assert!(c.iter().any(|x| x == "mkfs.vfat -F 32 -n EFI /dev/nvme0n1p1"));
        assert!(c.iter().any(|x| x == "mkfs.ext4 -F -L Glove /dev/nvme0n1p2"));
        assert!(c.iter().any(|x| x.contains("--target=x86_64-efi")));
        // A senha nunca aparece nos argumentos.
        assert!(!c.iter().any(|x| x.contains("segredo")));
    }

    #[test]
    fn espaco_livre_mantem_o_outro() {
        let p = plano(&esc(Modo::Livre, true));
        let c = cmds(&p);
        assert!(!c.iter().any(|x| x.starts_with("wipefs") || x.contains("mkfs.vfat")));
        assert!(c.iter().any(|x| x == "mkfs.ext4 -F -L Glove /dev/nvme0n1p2"));
        assert!(c.iter().any(|x| x == "mount /dev/nvme0n1p1 /mnt/glove/boot/efi"));
    }

    #[test]
    fn bios() {
        let c = cmds(&plano(&esc(Modo::Inteiro, false)));
        assert!(c.iter().any(|x| x == "chroot /mnt/glove grub-install --target=i386-pc --recheck /dev/nvme0n1"));
        assert!(!c.iter().any(|x| x.contains("mkfs.vfat")));
    }
}
