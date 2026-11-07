#!/bin/sh
# Instalador do Glove para qualquer Linux: compila e instala no sistema, desinstala,
# e gera os pacotes .deb (Debian/Ubuntu), .tgz (Slackware), .rpm (Fedora) e
# .pkg.tar.zst (Arch).
#
#   ./instalador.sh                      menu
#   ./instalador.sh instalar [PREFIXO]   compila e instala (padrão: /usr/local)
#   ./instalador.sh desinstalar          o inverso de instalar
#   ./instalador.sh deb|slackware|rpm|arch|todos   gera os pacotes em pacotes/
#   ./instalador.sh dependencias         instala o que falta para compilar
#
# Para testar sem instalar nada: ./instalar.sh (usa os programas do projeto).
set -e
cd "$(dirname "$0")"
raiz=$(pwd)
versao=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
arq=$(uname -m)
saida="$raiz/pacotes"
url="https://github.com/fabioaacarneiro/glove"
resumo="Ambiente gráfico Wayland leve, inspirado no Mac OS X Tiger"
descricao="Compositor Wayland com barra de menu global, Dock com ampliação, Exposé,
Painel e apps próprios (Arquivos, Preferências, Terminal, Editor e outros),
com arte própria desenhada por código no estilo dos anos 2000."

# --- utilidades ---------------------------------------------------------------

msg() { printf '\033[1m==>\033[0m %s\n' "$*"; }
erro() { printf 'Erro: %s\n' "$*" >&2; exit 1; }
tem() { command -v "$1" >/dev/null 2>&1; }

# Pasta raiz do sistema onde instalar (só para testes: DESTDIR=/tmp/x).
sistema=${DESTDIR:-}

# Roda como root: direto se já for (ou testando num DESTDIR), senão pelo sudo (ou doas).
root() {
    if [ "$(id -u)" = 0 ] || [ -n "$sistema" ]; then "$@"
    elif tem sudo; then sudo "$@"
    elif tem doas; then doas "$@"
    else erro "precisa de root para: $*"; fi
}

pergunta() {
    if [ "$2" = s ]; then op="[S/n]"; else op="[s/N]"; fi
    printf '%s %s ' "$1" "$op"
    read -r r || r=""
    [ -z "$r" ] && r=$2
    case "$r" in s|S|sim|y|Y) return 0 ;; *) return 1 ;; esac
}

# Distribuição pelo gerenciador de pacotes que existe.
distro() {
    if tem apt-get; then echo debian
    elif tem dnf; then echo fedora
    elif tem pacman; then echo arch
    elif tem zypper; then echo suse
    elif tem xbps-install; then echo void
    elif tem apk; then echo alpine
    elif [ -f /etc/slackware-version ]; then echo slackware
    else echo outra; fi
}

# --- dependências ---------------------------------------------------------------

dependencias() {
    d=$(distro)
    msg "Instalando o que falta para compilar ($d)"
    case $d in
        debian) root apt-get install -y build-essential pkg-config libudev-dev libinput-dev libseat-dev \
                    libgbm-dev libdrm-dev libxkbcommon-dev libegl-dev libgles-dev fontconfig fonts-dejavu-core ;;
        fedora) root dnf install -y gcc pkgconf-pkg-config systemd-devel libinput-devel libseat-devel \
                    mesa-libgbm-devel libdrm-devel libxkbcommon-devel mesa-libEGL-devel fontconfig dejavu-sans-fonts ;;
        arch)   root pacman -S --needed --noconfirm base-devel pkgconf systemd-libs libinput seatd mesa libdrm \
                    libxkbcommon fontconfig ttf-dejavu ;;
        suse)   root zypper install -y gcc pkg-config systemd-devel libinput-devel libseat-devel libgbm-devel \
                    libdrm-devel libxkbcommon-devel Mesa-libEGL-devel fontconfig dejavu-fonts ;;
        void)   root xbps-install -Sy base-devel pkg-config eudev-libudev-devel libinput-devel libseat-devel \
                    mesa-devel libdrm-devel libxkbcommon-devel fontconfig dejavu-fonts-ttf ;;
        alpine) root apk add build-base pkgconf eudev-dev libinput-dev seatd-dev mesa-dev libdrm-dev \
                    libxkbcommon-dev fontconfig font-dejavu ;;
        slackware) echo "Slackware: a instalação completa já traz quase tudo; falta só o seatd (libseat), pelo SlackBuilds.org." ;;
        *) echo "Instale: compilador C, pkg-config e os pacotes de desenvolvimento de udev, libinput, libseat, gbm, libdrm, xkbcommon e EGL." ;;
    esac
    if ! tem cargo; then
        echo
        echo "Falta o Rust (1.85 ou mais novo). O jeito mais simples, em qualquer distribuição:"
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    fi
}

# --- compilação -------------------------------------------------------------------

compilar() {
    tem cargo || erro "falta o Rust (cargo). Rode: ./instalador.sh dependencias"
    msg "Compilando o Glove $versao (alguns minutos)"
    cargo build --release --locked
}

# Os atalhos dos apps (glove-files, glove-prefs…) apontam para o glove-apps.
apps() {
    find target/release -maxdepth 1 -type l -lname glove-apps -printf '%f\n' | sort
}

# Monta a instalação em $1 (pasta temporária) com o prefixo $2.
# $3 = estilo do PAM: debian (common-auth) ou outro (system-auth).
montar() {
    destino=$1; prefixo=$2; pam=$3
    [ -x target/release/glove ] && [ -x target/release/glove-apps ] || compilar
    [ -n "$(apps)" ] || compilar
    bin="$destino$prefixo/bin"
    mkdir -p "$bin"
    for b in glove glove-apps glove-portal glove-escolher; do
        install -m 755 "target/release/$b" "$bin/$b"
    done
    for a in $(apps); do
        ln -sf glove-apps "$bin/$a"
    done
    install -m 755 data/glove-session "$bin/glove-session"
    # Sessão na tela de login (todas as telas de login leem /usr/share/wayland-sessions).
    mkdir -p "$destino/usr/share/wayland-sessions"
    sed "s|^Exec=.*|Exec=$prefixo/bin/glove-session|" data/glove.desktop > "$destino/usr/share/wayland-sessions/glove.desktop"
    chmod 644 "$destino/usr/share/wayland-sessions/glove.desktop"
    # PAM para desbloquear a tela (só confere a senha).
    mkdir -p "$destino/etc/pam.d"
    if [ "$pam" = debian ]; then
        printf '# Desbloqueio da tela do Glove\nauth    optional  pam_faildelay.so delay=2000000\n@include common-auth\n@include common-account\n' > "$destino/etc/pam.d/glove"
    else
        printf '# Desbloqueio da tela do Glove\nauth     include  system-auth\naccount  include  system-auth\n' > "$destino/etc/pam.d/glove"
    fi
    chmod 644 "$destino/etc/pam.d/glove"
    # Tema de ícones (gerado pelo código, igual ao do repositório glove-icons).
    target/release/glove-arte icones "$destino$prefixo/share/icons/Glove" >/dev/null
    # Documentação e licença.
    mkdir -p "$destino$prefixo/share/doc/glove"
    install -m 644 README.md LICENSE "$destino$prefixo/share/doc/glove/"
    install -m 644 docs/PLANO.md docs/MEMORIA.md "$destino$prefixo/share/doc/glove/"
    # Pastas com a permissão padrão (independente da máscara de quem compila).
    find "$destino" -type d -exec chmod 755 {} +
    find "$destino" -type f -exec chmod go-w {} +
}

# Tamanho em KB de uma pasta montada.
tamanho_kb() { du -sk "$1" | cut -f1; }

# --- instalar / desinstalar -------------------------------------------------------

lista_local() { echo "$1/share/glove/instalados.txt"; }

instalar() {
    prefixo=${1:-/usr/local}
    case "$prefixo" in /*) ;; *) erro "o prefixo precisa ser um caminho absoluto" ;; esac
    if [ ! -x target/release/glove ] || pergunta "Compilar de novo antes de instalar?" n; then
        if ! tem cargo || ! tem pkg-config; then
            pergunta "Faltam ferramentas para compilar. Instalar agora?" s && dependencias
        fi
        compilar
    fi
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    case $(distro) in debian) estilo=debian ;; *) estilo=outro ;; esac
    msg "Montando a instalação em $prefixo"
    montar "$tmp" "$prefixo" "$estilo"
    # Lista do que foi instalado: é o que o "desinstalar" apaga.
    lista=$(lista_local "$prefixo")
    mkdir -p "$tmp$(dirname "$lista")"
    (cd "$tmp" && find . ! -type d | sed 's|^\.||' | sort) > "$tmp$lista"
    echo "$lista" >> "$tmp$lista"
    msg "Copiando para o sistema (pede a senha)"
    root mkdir -p "$sistema/"
    root cp -a "$tmp/." "$sistema/"
    root sh -c 'command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -f "$1" 2>/dev/null || true' sh "$sistema$prefixo/share/icons/Glove"
    echo
    echo "Glove $versao instalado em $prefixo. Saia da sessão e escolha \"Glove\" na tela de login."
    echo "Extras opcionais (rodam a partir do projeto): ./instalar.sh oferece a tela de login"
    echo "(greetd) e a tela de início (Plymouth)."
}

desinstalar() {
    # Instalado por pacote: quem remove é o gerenciador de pacotes.
    if [ -z "$sistema" ] && tem dpkg-query && dpkg-query -W glove >/dev/null 2>&1; then
        pergunta "O Glove foi instalado pelo pacote .deb. Remover com o apt?" s && root apt-get remove -y glove; return
    fi
    if [ -z "$sistema" ] && tem rpm && rpm -q glove >/dev/null 2>&1; then
        pergunta "O Glove foi instalado pelo pacote .rpm. Remover?" s && root rpm -e glove; return
    fi
    if [ -z "$sistema" ] && tem pacman && pacman -Q glove >/dev/null 2>&1; then
        pergunta "O Glove foi instalado pelo pacote do Arch. Remover?" s && root pacman -R --noconfirm glove; return
    fi
    if [ -z "$sistema" ] && ls /var/log/packages/glove-[0-9]* >/dev/null 2>&1; then
        pergunta "O Glove foi instalado pelo pacote do Slackware. Remover?" s && root removepkg glove; return
    fi
    achou=""
    for p in /usr/local /usr /opt/glove "$1"; do
        [ -n "$p" ] && [ -f "$sistema$(lista_local "$p")" ] && achou="$achou $p"
    done
    [ -n "$achou" ] || erro "não achei uma instalação do Glove feita por este instalador"
    for p in $achou; do
        lista="$sistema$(lista_local "$p")"
        pergunta "Remover o Glove instalado em $p?" s || continue
        msg "Removendo $(wc -l < "$lista") arquivos"
        # Apaga cada arquivo da lista e depois as pastas próprias do Glove.
        root sh -c 'while IFS= read -r f; do [ -n "$f" ] && rm -f "$2$f"; done < "$1"' sh "$lista" "$sistema" || true
        root sh -c 'for d in "$1/share/icons/Glove" "$1/share/doc/glove" "$1/share/glove"; do rm -rf "$d"; done' sh "$sistema$p"
    done
    echo "Glove removido. Sua configuração pessoal (~/.config/glove) foi mantida."
}

# --- pacotes ------------------------------------------------------------------------

# Pacotes de que o Glove precisa para rodar e os que melhoram (por distribuição).
deps_deb="libgbm1, libseat1, libudev1, libinput10 | libinput-bin, libxkbcommon0, libdrm2, libegl1, libgles2, libgl1-mesa-dri, fontconfig, fonts-dejavu-core"
rec_deb="xwayland, mpv, poppler-utils, pipewire-bin, wireplumber, xdg-utils, desktop-file-utils, dbus-bin"
deps_rpm="mesa-libEGL mesa-libGLES mesa-dri-drivers fontconfig dejavu-sans-fonts"
rec_rpm="xorg-x11-server-Xwayland mpv poppler-utils pipewire-utils wireplumber xdg-utils desktop-file-utils"
deps_arch="mesa libinput libxkbcommon seatd systemd-libs libdrm fontconfig ttf-dejavu"
opt_arch="xorg-xwayland:apps_X11 mpv:Vídeo poppler:PDF_no_Visualizador pipewire:som wireplumber:volume xdg-utils:Abrir_com desktop-file-utils:Abrir_com greetd:tela_de_login"

pacote_deb() {
    tem dpkg-deb || erro "falta o dpkg-deb (pacote dpkg)"
    tmp=$(mktemp -d)
    montar "$tmp" /usr debian
    mkdir -p "$tmp/DEBIAN"
    case $arq in x86_64) a=amd64 ;; aarch64) a=arm64 ;; *) a=$arq ;; esac
    cat > "$tmp/DEBIAN/control" <<EOF
Package: glove
Version: $versao
Architecture: $a
Maintainer: Fabio Carneiro <$(git config user.email 2>/dev/null || echo glove@localhost)>
Installed-Size: $(tamanho_kb "$tmp")
Depends: $deps_deb
Recommends: $rec_deb
Suggests: greetd, plymouth
Section: x11
Priority: optional
Homepage: $url
Description: $resumo
$(echo "$descricao" | sed 's/^/ /')
EOF
    cat > "$tmp/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -f /usr/share/icons/Glove || true
EOF
    chmod 755 "$tmp/DEBIAN/postinst"
    mkdir -p "$saida"
    f="$saida/glove_${versao}_$a.deb"
    dpkg-deb --root-owner-group -Zxz --build "$tmp" "$f" >/dev/null
    rm -rf "$tmp"
    msg "Pacote Debian/Ubuntu: $f"
}

pacote_slackware() {
    tmp=$(mktemp -d)
    montar "$tmp" /usr outro
    mkdir -p "$tmp/install"
    # slack-desc: 11 linhas com o nome do pacote na frente (a régua é exigida pelo pkgtools).
    {
        echo "     |-----handy-ruler------------------------------------------------------|"
        echo "glove: glove ($resumo)"
        echo "glove:"
        echo "$descricao" | tr '\n' ' ' | fold -s -w 70 | sed 's/ *$//; s/^/glove: /'
        echo
        echo "glove:"
        echo "glove: Precisa do seatd (libseat), do SlackBuilds.org."
        echo "glove: Site: $url"
        echo "glove:"
        echo "glove:"
    } | head -12 > "$tmp/install/slack-desc"
    cat > "$tmp/install/doinst.sh" <<'EOF'
if [ -x /usr/bin/gtk-update-icon-cache ]; then
  /usr/bin/gtk-update-icon-cache -q -f usr/share/icons/Glove >/dev/null 2>&1
fi
EOF
    mkdir -p "$saida"
    f="$saida/glove-$versao-$arq-1.tgz"
    (cd "$tmp" && tar --owner=0 --group=0 --numeric-owner -czf "$f" .)
    rm -rf "$tmp"
    msg "Pacote Slackware: $f (instale com: installpkg $(basename "$f"))"
}

pacote_rpm() {
    tmp=$(mktemp -d)
    montar "$tmp/raiz" /usr outro
    mkdir -p "$saida"
    spec="$saida/glove.spec"
    {
        cat <<EOF
Name:           glove
Version:        $versao
Release:        1
Summary:        $resumo
License:        GPL-3.0-or-later
URL:            $url
Requires:       $deps_rpm
Recommends:     $(echo "$rec_rpm" | sed 's/ /, /g')
%global debug_package %{nil}
%global __os_install_post %{nil}

%description
$descricao

%install
cp -a %{_raiz}/. %{buildroot}/

%post
gtk-update-icon-cache -q -f /usr/share/icons/Glove 2>/dev/null || :

%files
EOF
        (cd "$tmp/raiz" && find . ! -type d | sed 's|^\.||' | sort | sed 's|^\(/etc/.*\)|%config(noreplace) \1|')
        (cd "$tmp/raiz" && find ./usr/share/icons/Glove ./usr/share/doc/glove -type d | sed 's|^\.|%dir |' | sort)
    } > "$spec"
    if tem rpmbuild; then
        rpmbuild -bb --define "_topdir $tmp/rpm" --define "_raiz $tmp/raiz" "$spec" >/dev/null
        cp "$tmp"/rpm/RPMS/*/glove-*.rpm "$saida/"
        msg "Pacote Fedora: $(ls "$saida"/glove-*.rpm | tail -1)"
    else
        msg "Sem o rpmbuild aqui: deixei a receita em $spec"
        echo "  No Fedora: sudo dnf install rpm-build; no Debian/Ubuntu: sudo apt install rpm."
        echo "  Depois rode de novo: ./instalador.sh rpm"
    fi
    rm -rf "$tmp"
}

pacote_arch() {
    mkdir -p "$saida"
    # Receita para compilar no próprio Arch com o makepkg (o jeito do Arch).
    cat > "$saida/PKGBUILD" <<EOF
# Receita do Glove para o Arch (gerada por instalador.sh).
pkgname=glove
pkgver=$versao
pkgrel=1
pkgdesc="$resumo"
arch=('x86_64' 'aarch64')
url="$url"
license=('GPL-3.0-or-later')
depends=($(echo "$deps_arch" | sed "s/[^ ]*/'&'/g"))
optdepends=($(echo "$opt_arch" | tr ' ' '\n' | sed "s/_/ /g; s/^/'/; s/\$/'/; s/:/: /" | tr '\n' ' '))
makedepends=('cargo' 'pkgconf')
source=("git+$url.git#tag=v$versao")
sha256sums=('SKIP')

prepare() {
  cd glove
  cargo fetch --locked --target "\$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  cd glove
  cargo build --release --frozen
}

package() {
  cd glove
  ./instalador.sh _montar "\$pkgdir" /usr outro
}
EOF
    if tem makepkg; then
        (cd "$saida" && makepkg -f)
        msg "Pacote Arch: $(ls "$saida"/glove-*.pkg.tar.zst | tail -1)"
        return
    fi
    # Fora do Arch: monta o pacote direto (mesmo formato que o makepkg gera).
    tem zstd || erro "falta o zstd"
    tmp=$(mktemp -d)
    montar "$tmp" /usr outro
    {
        echo "pkgname = glove"
        echo "pkgbase = glove"
        echo "pkgver = $versao-1"
        echo "pkgdesc = $resumo"
        echo "url = $url"
        echo "builddate = $(date +%s)"
        echo "packager = instalador.sh do Glove"
        echo "size = $(( $(tamanho_kb "$tmp") * 1024 ))"
        echo "arch = $arq"
        echo "license = GPL-3.0-or-later"
        for d in $deps_arch; do echo "depend = $d"; done
        for d in $opt_arch; do echo "optdepend = $(echo "$d" | sed 's/_/ /g; s/:/: /')"; done
    } > "$tmp/.PKGINFO"
    # .MTREE: a lista de arquivos que o pacman usa para conferir a instalação.
    (
        cd "$tmp"
        echo "#mtree"
        echo "/set type=file uid=0 gid=0 mode=644"
        find . -mindepth 1 ! -name .PKGINFO | sort | while IFS= read -r p; do
            t=$(date -r "$p" +%s 2>/dev/null || echo 0)
            if [ -L "$p" ]; then
                echo "$p time=$t.0 mode=777 type=link link=$(readlink "$p")"
            elif [ -d "$p" ]; then
                echo "$p time=$t.0 mode=755 type=dir"
            else
                m=$(stat -c %a "$p")
                echo "$p time=$t.0 mode=$m size=$(stat -c %s "$p") sha256digest=$(sha256sum "$p" | cut -d' ' -f1)"
            fi
        done
    ) | gzip -9n > "$tmp/.MTREE.tmp"
    mv "$tmp/.MTREE.tmp" "$tmp/.MTREE"
    chmod 644 "$tmp/.PKGINFO" "$tmp/.MTREE"
    f="$saida/glove-$versao-1-$arq.pkg.tar.zst"
    (cd "$tmp" && tar --owner=0 --group=0 --numeric-owner -cf - .PKGINFO .MTREE $(ls -A | grep -v '^\.PKGINFO$\|^\.MTREE$') | zstd -19 -q -f -o "$f")
    rm -rf "$tmp"
    msg "Pacote Arch: $f (instale com: sudo pacman -U $(basename "$f"))"
    echo "  No Arch também dá para compilar lá: makepkg -si (com o PKGBUILD em $saida)"
}

# --- menu ---------------------------------------------------------------------------

menu() {
    echo "Glove $versao — instalador ($(distro), $arq)"
    echo
    echo "  1) Instalar (compila e instala em /usr/local)"
    echo "  2) Desinstalar"
    echo "  3) Pacote .deb (Debian, Ubuntu, Mint…)"
    echo "  4) Pacote .tgz (Slackware)"
    echo "  5) Pacote .rpm (Fedora, openSUSE…)"
    echo "  6) Pacote do Arch (.pkg.tar.zst e PKGBUILD)"
    echo "  7) Todos os pacotes"
    echo "  8) Instalar o que falta para compilar"
    echo "  0) Sair"
    printf 'Escolha: '
    read -r op || op=0
    case "$op" in
        1) instalar ;;
        2) desinstalar ;;
        3) pacote_deb ;;
        4) pacote_slackware ;;
        5) pacote_rpm ;;
        6) pacote_arch ;;
        7) pacote_deb; pacote_slackware; pacote_rpm; pacote_arch ;;
        8) dependencias ;;
        *) exit 0 ;;
    esac
}

case "${1:-}" in
    "") menu ;;
    instalar) instalar "$2" ;;
    desinstalar|remover) desinstalar "$2" ;;
    deb) pacote_deb ;;
    slackware|slack|tgz) pacote_slackware ;;
    rpm|fedora) pacote_rpm ;;
    arch|pacman) pacote_arch ;;
    todos|pacotes) pacote_deb; pacote_slackware; pacote_rpm; pacote_arch ;;
    dependencias) dependencias ;;
    _montar) montar "$2" "$3" "$4" ;;
    -h|--help|ajuda) sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//' ;;
    *) erro "opção desconhecida: $1 (veja ./instalador.sh ajuda)" ;;
esac
