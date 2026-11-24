#!/bin/sh
# Monta o site dos repositórios do Glove (GitHub Pages), assinados com a chave que
# está no chaveiro do gpg (GNUPGHOME): o apt (Debian, Ubuntu e o Glove OS), o dnf
# (Fedora) e o pacman (Arch). O Slackware não tem repositório: o Glove baixa o pacote
# da release.
#
#   ci/repositorios.sh VERSÃO PASTA_DOS_DEB PASTA_DAS_DISTROS SITE
#
# PASTA_DAS_DISTROS traz o .rpm, o .pkg.tar.zst e o índice do pacman (glove.db e
# glove.files, feitos no Arch por ci/arch.sh), com o pacote da Jangada ao lado. Uma distribuição que não gerou pacote
# nesta versão continua com o repositório que já está no ar (`manter`), para o dnf e o
# pacman de quem o usa não darem erro.
set -e
v=$1; debs=$2; distros=$3; r=$4
[ -n "$r" ] || { echo "uso: ci/repositorios.sh VERSÃO PASTA_DOS_DEB PASTA_DAS_DISTROS SITE"; exit 1; }
cd "$(dirname "$0")/.."
url=${GLOVE_REPO:-https://fabioaacarneiro.github.io/glove}
impressao=$(gpg --show-keys --with-colons data/glove-os/glove-os-repositorio.asc | awk -F: '$1=="fpr"{print $10; exit}')
mkdir -p "$r"

# A lista dos arquivos de um repositório (para o `manter` da versão seguinte).
listar() {
    (cd "$r/$1" && find . -type f ! -name arquivos.txt | sed 's|^\./||' | sort > arquivos.txt)
}

# Sem pacote novo: copia o repositório que está no ar, pela lista dele.
manter() {
    echo "::warning::Sem o pacote de $1 nesta versão: fica o repositório que já está no ar."
    if curl -fsS --max-time 30 "$url/$1/arquivos.txt" -o /tmp/arquivos.txt; then
        while read -r f; do
            mkdir -p "$r/$1/$(dirname "$f")"
            curl -fsS --max-time 300 "$url/$1/$f" -o "$r/$1/$f"
        done < /tmp/arquivos.txt
        cp /tmp/arquivos.txt "$r/$1/arquivos.txt"
    fi
}

# --- apt -----------------------------------------------------------------------------
mkdir -p "$r/pool/main" "$r/dists/glove/main/binary-amd64"
cp "$debs"/*.deb "$r/pool/main/"
(cd "$r" && apt-ftparchive packages pool > dists/glove/main/binary-amd64/Packages)
gzip -9kn "$r/dists/glove/main/binary-amd64/Packages"
(cd "$r/dists/glove" && apt-ftparchive \
    -o APT::FTPArchive::Release::Origin="Glove OS" \
    -o APT::FTPArchive::Release::Label="Glove OS" \
    -o APT::FTPArchive::Release::Suite=glove \
    -o APT::FTPArchive::Release::Codename=glove \
    -o APT::FTPArchive::Release::Architectures="amd64 all" \
    -o APT::FTPArchive::Release::Components=main \
    -o APT::FTPArchive::Release::Description="Glove OS: o Glove e a identidade do Glove OS (o Debian vem do Debian)" \
    release . > Release)
gpg --batch --yes --clearsign -o "$r/dists/glove/InRelease" "$r/dists/glove/Release"
gpg --batch --yes -abs -o "$r/dists/glove/Release.gpg" "$r/dists/glove/Release"
cp data/glove-os/glove-os-repositorio.asc data/glove-os/glove-os-repositorio.gpg "$r/"

# --- dnf (Fedora) --------------------------------------------------------------------
# Os pacotes não são assinados um a um: o índice é (repo_gpgcheck), e ele traz a soma
# de cada pacote.
if ls "$distros"/*.rpm >/dev/null 2>&1; then
    mkdir -p "$r/fedora"
    cp "$distros"/*.rpm "$r/fedora/"
    createrepo_c -q "$r/fedora"
    gpg --batch --yes --detach-sign --armor "$r/fedora/repodata/repomd.xml"
    listar fedora
else
    manter fedora
fi
cat > "$r/glove.repo" <<EOF2
[glove]
name=Glove
baseurl=$url/fedora
enabled=1
gpgcheck=0
repo_gpgcheck=1
gpgkey=$url/glove-os-repositorio.asc
skip_if_unavailable=True
metadata_expire=6h
EOF2

# --- pacman (Arch) -------------------------------------------------------------------
# O pacote leva a assinatura ao lado (.sig); o índice foi feito no Arch (repo-add).
if ls "$distros"/*.pkg.tar.zst >/dev/null 2>&1 && [ -f "$distros/glove.db" ]; then
    mkdir -p "$r/arch"
    cp "$distros"/*.pkg.tar.zst "$distros/glove.db" "$distros/glove.files" "$r/arch/"
    for p in "$r"/arch/*.pkg.tar.zst; do
        gpg --batch --yes --detach-sign "$p"
    done
    listar arch
else
    manter arch
fi

# --- o que há de novo e a página -------------------------------------------------------
# (Lido pelo painel Atualização de Software do Glove.)
notas=$(git log -1 --format=%B | grep -v '^Co-Authored-By:' || true)
jq -n --arg v "$v" --arg d "$(date -u +%Y-%m-%d)" --arg n "$notas" '{versao:$v, data:$d, notas:$n}' > "$r/novidades.json"
cat > "$r/index.html" <<EOF2
<!doctype html><meta charset="utf-8"><title>Glove — repositórios</title>
<h1>Glove — repositórios</h1>
<p>Versão $v. O Glove OS já vem com o repositório; nas outras distribuições, configure uma vez e
o Glove passa a chegar junto com as atualizações do sistema. (No Slackware, o Glove baixa o
pacote da <a href="https://github.com/fabioaacarneiro/glove/releases/latest">release</a>.)</p>
<h2>Debian e Ubuntu (apt)</h2>
<pre>sudo curl -fsSL $url/glove-os-repositorio.gpg -o /usr/share/keyrings/glove-os-repositorio.gpg
printf 'Types: deb\\nURIs: $url\\nSuites: glove\\nComponents: main\\nSigned-By: /usr/share/keyrings/glove-os-repositorio.gpg\\n' | sudo tee /etc/apt/sources.list.d/glove-os.sources
sudo apt update && sudo apt install glove</pre>
<h2>Fedora (dnf)</h2>
<pre>sudo curl -fsSL $url/glove.repo -o /etc/yum.repos.d/glove.repo
sudo dnf install glove</pre>
<h2>Arch (pacman)</h2>
<pre>curl -fsSL $url/glove-os-repositorio.asc | sudo pacman-key --add -
sudo pacman-key --lsign-key $impressao
printf '\\n[glove]\\nServer = $url/arch\\n' | sudo tee -a /etc/pacman.conf
sudo pacman -Sy glove</pre>
<h2>A Jangada (o navegador)</h2>
<p>Vem pelo mesmo repositório: <code>sudo apt install jangada</code>, <code>sudo dnf install jangada</code>
ou <code>sudo pacman -S jangada</code> (o motor, o WebKitGTK, vem da distribuição). No Slackware
-current, o pacote está na <a href="https://github.com/fabioaacarneiro/jangada/releases/latest">release
da Jangada</a> (com o motor junto); no Slackware 15.0, na
<a href="https://github.com/fabioaacarneiro/jangada/releases/tag/v1.15.1">1.15.1</a>.</p>
EOF2
find "$r" -type f | sort
