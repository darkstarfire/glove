#!/bin/sh
# Põe o Glove na lista de sessões da tela de login, usando os arquivos deste projeto
# (sem copiar binários: basta compilar de novo para atualizar).
#   sudo ./data/instalar-sessao.sh            instala
#   sudo ./data/instalar-sessao.sh --remover  tira da lista
set -e
dir="$(cd "$(dirname "$0")" && pwd)"
dest=/usr/share/wayland-sessions/glove.desktop
if [ "$1" = "--remover" ]; then
    rm -f "$dest"
    echo "Glove removido da tela de login."
    exit 0
fi
if [ ! -x "$dir/../target/release/glove" ]; then
    echo "Compile antes: cargo build --release" >&2
    exit 1
fi
sed "s|^Exec=.*|Exec=$dir/glove-session|" "$dir/glove.desktop" > "$dest"
chmod 644 "$dest"
echo "Pronto: saia da sessão e escolha \"Glove\" na tela de login (ícone de engrenagem)."
