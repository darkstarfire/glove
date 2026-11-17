#!/bin/sh
# Compila o Glove e gera o pacote do Arch (.pkg.tar.zst) num contêiner Arch
# (.github/workflows/pacotes-distros.yml).
#
# O pacote é montado direto do código desta versão (GLOVE_SEM_MAKEPKG), sem o makepkg:
# ele não roda como root, e a receita (PKGBUILD) baixaria o código de novo pela tag.
set -e
cd "$(dirname "$0")/.."
. ci/comum.sh
pacman -Syu --noconfirm --needed base-devel pkgconf systemd-libs libinput seatd mesa libdrm \
    libxkbcommon fontconfig ttf-dejavu zstd curl
instalar_rust
GLOVE_SEM_MAKEPKG=1 ./glove-setup-tool.sh arch
ls pacotes/glove-*.pkg.tar.zst
# O pacman instala também as dependências do pacote (greetd…): confere os nomes.
pacman -U --noconfirm pacotes/glove-*.pkg.tar.zst
conferir

# O índice do repositório do pacman (glove.db e glove.files), que o ci/repositorios.sh
# publica no GitHub Pages ao lado do pacote. Como arquivos de verdade, sem os atalhos
# que o repo-add cria.
mkdir -p pacotes/repo-arch
(cd pacotes && repo-add -q repo-arch/glove.db.tar.gz glove-*.pkg.tar.zst)
for f in glove.db glove.files; do
    cp -L "pacotes/repo-arch/$f" "pacotes/repo-arch/$f.tmp"
    mv -f "pacotes/repo-arch/$f.tmp" "pacotes/repo-arch/$f"
done
rm -f pacotes/repo-arch/glove.db.tar.gz pacotes/repo-arch/glove.files.tar.gz
ls -l pacotes/repo-arch/
