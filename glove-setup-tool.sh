#!/bin/sh
# Glove setup tool: builds and installs on any Linux, uninstalls, makes the packages
# (.deb, Slackware .tgz, .rpm, Arch), enables the login screen and builds the live ISO.
# Messages are in English; in Portuguese when the system locale is Portuguese.
# Help: ./glove-setup-tool.sh help
set -e
cd "$(dirname "$0")"
root_dir=$(pwd)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
arch=$(uname -m)
out_dir="$root_dir/pacotes"
url="https://github.com/fabioaacarneiro/glove"
summary="Light Wayland desktop inspired by Mac OS X Tiger"
description="Wayland compositor with a global menu bar, a magnifying Dock, Exposé,
Dashboard and its own apps (Files, Preferences, Terminal, Text Editor and more),
with original code-drawn artwork in the style of the 2000s."

# --- utilities ----------------------------------------------------------------

# Message language: Portuguese if the system locale is Portuguese, English otherwise.
case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in pt*) lang=pt ;; *) lang=en ;; esac
# t "texto em português" "text in English"
t() { if [ "$lang" = pt ]; then printf '%s' "$1"; else printf '%s' "$2"; fi; }

msg() { printf '\033[1m==>\033[0m %s\n' "$*"; }
die() { printf '%s %s\n' "$(t 'Erro:' 'Error:')" "$*" >&2; exit 1; }
has() { command -v "$1" >/dev/null 2>&1; }

# System root to install into (for tests only: DESTDIR=/tmp/x).
sysroot=${DESTDIR:-}

# Runs as root: directly if already root (or testing in a DESTDIR), else through sudo (or doas).
root() {
    if [ "$(id -u)" = 0 ] || [ -n "$sysroot" ]; then "$@"
    elif has sudo; then sudo "$@"
    elif has doas; then doas "$@"
    else die "$(t "precisa de root para: $*" "root is needed for: $*")"; fi
}

ask() {
    # ask "question" y|n (the default answer)  ->  0 = yes
    if [ "$lang" = pt ]; then yes_key=S; else yes_key=Y; fi
    if [ "$2" = y ]; then op="[$yes_key/n]"; else op="[$(echo $yes_key | tr SY sy)/N]"; fi
    printf '%s %s ' "$1" "$op"
    read -r r || r=""
    [ -z "$r" ] && r=$2
    case "$r" in y|Y|yes|s|S|sim) return 0 ;; *) return 1 ;; esac
}

# The distribution, by the package manager that exists.
distro() {
    if has apt-get; then echo debian
    elif has dnf; then echo fedora
    elif has pacman; then echo arch
    elif has zypper; then echo suse
    elif has xbps-install; then echo void
    elif has apk; then echo alpine
    elif [ -f /etc/slackware-version ]; then echo slackware
    else echo other; fi
}

# --- dependencies ---------------------------------------------------------------

install_deps() {
    d=$(distro)
    msg "$(t "Instalando o que falta para compilar ($d)" "Installing what’s missing to build ($d)")"
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
        slackware) echo "$(t "Slackware: a instalação completa já traz quase tudo; falta só o seatd (libseat), pelo SlackBuilds.org." "Slackware: a full installation already has almost everything; only seatd (libseat) is missing, from SlackBuilds.org.")" ;;
        *) echo "$(t "Instale: compilador C, pkg-config e os pacotes de desenvolvimento de udev, libinput, libseat, gbm, libdrm, xkbcommon e EGL." "Install: a C compiler, pkg-config and the development packages for udev, libinput, libseat, gbm, libdrm, xkbcommon and EGL.")" ;;
    esac
    if ! has cargo; then
        echo
        echo "$(t "Falta o Rust (1.85 ou mais novo). O jeito mais simples, em qualquer distribuição:" "Rust (1.85 or newer) is missing. The simplest way, on any distribution:")"
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    fi
}

# --- build ------------------------------------------------------------------------

# glove-ui (the apps' library) is a Git submodule in ui/. This brings it to the version
# this Glove asks for. "git pull" does not update the submodule by itself: without
# this, the new Glove builds against the old glove-ui and fails.
update_submodule() {
    if [ -d .git ] && has git; then
        if [ ! -f ui/Cargo.toml ]; then
            msg "$(t "Baixando a glove-ui (submódulo)" "Downloading glove-ui (submodule)")"
        fi
        # A developer may have glove-ui ahead of the recorded version: leave it alone then.
        reg=$(git ls-tree HEAD ui 2>/dev/null | awk '{print $3}')
        current=$(git -C ui rev-parse HEAD 2>/dev/null || true)
        if [ -n "$current" ] && [ -n "$reg" ] && [ "$current" != "$reg" ] && git -C ui merge-base --is-ancestor "$reg" "$current" 2>/dev/null; then
            return 0
        fi
        git submodule sync --quiet
        git submodule update --init --recursive || die "$(t "não deu para atualizar a glove-ui (pasta ui/); rode: git submodule update --init --recursive" "could not update glove-ui (ui/ folder); run: git submodule update --init --recursive")"
    elif [ ! -f ui/Cargo.toml ]; then
        die "$(t "falta a pasta ui/ (glove-ui): clone com git clone --recursive" "the ui/ folder (glove-ui) is missing: clone with git clone --recursive")"
    fi
}

compile() {
    update_submodule
    has cargo || die "$(t "falta o Rust (cargo). Rode: ./glove-setup-tool.sh dependencias" "Rust (cargo) is missing. Run: ./glove-setup-tool.sh deps")"
    msg "$(t "Compilando o Glove $version (alguns minutos)" "Building Glove $version (a few minutes)")"
    cargo build --release --locked
}

# The app shortcuts (glove-files, glove-prefs…) are links to glove-apps.
app_links() {
    find target/release -maxdepth 1 -type l -lname glove-apps -printf '%f\n' | sort
}

# Stages the installation in $1 (a temporary folder) with the prefix $2.
# $3 = PAM style: debian (common-auth) or other (system-auth).
stage() {
    dest=$1; prefix=$2; pam=$3
    [ -x target/release/glove ] && [ -x target/release/glove-apps ] || compile
    [ -n "$(app_links)" ] || compile
    bin="$dest$prefix/bin"
    mkdir -p "$bin"
    for b in glove glove-apps glove-portal glove-escolher; do
        install -m 755 "target/release/$b" "$bin/$b"
    done
    for a in $(app_links); do
        ln -sf glove-apps "$bin/$a"
    done
    install -m 755 data/glove-session "$bin/glove-session"
    # Login screen: the glove-login service (greetd with Glove's configuration).
    # To enable: sudo systemctl enable --force glove-login
    mkdir -p "$dest$prefix/lib/glove" "$dest$prefix/lib/systemd/system"
    install -m 755 data/glove-login-greetd "$dest$prefix/lib/glove/glove-login-greetd"
    sed "s|@PREFIXO@|$prefix|" data/glove-login.service > "$dest$prefix/lib/systemd/system/glove-login.service"
    chmod 644 "$dest$prefix/lib/systemd/system/glove-login.service"
    # Session entry (every login screen reads /usr/share/wayland-sessions).
    mkdir -p "$dest/usr/share/wayland-sessions"
    sed "s|^Exec=.*|Exec=$prefix/bin/glove-session|" data/glove.desktop > "$dest/usr/share/wayland-sessions/glove.desktop"
    chmod 644 "$dest/usr/share/wayland-sessions/glove.desktop"
    # PAM for unlocking the screen (only checks the password).
    mkdir -p "$dest/etc/pam.d"
    if [ "$pam" = debian ]; then
        printf '# Glove screen unlock\nauth    optional  pam_faildelay.so delay=2000000\n@include common-auth\n@include common-account\n' > "$dest/etc/pam.d/glove"
    else
        printf '# Glove screen unlock\nauth     include  system-auth\naccount  include  system-auth\n' > "$dest/etc/pam.d/glove"
    fi
    chmod 644 "$dest/etc/pam.d/glove"
    # Icon theme (drawn by code, the same as the glove-icons repository).
    target/release/glove-arte icones "$dest$prefix/share/icons/Glove" >/dev/null
    # Wallpapers (the Glove OS one is the default).
    mkdir -p "$dest$prefix/share/glove/papeis"
    install -m 644 data/papeis/*.png "$dest$prefix/share/glove/papeis/"
    # Documentation and license.
    mkdir -p "$dest$prefix/share/doc/glove"
    install -m 644 README.md LICENSE "$dest$prefix/share/doc/glove/"
    install -m 644 docs/PLANO.md docs/MEMORIA.md "$dest$prefix/share/doc/glove/"
    # Folders with the default permission (whatever the builder's umask is).
    find "$dest" -type d -exec chmod 755 {} +
    find "$dest" -type f -exec chmod go-w {} +
}

# After installing (packages and "install"): systemd sees the login screen service
# and the icon theme gets indexed.
post_install_sh='command -v systemctl >/dev/null 2>&1 && systemctl daemon-reload 2>/dev/null || true
for t in /usr/share/icons/Glove /usr/local/share/icons/Glove; do
  [ -d "$t" ] && command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -f "$t" 2>/dev/null || true
done
echo "Glove installed. To use the Glove login screen: sudo systemctl enable --force glove-login"
echo "To go back to the previous one (e.g. GDM): sudo systemctl enable --force gdm"'

# Before removing: if the Glove login screen is the main one, go back to the previous
# one (the one saved by "glove-setup-tool.sh login", or the first one installed), so
# the computer is not left without a login screen.
pre_remove_sh='if command -v systemctl >/dev/null 2>&1 && systemctl is-enabled glove-login >/dev/null 2>&1; then
  systemctl disable glove-login 2>/dev/null || true
  previous=$(cat /etc/glove/login/gerenciador-anterior 2>/dev/null || true)
  for d in $previous gdm gdm3 lightdm sddm lxdm greetd; do
    if systemctl cat "$d.service" >/dev/null 2>&1 && systemctl enable --force "$d.service" 2>/dev/null; then
      echo "Glove: the login screen goes back to $d."
      break
    fi
  done
fi'

# Size in KB of a staged folder.
size_kb() { du -sk "$1" | cut -f1; }

# --- install / uninstall ----------------------------------------------------------

manifest_path() { echo "$1/share/glove/instalados.txt"; }

# Copies the staged tree ($1) to the system ($2), replacing each file by name: copies
# it as ".new" and renames it. This way Glove can be installed while it is running (a
# program in use cannot be overwritten, "Text file busy"; renamed, the current
# session keeps the old one in memory and the next one opens the new one). The
# files end up owned by root, not by whoever built them.
copy_sh='cd "$1" || exit 1
find . -type d | while IFS= read -r d; do mkdir -p "$2/${d#./}" || exit 1; done
find . ! -type d | while IFS= read -r f; do
  target="$2/${f#./}"
  rm -f "$target.new"
  cp -a "$f" "$target.new" && mv -f "$target.new" "$target" || exit 1
  if [ "$(id -u)" = 0 ]; then chown -h root:root "$target"; fi
done'

# The package manager Glove is installed with as a package (deb, rpm, arch), or nothing.
installed_package() {
    if has dpkg-query && dpkg-query -W -f='${Status}' glove 2>/dev/null | grep -q "ok installed"; then echo deb
    elif has rpm && rpm -q glove >/dev/null 2>&1; then echo rpm
    elif has pacman && pacman -Q glove >/dev/null 2>&1; then echo arch
    fi
}

install_glove() {
    prefix=${1:-/usr/local}
    case "$prefix" in /*) ;; *) die "$(t "o prefixo precisa ser um caminho absoluto" "the prefix must be an absolute path")" ;; esac
    if [ ! -x target/release/glove ] || ask "$(t "Compilar de novo antes de instalar?" "Build again before installing?")" n; then
        if ! has cargo || ! has pkg-config; then
            ask "$(t "Faltam ferramentas para compilar. Instalar agora?" "Build tools are missing. Install them now?")" y && install_deps
        fi
        compile
    fi
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    case $(distro) in debian) pam_style=debian ;; *) pam_style=other ;; esac
    msg "$(t "Montando a instalação em $prefix" "Preparing the installation in $prefix")"
    stage "$tmp" "$prefix" "$pam_style"
    # List of what was installed: this is what "uninstall" deletes.
    list=$(manifest_path "$prefix")
    mkdir -p "$tmp$(dirname "$list")"
    (cd "$tmp" && find . ! -type d | sed 's|^\.||' | sort) > "$tmp$list"
    echo "$list" >> "$tmp$list"
    # Also installed as a package: both use the same session entry (and the login
    # would open one version or the other). Better to keep only one.
    if [ -z "$sysroot" ]; then
        pg=$(installed_package)
        if [ -n "$pg" ]; then
            if ask "$(t "O Glove também está instalado por pacote ($pg). Os dois brigam pela mesma sessão. Remover o pacote antes de instalar?" "Glove is also installed as a package ($pg). Both use the same session. Remove the package before installing?")" y; then
                case $pg in
                    deb) root apt-get remove -y glove ;;
                    rpm) root rpm -e glove ;;
                    arch) root pacman -R --noconfirm glove ;;
                esac
            else
                echo "$(t "Aviso: com o pacote instalado, a sessão pode continuar abrindo a versão dele." "Warning: with the package installed, the session may keep running its version.")" >&2
            fi
        fi
    fi
    msg "$(t "Copiando para o sistema (pede a senha)" "Copying to the system (asks for your password)")"
    root mkdir -p "$sysroot/"
    root sh -c "$copy_sh" sh "$tmp" "$sysroot" || die "$(t "a cópia para o sistema falhou" "copying to the system failed")"
    if [ -z "$sysroot" ]; then
        root sh -c "$post_install_sh"
    fi
    echo
    echo "$(t "Glove $version instalado em $prefix. Saia da sessão e escolha \"Glove\" na tela de login." "Glove $version installed in $prefix. Log out and choose \"Glove\" on the login screen.")"
    echo "$(t "Tela de login do Glove: ./glove-setup-tool.sh login (ou sudo systemctl enable --force glove-login)." "Glove login screen: ./glove-setup-tool.sh login (or sudo systemctl enable --force glove-login).")"
    # The GRUB screen: asked once (not again when it already has the Glove image).
    if [ -z "$sysroot" ]; then
        gdir=$(grub_dir)
        if [ -n "$gdir" ] && [ ! -f "$gdir/glove-os.png" ]; then
            echo
            if ask "$(t "Trocar também a tela do GRUB (o menu que aparece ao ligar) pelo papel de parede do Glove OS?" "Also change the GRUB screen (the menu shown at power-on) to the Glove OS wallpaper?")" n; then
                grub_screen yes
            else
                echo "$(t "Para trocar depois: ./glove-setup-tool.sh grub" "To change it later: ./glove-setup-tool.sh grub")"
            fi
        fi
    fi
}

uninstall_glove() {
    # Installed as a package: the package manager is the one that removes it.
    if [ -z "$sysroot" ] && has dpkg-query && dpkg-query -W glove >/dev/null 2>&1; then
        ask "$(t "O Glove foi instalado pelo pacote .deb. Remover com o apt?" "Glove was installed from the .deb package. Remove it with apt?")" y && root apt-get remove -y glove; return
    fi
    if [ -z "$sysroot" ] && has rpm && rpm -q glove >/dev/null 2>&1; then
        ask "$(t "O Glove foi instalado pelo pacote .rpm. Remover?" "Glove was installed from the .rpm package. Remove it?")" y && root rpm -e glove; return
    fi
    if [ -z "$sysroot" ] && has pacman && pacman -Q glove >/dev/null 2>&1; then
        ask "$(t "O Glove foi instalado pelo pacote do Arch. Remover?" "Glove was installed from the Arch package. Remove it?")" y && root pacman -R --noconfirm glove; return
    fi
    if [ -z "$sysroot" ] && ls /var/log/packages/glove-[0-9]* >/dev/null 2>&1; then
        ask "$(t "O Glove foi instalado pelo pacote do Slackware. Remover?" "Glove was installed from the Slackware package. Remove it?")" y && root removepkg glove; return
    fi
    found=""
    for p in /usr/local /usr /opt/glove "$1"; do
        [ -n "$p" ] && [ -f "$sysroot$(manifest_path "$p")" ] && found="$found $p"
    done
    [ -n "$found" ] || die "$(t "não achei uma instalação do Glove feita por este instalador" "no Glove installation made by this tool was found")"
    for p in $found; do
        list="$sysroot$(manifest_path "$p")"
        ask "$(t "Remover o Glove instalado em $p?" "Remove the Glove installed in $p?")" y || continue
        [ -z "$sysroot" ] && root sh -c "$pre_remove_sh"
        msg "$(t "Removendo $(wc -l < "$list") arquivos" "Removing $(wc -l < "$list") files")"
        # Deletes each file in the list, then Glove's own folders.
        root sh -c 'while IFS= read -r f; do [ -n "$f" ] && rm -f "$2$f"; done < "$1"' sh "$list" "$sysroot" || true
        root sh -c 'for d in "$1/share/icons/Glove" "$1/share/doc/glove" "$1/share/glove"; do rm -rf "$d"; done' sh "$sysroot$p"
    done
    # The GRUB screen with the Glove image: back to what it was.
    gdir=$(grub_dir)
    if [ -z "$sysroot" ] && [ -n "$gdir" ] && [ -f "$gdir/glove-os.png" ]; then
        grub_screen remove
    fi
    echo "$(t "Glove removido. Sua configuração pessoal (~/.config/glove) foi mantida." "Glove removed. Your personal settings (~/.config/glove) were kept.")"
}

# --- packages -----------------------------------------------------------------------

# Packages Glove needs to run and the ones that improve it (per distribution).
deps_deb="libgbm1, libseat1, libudev1, libinput10 | libinput-bin, libxkbcommon0, libdrm2, libegl1, libgles2, libgl1-mesa-dri, fontconfig, fonts-dejavu-core, greetd"
rec_deb="xwayland, mpv, poppler-utils, pipewire-bin, wireplumber, xdg-utils, desktop-file-utils, dbus-bin"
deps_rpm="mesa-libEGL mesa-libGLES mesa-dri-drivers fontconfig dejavu-sans-fonts greetd"
rec_rpm="xorg-x11-server-Xwayland mpv poppler-utils pipewire-utils wireplumber xdg-utils desktop-file-utils"
deps_arch="mesa libinput libxkbcommon seatd systemd-libs libdrm fontconfig ttf-dejavu greetd"
opt_arch="xorg-xwayland:X11_apps mpv:Video poppler:PDF_in_the_Viewer pipewire:sound wireplumber:volume xdg-utils:Open_With desktop-file-utils:Open_With"

package_deb() {
    has dpkg-deb || die "$(t "falta o dpkg-deb (pacote dpkg)" "dpkg-deb is missing (dpkg package)")"
    tmp=$(mktemp -d)
    stage "$tmp" /usr debian
    mkdir -p "$tmp/DEBIAN"
    case $arch in x86_64) a=amd64 ;; aarch64) a=arm64 ;; *) a=$arch ;; esac
    cat > "$tmp/DEBIAN/control" <<EOF
Package: glove
Version: $version
Architecture: $a
Maintainer: Fabio Carneiro <$(git config user.email 2>/dev/null || echo glove@localhost)>
Installed-Size: $(size_kb "$tmp")
Depends: $deps_deb
Recommends: $rec_deb
Suggests: plymouth
Section: x11
Priority: optional
Homepage: $url
Description: $summary
$(echo "$description" | sed 's/^/ /')
EOF
    printf '#!/bin/sh\nset -e\n%s\n' "$post_install_sh" > "$tmp/DEBIAN/postinst"
    # Only on a real removal (not on an upgrade).
    printf '#!/bin/sh\nset -e\nif [ "$1" = remove ] || [ "$1" = purge ]; then\n%s\nfi\n' "$pre_remove_sh" > "$tmp/DEBIAN/prerm"
    printf '#!/bin/sh\nset -e\ncommand -v systemctl >/dev/null 2>&1 && systemctl daemon-reload 2>/dev/null || true\n' > "$tmp/DEBIAN/postrm"
    chmod 755 "$tmp/DEBIAN/postinst" "$tmp/DEBIAN/prerm" "$tmp/DEBIAN/postrm"
    mkdir -p "$out_dir"
    f="$out_dir/glove_${version}_$a.deb"
    dpkg-deb --root-owner-group -Zxz --build "$tmp" "$f" >/dev/null
    rm -rf "$tmp"
    msg "$(t "Pacote Debian/Ubuntu: $f" "Debian/Ubuntu package: $f")"
}

# The glove-os package: the identity of the Glove OS distro (name, logo, fastfetch
# and neofetch, GRUB). Only for Glove OS (the ISO): the glove package alone,
# installed on another distro, does not change its name.
package_glove_os() {
    has dpkg-deb || die "$(t "falta o dpkg-deb (pacote dpkg)" "dpkg-deb is missing (dpkg package)")"
    d="$root_dir/data/glove-os"
    tmp=$(mktemp -d)
    o="$tmp/usr/share/glove-os"
    mkdir -p "$o" "$tmp/DEBIAN" "$tmp/etc/xdg/fastfetch" "$tmp/etc/skel/.config/neofetch" "$tmp/etc/apt/apt.conf.d" "$tmp/etc/default/grub.d" "$tmp/usr/share/pixmaps"
    echo "$version" > "$o/versao"
    install -m 755 "$d/identidade" "$o/identidade"
    install -m 644 "$d/logo.txt" "$d/logo-neofetch.txt" "$o/"
    install -m 644 "$d/glove-os.png" "$o/logo.png"
    install -m 644 "$d/fastfetch.jsonc" "$tmp/etc/xdg/fastfetch/config.jsonc"
    install -m 644 "$d/neofetch.conf" "$tmp/etc/skel/.config/neofetch/config.conf"
    install -m 644 "$d/apt-glove-os.conf" "$tmp/etc/apt/apt.conf.d/99glove-os"
    install -m 644 "$d/grub-glove-os.cfg" "$tmp/etc/default/grub.d/glove-os.cfg"
    mkdir -p "$tmp/etc/grub.d"
    install -m 755 "$root_dir/data/grub/06_glove" "$tmp/etc/grub.d/06_glove"
    for f in "$d"/icones/glove-os-*.png; do
        s=$(basename "$f" .png | sed 's/glove-os-//')
        mkdir -p "$tmp/usr/share/icons/hicolor/${s}x${s}/apps"
        install -m 644 "$f" "$tmp/usr/share/icons/hicolor/${s}x${s}/apps/glove-os.png"
    done
    install -m 644 "$d/icones/glove-os-256.png" "$tmp/usr/share/pixmaps/glove-os.png"
    install -m 755 "$d/postinst" "$d/postrm" "$tmp/DEBIAN/"
    (cd "$tmp" && find etc -type f | sed 's|^|/|') > "$tmp/DEBIAN/conffiles"
    {
        echo "Package: glove-os"
        echo "Version: $version"
        echo "Architecture: all"
        echo "Maintainer: Fabio Carneiro <$(git config user.email 2>/dev/null || echo glove@localhost)>"
        echo "Installed-Size: $(size_kb "$tmp")"
        echo "Depends: glove (>= $version), base-files"
        echo "Section: misc"
        echo "Priority: optional"
        echo "Homepage: $url"
        echo "Description: Glove OS: distro identity (Debian inside)"
        echo " Makes the system present itself as Glove OS: the name in os-release (ID=debian"
        echo " and the Debian version stay the same, for programs), logo, fastfetch,"
        echo " neofetch, console screen and GRUB menu. Only for Glove OS."
    } > "$tmp/DEBIAN/control"
    mkdir -p "$out_dir"
    f="$out_dir/glove-os_${version}_all.deb"
    dpkg-deb --root-owner-group -Zxz --build "$tmp" "$f" >/dev/null
    rm -rf "$tmp"
    msg "$(t "Pacote da identidade Glove OS: $f" "Glove OS identity package: $f")"
}

package_slackware() {
    tmp=$(mktemp -d)
    stage "$tmp" /usr other
    mkdir -p "$tmp/install"
    # slack-desc: 11 lines starting with the package name (pkgtools requires the ruler).
    {
        echo "     |-----handy-ruler------------------------------------------------------|"
        echo "glove: glove ($summary)"
        echo "glove:"
        echo "$description" | tr '\n' ' ' | fold -s -w 70 | sed 's/ *$//; s/^/glove: /'
        echo
        echo "glove:"
        echo "glove: Needs seatd and greetd (SlackBuilds.org)."
        echo "glove: Site: $url"
        echo "glove:"
        echo "glove:"
    } | head -12 > "$tmp/install/slack-desc"
    cat > "$tmp/install/doinst.sh" <<'EOF'
if [ -x /usr/bin/gtk-update-icon-cache ]; then
  /usr/bin/gtk-update-icon-cache -q -f usr/share/icons/Glove >/dev/null 2>&1
fi
# Login screen: with systemd, sudo systemctl enable --force glove-login; without
# systemd (the Slackware default), run /usr/lib/glove/glove-login-greetd in place of
# the login manager (in /etc/rc.d/rc.4).
EOF
    mkdir -p "$out_dir"
    f="$out_dir/glove-$version-$arch-1.tgz"
    (cd "$tmp" && tar --owner=0 --group=0 --numeric-owner -czf "$f" .)
    rm -rf "$tmp"
    msg "$(t "Pacote Slackware: $f (instale com: installpkg $(basename "$f"))" "Slackware package: $f (install with: installpkg $(basename "$f"))")"
}

package_rpm() {
    tmp=$(mktemp -d)
    stage "$tmp/stage" /usr other
    mkdir -p "$out_dir"
    spec="$out_dir/glove.spec"
    {
        cat <<EOF
Name:           glove
Version:        $version
Release:        1
Summary:        $summary
License:        GPL-3.0-or-later
URL:            $url
Requires:       $deps_rpm
Recommends:     $(echo "$rec_rpm" | sed 's/ /, /g')
%global debug_package %{nil}
%global __os_install_post %{nil}

%description
$description

%install
cp -a %{_stage}/. %{buildroot}/

%post
$post_install_sh

%preun
if [ \$1 -eq 0 ]; then
$pre_remove_sh
fi

%postun
systemctl daemon-reload 2>/dev/null || :

%files
EOF
        (cd "$tmp/stage" && find . ! -type d | sed 's|^\.||' | sort | sed 's|^\(/etc/.*\)|%config(noreplace) \1|')
        (cd "$tmp/stage" && find ./usr/share/icons/Glove ./usr/share/doc/glove -type d | sed 's|^\.|%dir |' | sort)
    } > "$spec"
    if has rpmbuild; then
        rpmbuild -bb --define "_topdir $tmp/rpm" --define "_stage $tmp/stage" "$spec" >/dev/null
        cp "$tmp"/rpm/RPMS/*/glove-*.rpm "$out_dir/"
        msg "$(t "Pacote Fedora: $(ls "$out_dir"/glove-*.rpm | tail -1)" "Fedora package: $(ls "$out_dir"/glove-*.rpm | tail -1)")"
    else
        msg "$(t "Sem o rpmbuild aqui: deixei a receita em $spec" "No rpmbuild here: the recipe is in $spec")"
        echo "$(t "  No Fedora: sudo dnf install rpm-build; no Debian/Ubuntu: sudo apt install rpm." "  On Fedora: sudo dnf install rpm-build; on Debian/Ubuntu: sudo apt install rpm.")"
        echo "$(t "  Depois rode de novo: ./glove-setup-tool.sh rpm" "  Then run again: ./glove-setup-tool.sh rpm")"
    fi
    rm -rf "$tmp"
}

package_arch() {
    mkdir -p "$out_dir"
    # pacman steps on install, upgrade and remove.
    {
        echo "post_install() {"; echo "$post_install_sh"; echo "}"
        echo "post_upgrade() {"; echo "  command -v systemctl >/dev/null 2>&1 && systemctl daemon-reload 2>/dev/null || true"; echo "}"
        echo "pre_remove() {"; echo "$pre_remove_sh"; echo "}"
        echo "post_remove() {"; echo "  command -v systemctl >/dev/null 2>&1 && systemctl daemon-reload 2>/dev/null || true"; echo "}"
    } > "$out_dir/glove.install"
    # Recipe to build on Arch itself with makepkg (the Arch way).
    cat > "$out_dir/PKGBUILD" <<EOF
# Glove recipe for Arch (made by glove-setup-tool.sh).
pkgname=glove
pkgver=$version
pkgrel=1
pkgdesc="$summary"
arch=('x86_64' 'aarch64')
url="$url"
license=('GPL-3.0-or-later')
depends=($(echo "$deps_arch" | sed "s/[^ ]*/'&'/g"))
optdepends=($(echo "$opt_arch" | tr ' ' '\n' | sed "s/_/ /g; s/^/'/; s/\$/'/; s/:/: /" | tr '\n' ' '))
makedepends=('cargo' 'pkgconf')
install=glove.install
source=("git+$url.git#tag=v$version")
sha256sums=('SKIP')

prepare() {
  cd glove
  git submodule update --init --recursive
  cargo fetch --locked --target "\$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  cd glove
  cargo build --release --frozen
}

package() {
  cd glove
  ./glove-setup-tool.sh _stage "\$pkgdir" /usr other
}
EOF
    if has makepkg; then
        (cd "$out_dir" && makepkg -f)
        msg "$(t "Pacote Arch: $(ls "$out_dir"/glove-*.pkg.tar.zst | tail -1)" "Arch package: $(ls "$out_dir"/glove-*.pkg.tar.zst | tail -1)")"
        return
    fi
    # Outside Arch: builds the package directly (same format makepkg produces).
    has zstd || die "$(t "falta o zstd" "zstd is missing")"
    tmp=$(mktemp -d)
    stage "$tmp" /usr other
    {
        echo "pkgname = glove"
        echo "pkgbase = glove"
        echo "pkgver = $version-1"
        echo "pkgdesc = $summary"
        echo "url = $url"
        echo "builddate = $(date +%s)"
        echo "packager = Glove glove-setup-tool.sh"
        echo "size = $(( $(size_kb "$tmp") * 1024 ))"
        echo "arch = $arch"
        echo "license = GPL-3.0-or-later"
        for d in $deps_arch; do echo "depend = $d"; done
        for d in $opt_arch; do echo "optdepend = $(echo "$d" | sed 's/_/ /g; s/:/: /')"; done
    } > "$tmp/.PKGINFO"
    cp "$out_dir/glove.install" "$tmp/.INSTALL"
    # .MTREE: the file list pacman uses to verify the installation.
    (
        cd "$tmp"
        echo "#mtree"
        echo "/set type=file uid=0 gid=0 mode=644"
        find . -mindepth 1 ! -name .PKGINFO ! -name .INSTALL | sort | while IFS= read -r p; do
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
    chmod 644 "$tmp/.PKGINFO" "$tmp/.MTREE" "$tmp/.INSTALL"
    f="$out_dir/glove-$version-1-$arch.pkg.tar.zst"
    (cd "$tmp" && tar --owner=0 --group=0 --numeric-owner -cf - .PKGINFO .INSTALL .MTREE $(ls -A | grep -v '^\.PKGINFO$\|^\.MTREE$\|^\.INSTALL$') | zstd -19 -q -f -o "$f")
    rm -rf "$tmp"
    msg "$(t "Pacote Arch: $f (instale com: sudo pacman -U $(basename "$f"))" "Arch package: $f (install with: sudo pacman -U $(basename "$f"))")"
    echo "$(t "  No Arch também dá para compilar lá: makepkg -si (com o PKGBUILD em $out_dir)" "  On Arch you can also build it there: makepkg -si (with the PKGBUILD in $out_dir)")"
}

# --- login screen (greetd) -----------------------------------------------------------

# Makes the Glove login screen (the glove-login service, which comes with the
# installation and the packages) the main one, remembering the previous one; "login
# remove" brings it back. Same as: sudo systemctl enable --force glove-login
login() {
    if [ "$(id -u)" != 0 ]; then
        exec sudo "$0" login "$@"
    fi
    state_dir=/etc/glove/login
    # Old versions of this tool replaced greetd's own configuration: put the original
    # back (the glove-login service uses a configuration of its own).
    if [ -f /etc/greetd/config.toml.antes-do-glove ]; then
        mv /etc/greetd/config.toml.antes-do-glove /etc/greetd/config.toml
        systemctl disable greetd 2>/dev/null || true
    fi
    if [ "${1:-}" = remove ] || [ "${1:-}" = remover ]; then
        previous=$(cat "$state_dir/gerenciador-anterior" 2>/dev/null || true)
        systemctl disable glove-login 2>/dev/null || true
        for d in $previous gdm gdm3 lightdm sddm lxdm greetd; do
            if systemctl cat "$d.service" >/dev/null 2>&1 && systemctl enable --force "$d.service" 2>/dev/null; then
                echo "$(t "A tela de login volta a ser a do $d (vale no próximo boot)." "The login screen goes back to $d (from the next boot).")"
                break
            fi
        done
        rm -f "$state_dir/gerenciador-anterior"
        return
    fi
    systemctl cat glove-login.service >/dev/null 2>&1 || die "$(t "o Glove não está instalado (instale o pacote ou rode: ./glove-setup-tool.sh instalar)" "Glove isn’t installed (install the package or run: ./glove-setup-tool.sh install)")"
    if ! has greetd; then
        msg "$(t "Instalando o greetd" "Installing greetd")"
        case $(distro) in
            debian) apt-get install -y greetd ;;
            fedora) dnf install -y greetd ;;
            arch) pacman -S --needed --noconfirm greetd ;;
            suse) zypper install -y greetd ;;
            void) xbps-install -Sy greetd ;;
            alpine) apk add greetd ;;
            *) die "$(t "instale o greetd pelo gerenciador de pacotes e rode de novo" "install greetd with the package manager and run again")" ;;
        esac
    fi
    mkdir -p "$state_dir"
    # The login screen uses the settings of whoever installed it (display, scale, keyboard and theme).
    if [ -n "${SUDO_USER:-}" ]; then
        home_dir=$(getent passwd "$SUDO_USER" | cut -d: -f6)
        if [ -f "$home_dir/.config/glove/config.toml" ]; then
            install -m 644 "$home_dir/.config/glove/config.toml" "$state_dir/config.toml"
            echo "$(t "Tela de login com os ajustes de $SUDO_USER (monitor, escala, teclado e tema)." "Login screen with $SUDO_USER’s settings (display, scale, keyboard and theme).")"
        fi
    fi
    current=$(basename "$(readlink -f /etc/systemd/system/display-manager.service 2>/dev/null)" .service)
    if [ "$current" = glove-login ]; then
        echo "$(t "A tela de login do Glove já é a principal." "The Glove login screen is already the main one.")"
        return
    fi
    echo "$(t "A tela de login atual é ${current:-desconhecida}." "The current login screen is ${current:-unknown}.")"
    if ask "$(t "Trocar pela tela de login do Glove (vale a partir do próximo boot)?" "Switch to the Glove login screen (from the next boot)?")" y; then
        [ -n "$current" ] && echo "$current" > "$state_dir/gerenciador-anterior"
        systemctl enable --force glove-login
        echo "$(t "Pronto: reinicie para ver a tela de login do Glove." "Done: restart to see the Glove login screen.")"
        echo "$(t "Para voltar à do ${current:-sistema}: ./glove-setup-tool.sh login remover" "To go back to ${current:-the system’s}: ./glove-setup-tool.sh login remove")"
    fi
}

# --- boot menu screen (GRUB) ---------------------------------------------------------

# GRUB's folder in /boot ("" when this system does not boot with GRUB).
grub_dir() {
    [ -d "$sysroot/etc/grub.d" ] || return 0
    for d in /boot/grub /boot/grub2; do
        # (On some systems only root can look inside the folder.)
        if [ -f "$sysroot$d/grub.cfg" ] || { [ -d "$sysroot$d" ] && [ ! -r "$sysroot$d" ]; }; then echo "$d"; return 0; fi
    done
}

# Puts the Glove OS wallpaper behind the GRUB menu (the screen where the system to
# start is chosen), with white menu text; "grub remove" undoes it. What it writes:
#   /boot/grub/glove-os.png           the image (in /boot, where GRUB can always read it)
#   /etc/default/grub.d/glove.cfg     GRUB_BACKGROUND (Debian, Ubuntu…), or lines marked
#                                     "# glove-setup-tool" at the end of /etc/default/grub
#   /etc/grub.d/06_glove              the text colors
# "grub yes" does not ask (used right after the question at the end of "install").
grub_screen() {
    if [ "$(id -u)" != 0 ] && [ -z "$sysroot" ]; then
        root "$root_dir/glove-setup-tool.sh" grub "$@"
        return
    fi
    gdir=$(grub_dir)
    [ -n "$gdir" ] || die "$(t "não achei o GRUB neste sistema (ele usa outro carregador de início?)" "GRUB was not found on this system (does it use another bootloader?)")"
    img="$gdir/glove-os.png"
    dropin=/etc/default/grub.d/glove.cfg
    main=/etc/default/grub
    colors=/etc/grub.d/06_glove
    mark="# glove-setup-tool"
    refresh() {
        [ -z "$sysroot" ] || return 0
        msg "$(t "Refazendo o menu do GRUB" "Rebuilding the GRUB menu")"
        if has update-grub; then update-grub
        elif has grub-mkconfig; then grub-mkconfig -o "$gdir/grub.cfg"
        elif has grub2-mkconfig; then grub2-mkconfig -o "$gdir/grub.cfg"
        else die "$(t "não achei o update-grub nem o grub-mkconfig" "neither update-grub nor grub-mkconfig was found")"; fi
    }
    case "${1:-}" in
        remove|remover)
            rm -f "$sysroot$img" "$sysroot$dropin" "$sysroot$colors"
            if [ -f "$sysroot$main" ]; then sed -i "/$mark\$/d" "$sysroot$main"; fi
            refresh
            echo "$(t "A tela do GRUB voltou a ser a de antes." "The GRUB screen is back to what it was.")"
            return ;;
    esac
    src=""
    for f in "$root_dir/data/papeis/glove-os.png" /usr/local/share/glove/papeis/glove-os.png /usr/share/glove/papeis/glove-os.png; do
        if [ -f "$f" ]; then src=$f; break; fi
    done
    [ -n "$src" ] || die "$(t "não achei o papel de parede do Glove OS (glove-os.png)" "the Glove OS wallpaper (glove-os.png) was not found")"
    if [ "${1:-}" != yes ]; then
        ask "$(t "Trocar a tela do GRUB (o menu que aparece ao ligar) pelo papel de parede do Glove OS?" "Change the GRUB screen (the menu shown at power-on) to the Glove OS wallpaper?")" y || return 0
    fi
    install -m 644 "$src" "$sysroot$img"
    install -m 755 "$root_dir/data/grub/06_glove" "$sysroot$colors"
    lines="GRUB_BACKGROUND=\"$img\""
    if [ -f "$sysroot$main" ]; then
        sed -i "/$mark\$/d" "$sysroot$main"
        # A theme hides the background image: off while the Glove image is in place.
        if grep -q '^GRUB_THEME=' "$sysroot$main"; then
            lines="$lines
GRUB_THEME=\"\""
            echo "$(t "O tema do GRUB configurado em $main fica desligado enquanto a imagem do Glove estiver ativa." "The GRUB theme set in $main stays off while the Glove image is in place.")"
        fi
        # Text-only menu (Fedora's default): the image needs the graphical one.
        if grep -q '^GRUB_TERMINAL_OUTPUT=.*console' "$sysroot$main"; then
            lines="$lines
GRUB_TERMINAL_OUTPUT=\"gfxterm\""
        fi
        if grep -q '^GRUB_TERMINAL=.*console' "$sysroot$main"; then
            echo "$(t "Aviso: $main tem GRUB_TERMINAL=console (menu só de texto); a imagem só aparece sem essa linha." "Warning: $main has GRUB_TERMINAL=console (text-only menu); the image only shows without that line.")" >&2
        fi
    fi
    mkconfig=$(command -v grub-mkconfig 2>/dev/null || command -v grub2-mkconfig 2>/dev/null || true)
    if [ -d "$sysroot/etc/default/grub.d" ] || { [ -n "$mkconfig" ] && grep -q 'default/grub\.d' "$mkconfig"; }; then
        mkdir -p "$sysroot/etc/default/grub.d"
        {
            echo "# Glove: the Glove OS wallpaper behind the GRUB menu."
            echo "# Written by glove-setup-tool.sh; undo with: ./glove-setup-tool.sh grub remove"
            echo "$lines"
        } > "$sysroot$dropin"
    else
        echo "$lines" | sed "s/\$/ $mark/" >> "$sysroot$main"
    fi
    refresh
    echo "$(t "Pronto: o papel de parede do Glove OS aparece no menu do GRUB no próximo início." "Done: the Glove OS wallpaper shows in the GRUB menu on the next boot.")"
    echo "$(t "Para desfazer: ./glove-setup-tool.sh grub remover" "To undo: ./glove-setup-tool.sh grub remove")"
}

# --- Glove live ISO (Debian) ----------------------------------------------------------
# Downloads Debian's official live-build and debootstrap (the live-build from other
# distributions is usually too old) into iso/ferramentas, builds the image in
# iso/build and leaves the ISO in iso/. The iso/ folder stays out of Git.

debian_suite=${GLOVE_DEBIAN:-trixie}
# Debian mirror: GLOVE_MIRROR (GLOVE_ESPELHO, the old name, still works).
mirror=${GLOVE_MIRROR:-${GLOVE_ESPELHO:-http://deb.debian.org/debian}}

download() {
    if has curl; then curl -fsSL "$1" -o "$2"; elif has wget; then wget -q "$1" -O "$2"; else die "$(t "falta o curl ou o wget" "curl or wget is missing")"; fi
}

# Downloads the newest version (no backports) of a Debian "all" package and extracts it into $2.
fetch_tool() {
    name=$1; dest=$2
    [ -d "$dest/usr" ] && return 0
    folder="$mirror/pool/main/$(printf %s "$name" | cut -c1)/$name/"
    list=$(mktemp)
    download "$folder" "$list"
    deb=$(grep -o "${name}_[^\"]*_all.deb" "$list" | grep -v '~bpo' | sort -uV | tail -1)
    rm -f "$list"
    [ -n "$deb" ] || die "$(t "não achei o $name em $folder" "could not find $name in $folder")"
    msg "$(t "Baixando $deb" "Downloading $deb")"
    download "$folder$deb" "$dest.deb"
    mkdir -p "$dest"
    dpkg-deb -x "$dest.deb" "$dest"
    rm -f "$dest.deb"
}

build_iso() {
    case $arch in x86_64) ;; *) die "$(t "a ISO é só para x86_64 (amd64)" "the ISO is x86_64 (amd64) only")" ;; esac
    has dpkg-deb || die "$(t "falta o dpkg-deb (pacote dpkg)" "dpkg-deb is missing (dpkg package)")"
    has sudo || die "$(t "falta o sudo" "sudo is missing")"
    package_deb
    package_glove_os
    base="$root_dir/iso"
    tools_dir="$base/ferramentas"
    mkdir -p "$tools_dir"
    fetch_tool live-build "$tools_dir/live-build"
    fetch_tool debootstrap "$tools_dir/debootstrap"
    # The tree live-build uses outside the system (like its source tree).
    lbt="$tools_dir/lb"
    if [ ! -e "$lbt/scripts/build.sh" ]; then
        mkdir -p "$lbt"
        ln -sfn ../live-build/usr/lib/live "$lbt/scripts"
        ln -sfn ../live-build/usr/share/live/build "$lbt/share"
        ln -sfn ../live-build/usr/share/live/build/functions "$lbt/functions"
        ln -sfn ../live-build/usr/share/live/build/data "$lbt/data"
    fi
    export PATH="$tools_dir/live-build/usr/bin:$tools_dir/debootstrap/usr/sbin:$PATH"
    export LIVE_BUILD="$lbt"
    # Its version comes from /usr/share; outside the system, read it from the downloaded tree.
    sed -i 's#cat /usr/share/live/build/VERSION#cat "${LIVE_BUILD}/share/VERSION"#' "$tools_dir/live-build/usr/share/live/build/functions/common.sh"
    # The boot menu with the Glove OS name (instead of "Live system").
    sed -i 's/"Live system/"Glove OS/g' "$tools_dir/live-build/usr/lib/live/build/binary_grub_cfg"
    sed -i 's/Live system/Glove OS/g' "$tools_dir/live-build/usr/share/live/build/bootloaders/syslinux_common/live.cfg.in"
    export DEBOOTSTRAP_DIR="$tools_dir/debootstrap/usr/share/debootstrap"
    build_dir="$base/build"
    # A previous build: start clean (the downloaded packages stay in the cache).
    if [ -d "$build_dir" ]; then
        (cd "$build_dir" && sudo env PATH="$PATH" LIVE_BUILD="$LIVE_BUILD" DEBOOTSTRAP_DIR="$DEBOOTSTRAP_DIR" lb clean) || true
    fi
    mkdir -p "$build_dir"
    cd "$build_dir"
    msg "$(t "Configurando a imagem (Debian $debian_suite)" "Configuring the image (Debian $debian_suite)")"
    lb config \
        --distribution "$debian_suite" \
        --archive-areas "main contrib non-free non-free-firmware" \
        --mirror-bootstrap "$mirror" --mirror-chroot "$mirror" --mirror-binary "$mirror" \
        --debian-installer none \
        --binary-images iso-hybrid \
        --apt-recommends false \
        --firmware-chroot true \
        --memtest none \
        --iso-application "Glove OS" --iso-publisher "Glove OS - $url" --iso-volume "Glove OS $version" \
        --bootappend-live "boot=live components quiet splash username=glove user-fullname=Glove hostname=glove-os locales=pt_BR.UTF-8 keyboard-layouts=br"
    cp -r "$root_dir/data/iso/package-lists/." config/package-lists/
    mkdir -p config/hooks/normal config/packages.chroot
    cp -r "$root_dir/data/iso/hooks/normal/." config/hooks/normal/
    rm -f config/packages.chroot/glove_*.deb config/packages.chroot/glove-os_*.deb
    cp "$out_dir/glove_${version}_amd64.deb" "$out_dir/glove-os_${version}_all.deb" config/packages.chroot/
    # Boot menu (BIOS and UEFI): the Glove OS wallpaper as the background image, and
    # the menu placed below the glove (live-build copies config/bootloaders over its own).
    mkdir -p config/includes.binary config/bootloaders
    cp -r "$root_dir/data/iso/includes.binary/." config/includes.binary/
    cp -r "$root_dir/data/iso/bootloaders/." config/bootloaders/
    msg "$(t "Montando a ISO (pede a senha do sudo; leva de 15 a 40 minutos e baixa uns 1,5 GB)" "Building the ISO (asks for the sudo password; takes 15 to 40 minutes and downloads about 1.5 GB)")"
    sudo env PATH="$PATH" LIVE_BUILD="$LIVE_BUILD" DEBOOTSTRAP_DIR="$DEBOOTSTRAP_DIR" lb build
    iso=$(ls -t ./*.iso 2>/dev/null | head -1)
    [ -n "$iso" ] || die "$(t "a ISO não foi gerada; veja $build_dir/build.log" "the ISO was not built; see $build_dir/build.log")"
    final="$base/glove-os-${version}-amd64.iso"
    sudo mv "$iso" "$final"
    sudo chown "$(id -u):$(id -g)" "$final"
    cd "$root_dir"
    msg "$(t "ISO pronta: $final" "ISO ready: $final")"
    echo "$(t "Para testar numa máquina virtual: ./glove-setup-tool.sh iso testar" "To try it in a virtual machine: ./glove-setup-tool.sh iso test")"
    echo "$(t "Para gravar num pendrive (apaga o pendrive!): sudo dd if=$final of=/dev/sdX bs=4M status=progress oflag=sync" "To write it to a USB stick (erases the stick!): sudo dd if=$final of=/dev/sdX bs=4M status=progress oflag=sync")"
}

# Starts the ISO in a virtual machine (QEMU), with a 32 GB virtual disk to try the installer.
test_iso() {
    final=$(ls -t "$root_dir"/iso/glove-os-*-amd64.iso "$root_dir"/iso/glove-*-amd64.iso 2>/dev/null | head -1)
    [ -n "$final" ] || die "$(t "nenhuma ISO em iso/; rode ./glove-setup-tool.sh iso" "no ISO in iso/; run ./glove-setup-tool.sh iso")"
    has qemu-system-x86_64 || die "$(t "falta o QEMU (sudo apt install qemu-system-x86 ovmf)" "QEMU is missing (sudo apt install qemu-system-x86 ovmf)")"
    disk="$root_dir/iso/disco-teste.qcow2"
    [ -f "$disk" ] || qemu-img create -f qcow2 "$disk" 32G >/dev/null
    uefi=""
    for f in /usr/share/OVMF/OVMF_CODE_4M.fd /usr/share/OVMF/OVMF_CODE.fd /usr/share/ovmf/OVMF.fd; do
        if [ -f "$f" ]; then
            vars="$root_dir/iso/ovmf-vars.fd"
            [ -f "$vars" ] || cp "$(dirname "$f")/$(basename "$f" | sed 's/CODE/VARS/')" "$vars" 2>/dev/null || true
            if [ -f "$vars" ]; then uefi="-drive if=pflash,format=raw,readonly=on,file=$f -drive if=pflash,format=raw,file=$vars"; else uefi="-bios $f"; fi
            break
        fi
    done
    kvm=""
    [ -w /dev/kvm ] && kvm="-enable-kvm -cpu host"
    msg "$(t "Ligando a máquina virtual (feche a janela para desligar)" "Starting the virtual machine (close the window to turn it off)")"
    # shellcheck disable=SC2086
    qemu-system-x86_64 $kvm -m 4096 -smp 2 $uefi -drive file="$disk",if=virtio -cdrom "$final" -boot menu=on -vga virtio -display gtk,gl=off -device intel-hda -device hda-duplex -nic user
}

show_help() {
    if [ "$lang" = pt ]; then cat <<'END'
Ferramenta de instalação do Glove (qualquer Linux)

  ./glove-setup-tool.sh                      menu
  ./glove-setup-tool.sh instalar [PREFIXO]   compila e instala (padrão: /usr/local)
  ./glove-setup-tool.sh desinstalar          o inverso de instalar
  ./glove-setup-tool.sh deb|slackware|rpm|arch|todos   gera os pacotes em pacotes/
  ./glove-setup-tool.sh dependencias         instala o que falta para compilar
  ./glove-setup-tool.sh login [remover]      torna a tela de login do Glove a principal
                                             (= sudo systemctl enable --force glove-login)
  ./glove-setup-tool.sh grub [remover]       põe o papel de parede do Glove OS na tela do GRUB
                                             (o menu que aparece ao ligar o computador)
  ./glove-setup-tool.sh iso                  monta a ISO do Glove ao vivo (Debian) em iso/
  ./glove-setup-tool.sh iso testar           liga a ISO numa máquina virtual (QEMU)

O pacote e o "instalar" trazem tudo, inclusive a tela de login (serviço glove-login,
sobre o greetd). Detalhes: README.md, seção "Instalar no sistema".
Para testar sem instalar nada: ./install.sh (usa os programas do projeto).
END
    else cat <<'END'
Glove setup tool (any Linux)

  ./glove-setup-tool.sh                      menu
  ./glove-setup-tool.sh install [PREFIX]     build and install (default: /usr/local)
  ./glove-setup-tool.sh uninstall            the reverse of install
  ./glove-setup-tool.sh deb|slackware|rpm|arch|all   build packages into pacotes/
  ./glove-setup-tool.sh deps                 install what's missing to build
  ./glove-setup-tool.sh login [remove]       make the Glove login screen the main one
                                             (= sudo systemctl enable --force glove-login)
  ./glove-setup-tool.sh grub [remove]        put the Glove OS wallpaper on the GRUB screen
                                             (the menu shown when the computer starts)
  ./glove-setup-tool.sh iso                  build the Glove live ISO (Debian) into iso/
  ./glove-setup-tool.sh iso test             start the ISO in a virtual machine (QEMU)

The package and "install" bring everything, including the login screen (the glove-login
service, on top of greetd). Details: README.en.md, "Installing" section.
To try it without installing anything: ./install.sh (uses the project's programs).
END
    fi
}

# --- menu ---------------------------------------------------------------------------

menu() {
    echo "$(t "Glove $version — instalador ($(distro), $arch)" "Glove $version — setup tool ($(distro), $arch)")"
    echo
    echo "$(t "  1) Instalar (compila e instala em /usr/local)" "  1) Install (builds and installs in /usr/local)")"
    echo "$(t "  2) Desinstalar" "  2) Uninstall")"
    echo "$(t "  3) Pacote .deb (Debian, Ubuntu, Mint…)" "  3) .deb package (Debian, Ubuntu, Mint…)")"
    echo "$(t "  4) Pacote .tgz (Slackware)" "  4) .tgz package (Slackware)")"
    echo "$(t "  5) Pacote .rpm (Fedora, openSUSE…)" "  5) .rpm package (Fedora, openSUSE…)")"
    echo "$(t "  6) Pacote do Arch (.pkg.tar.zst e PKGBUILD)" "  6) Arch package (.pkg.tar.zst and PKGBUILD)")"
    echo "$(t "  7) Todos os pacotes" "  7) All packages")"
    echo "$(t "  8) Instalar o que falta para compilar" "  8) Install what’s missing to build")"
    echo "$(t "  9) Tela de login do Glove (greetd)" "  9) Glove login screen (greetd)")"
    echo "$(t " 10) ISO do Glove ao vivo (Debian)" " 10) Glove live ISO (Debian)")"
    echo "$(t " 11) Tela do GRUB com o papel de parede do Glove OS" " 11) GRUB screen with the Glove OS wallpaper")"
    echo "$(t "  0) Sair" "  0) Quit")"
    printf '%s ' "$(t 'Escolha:' 'Choose:')"
    read -r op || op=0
    case "$op" in
        1) install_glove ;;
        2) uninstall_glove ;;
        3) package_deb ;;
        4) package_slackware ;;
        5) package_rpm ;;
        6) package_arch ;;
        7) package_deb; package_slackware; package_rpm; package_arch ;;
        8) install_deps ;;
        9) login ;;
        10) build_iso ;;
        11) grub_screen ;;
        *) exit 0 ;;
    esac
}

case "${1:-}" in
    "") menu ;;
    # English commands; the Portuguese names keep working.
    install|instalar) install_glove "$2" ;;
    uninstall|remove|desinstalar|remover) uninstall_glove "$2" ;;
    deb) package_deb ;;
    glove-os) package_glove_os ;;
    slackware|slack|tgz) package_slackware ;;
    rpm|fedora) package_rpm ;;
    arch|pacman) package_arch ;;
    all|packages|todos|pacotes) package_deb; package_slackware; package_rpm; package_arch ;;
    deps|dependencies|dependencias) install_deps ;;
    login) shift; login "$@" ;;
    grub|bootloader) shift; grub_screen "$@" ;;
    iso) case "${2:-}" in test|testar) test_iso ;; *) build_iso ;; esac ;;
    _stage|_montar) stage "$2" "$3" "$4" ;;
    -h|--help|help|ajuda) show_help ;;
    *) die "$(t "opção desconhecida: $1 (veja ./glove-setup-tool.sh ajuda)" "unknown option: $1 (see ./glove-setup-tool.sh help)")" ;;
esac
