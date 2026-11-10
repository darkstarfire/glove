#!/bin/sh
# Puts the Glove login screen in greetd (Glove in login mode + glove-login).
# Keeps the previous greetd configuration and restores it with --remove.
#   sudo ./data/install-login.sh           installs
#   sudo ./data/install-login.sh --remove  goes back to what was there before
# Never switches the login manager by itself: it only shows how to do it.
set -e
dir="$(cd "$(dirname "$0")" && pwd)"
# Message language: Portuguese if the system locale is Portuguese, English otherwise.
case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in pt*) lang=pt ;; *) lang=en ;; esac
t() { if [ "$lang" = pt ]; then printf '%s' "$1"; else printf '%s' "$2"; fi; }
say() { printf '%s\n' "$(t "$1" "$2")"; }

lib=/usr/local/lib/glove
conf=/etc/greetd/config.toml
# The file name stays in Portuguese: existing installations already have it.
backup=/etc/greetd/config.toml.antes-do-glove

if [ "$(id -u)" != 0 ]; then
    say "Rode com sudo." "Run with sudo." >&2
    exit 1
fi

case "${1:-}" in
    --remove|--remover)
        if [ -f "$backup" ]; then
            mv "$backup" "$conf"
            say "Configuração anterior do greetd devolvida." "Previous greetd configuration restored."
        else
            say "Não há configuração anterior guardada ($backup)." "There is no previous configuration saved ($backup)."
        fi
        rm -rf "$lib"
        say "Tela de login do Glove removida." "Glove login screen removed."
        exit 0
        ;;
esac

if ! command -v greetd >/dev/null 2>&1; then
    say "O greetd não está instalado. No Debian/Ubuntu: sudo apt install greetd" \
        "greetd is not installed. On Debian/Ubuntu: sudo apt install greetd" >&2
    exit 1
fi
for b in glove glove-login; do
    if [ ! -x "$dir/../target/release/$b" ]; then
        say "Compile antes: cargo build --release" "Build first: cargo build --release" >&2
        exit 1
    fi
done

# The greeter runs as another user, who cannot read the home folder: the login
# screen's programs go to $lib (the session keeps using the project's own).
mkdir -p "$lib"
install -m 755 "$dir/../target/release/glove" "$dir/../target/release/glove-login" "$lib/"

# Greeter user: _greetd (Debian/Ubuntu) or greeter (others).
if id _greetd >/dev/null 2>&1; then greeter=_greetd; else greeter=greeter; fi

mkdir -p /etc/greetd /etc/glove/login
if [ -f "$conf" ] && [ ! -f "$backup" ]; then
    cp "$conf" "$backup"
    say "Configuração anterior guardada em $backup" "Previous configuration saved to $backup"
fi
# Keeps the console (vt) that was already configured.
vt=$(sed -n 's/^vt *= *//p' "$backup" 2>/dev/null | head -1)
sed -e "s|@LIB@|$lib|" -e "s|@SESSION@|$dir/glove-session|" -e "s|@GREETER@|$greeter|" \
    -e "s|^vt = 1|vt = ${vt:-1}|" "$dir/greetd/config.toml" > "$conf"
chmod 644 "$conf"
# The login screen uses the settings of whoever installed it (interface scale,
# main display and resolution, keyboard, theme): without them, the scale is automatic.
if [ -n "$SUDO_USER" ]; then
    home_dir=$(getent passwd "$SUDO_USER" | cut -d: -f6)
    cfg="${home_dir}/.config/glove/config.toml"
    if [ -f "$cfg" ]; then
        install -m 644 "$cfg" /etc/glove/login/config.toml
        say "Tela de login com os ajustes de $SUDO_USER (monitor, escala, teclado e tema)." \
            "Login screen with $SUDO_USER's settings (display, scale, keyboard and theme)."
    fi
fi
say "Pronto: o greetd vai mostrar a tela de login do Glove." "Done: greetd will show the Glove login screen."
if ! systemctl is-enabled greetd >/dev/null 2>&1; then
    echo
    say "O greetd ainda não é o gerenciador de login. Para trocar (ex.: do GDM):" \
        "greetd is not the login manager yet. To switch (e.g. from GDM):"
    echo "  sudo systemctl disable gdm && sudo systemctl enable greetd"
    echo "$(t 'Para voltar:' 'To go back:') sudo ./data/install-login.sh --remove && sudo systemctl disable greetd && sudo systemctl enable gdm"
fi
