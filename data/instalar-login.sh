#!/bin/sh
# Põe a tela de login do Glove no greetd (o Glove em modo login + glove-login).
# Guarda a configuração anterior do greetd e a devolve com --remover.
#   sudo ./data/instalar-login.sh            instala
#   sudo ./data/instalar-login.sh --remover  volta ao que era antes
# Nunca troca o gerenciador de login sozinho: só mostra como fazer.
set -e
dir="$(cd "$(dirname "$0")" && pwd)"
lib=/usr/local/lib/glove
conf=/etc/greetd/config.toml
backup=/etc/greetd/config.toml.antes-do-glove

if [ "$(id -u)" != 0 ]; then
    echo "Rode com sudo." >&2
    exit 1
fi

if [ "$1" = "--remover" ]; then
    if [ -f "$backup" ]; then
        mv "$backup" "$conf"
        echo "Configuração anterior do greetd devolvida."
    else
        echo "Não há configuração anterior guardada ($backup)."
    fi
    rm -rf "$lib"
    echo "Tela de login do Glove removida."
    exit 0
fi

if ! command -v greetd >/dev/null 2>&1; then
    echo "O greetd não está instalado. No Debian/Ubuntu: sudo apt install greetd" >&2
    exit 1
fi
for b in glove glove-login; do
    if [ ! -x "$dir/../target/release/$b" ]; then
        echo "Compile antes: cargo build --release" >&2
        exit 1
    fi
done

# O greeter roda como outro usuário, que não lê a pasta pessoal: os programas
# da tela de login vão para $lib (a sessão continua usando os do projeto).
mkdir -p "$lib"
install -m 755 "$dir/../target/release/glove" "$dir/../target/release/glove-login" "$lib/"

# Usuário do greeter: _greetd (Debian/Ubuntu) ou greeter (outros).
if id _greetd >/dev/null 2>&1; then greeter=_greetd; else greeter=greeter; fi

mkdir -p /etc/greetd /etc/glove/login
if [ -f "$conf" ] && [ ! -f "$backup" ]; then
    cp "$conf" "$backup"
    echo "Configuração anterior guardada em $backup"
fi
# Mantém o console (vt) que já estava configurado.
vt=$(sed -n 's/^vt *= *//p' "$backup" 2>/dev/null | head -1)
sed -e "s|@LIB@|$lib|" -e "s|@SESSAO@|$dir/glove-session|" -e "s|@GREETER@|$greeter|" \
    -e "s|^vt = 1|vt = ${vt:-1}|" "$dir/greetd/config.toml" > "$conf"
chmod 644 "$conf"
# A tela de login usa a configuração de quem instalou (escala da interface,
# monitor principal e resolução, teclado, tema): sem ela, a escala é a automática.
if [ -n "$SUDO_USER" ]; then
    casa=$(getent passwd "$SUDO_USER" | cut -d: -f6)
    cfg="${casa}/.config/glove/config.toml"
    if [ -f "$cfg" ]; then
        install -m 644 "$cfg" /etc/glove/login/config.toml
        echo "Tela de login com os ajustes de $SUDO_USER (monitor, escala, teclado e tema)."
    fi
fi
echo "Pronto: o greetd vai mostrar a tela de login do Glove."
if ! systemctl is-enabled greetd >/dev/null 2>&1; then
    echo
    echo "O greetd ainda não é o gerenciador de login. Para trocar (ex.: do GDM):"
    echo "  sudo systemctl disable gdm && sudo systemctl enable greetd"
    echo "Para voltar: sudo ./data/instalar-login.sh --remover && sudo systemctl disable greetd && sudo systemctl enable gdm"
fi
