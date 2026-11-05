#!/bin/sh
# Instala o tema do Plymouth do Glove (início e desligamento do sistema).
# A arte é gerada por código: ./target/release/glove-arte plymouth data/plymouth/glove
#   sudo ./data/instalar-plymouth.sh            instala e ativa
#   sudo ./data/instalar-plymouth.sh --remover  volta ao tema anterior
set -e
dir="$(cd "$(dirname "$0")" && pwd)"
dest=/usr/share/plymouth/themes/glove
anterior=/var/lib/glove-plymouth-anterior

if [ "$(id -u)" != 0 ]; then
    echo "Rode com sudo." >&2
    exit 1
fi
if ! command -v plymouth >/dev/null 2>&1; then
    echo "O Plymouth não está instalado." >&2
    exit 1
fi

# Debian/Ubuntu escolhem o tema por update-alternatives; os outros, por plymouth-set-default-theme.
atual() {
    if command -v plymouth-set-default-theme >/dev/null 2>&1; then
        plymouth-set-default-theme
    else
        readlink -f /usr/share/plymouth/themes/default.plymouth
    fi
}
refaz_initramfs() {
    if command -v update-initramfs >/dev/null 2>&1; then update-initramfs -u
    elif command -v dracut >/dev/null 2>&1; then dracut -f
    elif command -v mkinitcpio >/dev/null 2>&1; then mkinitcpio -P
    fi
}

if [ "$1" = "--remover" ]; then
    if command -v plymouth-set-default-theme >/dev/null 2>&1; then
        plymouth-set-default-theme "$(cat "$anterior" 2>/dev/null || echo spinner)"
    else
        update-alternatives --remove default.plymouth "$dest/glove.plymouth" || true
    fi
    rm -rf "$dest" "$anterior"
    refaz_initramfs
    echo "Tema do Glove removido; voltou o anterior."
    exit 0
fi

[ -f "$dir/plymouth/glove/glove.script" ] || { echo "Gere a arte antes: ./target/release/glove-arte plymouth data/plymouth/glove" >&2; exit 1; }
[ -f "$anterior" ] || atual > "$anterior"
mkdir -p "$dest"
install -m 644 "$dir"/plymouth/glove/* "$dest/"
if command -v plymouth-set-default-theme >/dev/null 2>&1; then
    plymouth-set-default-theme glove
else
    update-alternatives --install /usr/share/plymouth/themes/default.plymouth default.plymouth "$dest/glove.plymouth" 100
    update-alternatives --set default.plymouth "$dest/glove.plymouth"
fi
refaz_initramfs
echo "Pronto: o tema do Glove aparece no próximo início (o anterior foi anotado em $anterior)."
