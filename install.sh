#!/bin/sh
# Glove installer: builds (if needed) and asks what to install.
#   ./install.sh           asks step by step
#   ./install.sh --remove  takes Glove off the login screen (and restores the previous one)
# The steps that change the system ask for the password (sudo) only when chosen.
# This is the development mode (the session uses the project's own programs). To
# install system-wide or build .deb/.rpm/Slackware/Arch packages: ./glove-setup-tool.sh
set -e
cd "$(dirname "$0")"
root_dir=$(pwd)

# Message language: Portuguese if the system locale is Portuguese, English otherwise.
case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in pt*) lang=pt ;; *) lang=en ;; esac
# t "texto em português" "text in English"
t() { if [ "$lang" = pt ]; then printf '%s' "$1"; else printf '%s' "$2"; fi; }
say() { printf '%s\n' "$(t "$1" "$2")"; }
has() { command -v "$1" >/dev/null 2>&1; }

ask() {
    # ask "question" y|n (the default answer)  ->  0 = yes
    if [ "$lang" = pt ]; then yes_key=S; else yes_key=Y; fi
    if [ "$2" = y ]; then op="[$yes_key/n]"; else op="[$(echo $yes_key | tr SY sy)/N]"; fi
    printf '%s %s ' "$1" "$op"
    read -r r || r=""
    [ -z "$r" ] && r=$2
    case "$r" in y|Y|yes|Yes|s|S|sim|Sim) return 0 ;; *) return 1 ;; esac
}

if [ "$(id -u)" = 0 ]; then
    say "Rode sem sudo: o instalador pede a senha só nos passos que precisam." \
        "Run without sudo: the installer asks for the password only in the steps that need it." >&2
    exit 1
fi

case "${1:-}" in
    --remove|--remover)
        if ask "$(t 'Tirar o Glove da lista de sessões?' 'Remove Glove from the session list?')" y; then
            sudo ./data/install-session.sh --remove
        fi
        if [ -f /etc/greetd/config.toml.antes-do-glove ] && ask "$(t 'Devolver a tela de login que havia antes do Glove?' 'Restore the login screen that was there before Glove?')" y; then
            sudo ./data/install-login.sh --remove
        fi
        exit 0
        ;;
esac

echo "== Glove =="
if [ ! -x target/release/glove ] || [ ! -x target/release/glove-login ]; then
    if ! has cargo; then
        say "Precisa do Rust (cargo) para compilar: https://rustup.rs" \
            "Rust (cargo) is needed to build: https://rustup.rs" >&2
        exit 1
    fi
    if ask "$(t 'O Glove ainda não foi compilado. Compilar agora (alguns minutos)?' 'Glove has not been built yet. Build now (a few minutes)?')" y; then
        ./build.sh
    else
        exit 1
    fi
elif ask "$(t 'Recompilar antes de instalar (pega as mudanças mais novas)?' 'Rebuild before installing (picks up the latest changes)?')" n; then
    ./build.sh
fi

# Outside programs the Glove apps use (only the missing ones are offered).
missing=""
for pair in mpv:mpv pdftoppm:poppler-utils Xwayland:xwayland pw-play:pipewire-bin wpctl:wireplumber xdg-mime:xdg-utils update-desktop-database:desktop-file-utils fc-cache:fontconfig dbus-update-activation-environment:dbus-bin; do
    cmd=${pair%%:*}
    package=${pair#*:}
    has "$cmd" || missing="$missing $package"
done
if [ -n "$missing" ]; then
    echo
    echo "$(t 'Faltam programas que alguns apps usam:' 'Programs some apps use are missing:')$missing"
    say "  (mpv: Vídeo; poppler-utils: PDF no Visualizador; xwayland: apps X11;" \
        "  (mpv: Video; poppler-utils: PDF in the Viewer; xwayland: X11 apps;"
    say "   pipewire/wireplumber: som; xdg-utils e desktop-file-utils: \"Abrir com\")" \
        "   pipewire/wireplumber: sound; xdg-utils and desktop-file-utils: \"Open with\")"
    if has apt-get && ask "$(t 'Instalar pelo apt?' 'Install with apt?')" y; then
        # shellcheck disable=SC2086
        sudo apt-get install -y $missing || say "Alguns pacotes não foram instalados; o resto continua." \
            "Some packages were not installed; the rest goes on."
    fi
fi

echo
say "1) Sessão: põe \"Glove\" na lista de ambientes da tela de login" \
    "1) Session: puts \"Glove\" in the login screen's list of desktops"
say "   (usa os programas deste projeto: recompilar já atualiza)." \
    "   (uses this project's programs: rebuilding updates it)."
if ask "$(t 'Instalar a sessão do Glove?' 'Install the Glove session?')" y; then
    sudo ./data/install-session.sh
fi

echo
say "2) Tela de login do Glove (greetd): lista de usuários com foto, senha e" \
    "2) Glove login screen (greetd): user list with pictures, password and"
say "   escolha do ambiente (Glove, GNOME…). Guarda a configuração anterior." \
    "   choice of desktop (Glove, GNOME…). Keeps the previous configuration."
if ask "$(t 'Configurar a tela de login do Glove?' 'Set up the Glove login screen?')" n; then
    if ! has greetd; then
        if has apt-get && ask "$(t 'O greetd não está instalado. Instalar pelo apt?' 'greetd is not installed. Install with apt?')" y; then
            sudo apt-get install -y greetd
        else
            say "Instale o greetd e rode de novo." "Install greetd and run again." >&2
            exit 1
        fi
    fi
    sudo ./data/install-login.sh
    if ! systemctl is-enabled greetd >/dev/null 2>&1; then
        current=""
        for g in gdm gdm3 lightdm sddm lxdm; do
            if systemctl is-enabled "$g" >/dev/null 2>&1; then current=$g; break; fi
        done
        echo "$(t 'O gerenciador de login atual é' 'The current login manager is') ${current:-$(t 'desconhecido' 'unknown')}."
        if [ -n "$current" ] && ask "$(t "Trocar o $current pelo greetd (vale a partir do próximo boot)?" "Replace $current with greetd (takes effect on the next boot)?")" n; then
            sudo systemctl disable "$current"
            sudo systemctl enable greetd
            echo "$(t 'Para voltar:' 'To go back:') ./install.sh --remove && sudo systemctl disable greetd && sudo systemctl enable $current"
        fi
    fi
fi

if has plymouth; then
    echo
    say "3) Tela de início e desligamento (Plymouth) com a arte do Glove." \
        "3) Boot and shutdown screen (Plymouth) with the Glove artwork."
    if ask "$(t 'Instalar o tema do Plymouth (refaz o initramfs)?' 'Install the Plymouth theme (rebuilds the initramfs)?')" n; then
        [ -d data/plymouth/glove ] || ./target/release/glove-arte plymouth data/plymouth/glove
        sudo ./data/install-plymouth.sh
    fi
fi

echo
say "Pronto. Saia da sessão e escolha \"Glove\" na tela de login." \
    "Done. Log out and choose \"Glove\" on the login screen."
echo "$(t 'Para testar numa janela, sem sair:' 'To try it in a window, without logging out:') $root_dir/target/release/glove"
