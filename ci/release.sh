#!/bin/sh
# Garante que a release da versão existe no GitHub: se ainda não existe, cria com as
# notas (o texto do commit da versão e como usar os arquivos). Usado pelos trabalhos que
# publicam na release (.github/workflows/iso.yml), na ordem em que terminarem.
#   ci/release.sh v0.12.73      (precisa do GH_TOKEN)
set -e
tag=$1
[ -n "$tag" ] || { echo "uso: ci/release.sh TAG"; exit 1; }
if gh release view "$tag" >/dev/null 2>&1; then
    exit 0
fi
v=${tag#v}
# As notas: a mensagem do commit da versão (sem a linha do coautor).
msg=$(git log -1 --format=%B | grep -v '^Co-Authored-By:' || true)
cat > /tmp/notas.md <<EOF2
$msg

**Glove OS (ISO ao vivo, amd64):** grave num pendrive com
\`sudo dd if=glove-os-$v-amd64.iso of=/dev/sdX bs=4M status=progress oflag=sync\`
(apaga o pendrive), ou com o Discos do Glove (Disco Inicializável…).

**O Glove noutras distribuições** (amd64):

| Distribuição | Arquivo | Instalar |
|---|---|---|
| Debian, Ubuntu, Mint, Zorin | \`glove_${v}_amd64.deb\` | \`sudo apt install ./glove_${v}_amd64.deb\` |
| Fedora 44 ou mais novo | \`glove-$v-1.x86_64.rpm\` | \`sudo dnf install ./glove-$v-1.x86_64.rpm\` |
| Arch | \`glove-$v-1-x86_64.pkg.tar.zst\` | \`sudo pacman -U glove-$v-1-x86_64.pkg.tar.zst\` |
| Slackware 15.0 ou -current | \`glove-$v-x86_64-${BUILD:-1}.tgz\` | \`upgradepkg --install-new glove-$v-x86_64-${BUILD:-1}.tgz\` (precisa do seatd e do greetd, do SlackBuilds.org) |

Dentro do Glove, Preferências ▸ Atualização de Software instala as versões seguintes. No Debian/Ubuntu,
no Fedora e no Arch há também os [repositórios](https://fabioaacarneiro.github.io/glove/) (apt, dnf e pacman).

Confira os arquivos com \`sha256sum -c SHA256SUMS\`.
EOF2
gh release create "$tag" --title "Glove $tag" --notes-file /tmp/notas.md --verify-tag
