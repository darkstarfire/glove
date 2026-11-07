#!/bin/sh
# Instalador do Glove: compila (se precisar) e pergunta o que instalar.
#   ./instalar.sh            pergunta passo a passo
#   ./instalar.sh --remover  tira o Glove da tela de login (e devolve a anterior)
# Os passos que mexem no sistema pedem a senha (sudo) só quando escolhidos.
# Este é o modo de desenvolvimento (a sessão usa os programas do projeto). Para
# instalar no sistema ou gerar pacotes .deb/.rpm/Slackware/Arch: ./instalador.sh
set -e
cd "$(dirname "$0")"
raiz=$(pwd)

pergunta() {
    # pergunta "texto" s|n  -> 0 = sim
    padrao=$2
    if [ "$padrao" = s ]; then op="[S/n]"; else op="[s/N]"; fi
    printf '%s %s ' "$1" "$op"
    read -r r || r=""
    [ -z "$r" ] && r=$padrao
    case "$r" in s|S|sim|Sim|y|Y) return 0 ;; *) return 1 ;; esac
}

if [ "$(id -u)" = 0 ]; then
    echo "Rode sem sudo: o instalador pede a senha só nos passos que precisam." >&2
    exit 1
fi

if [ "$1" = "--remover" ]; then
    if pergunta "Tirar o Glove da lista de sessões?" s; then
        sudo ./data/instalar-sessao.sh --remover
    fi
    if [ -f /etc/greetd/config.toml.antes-do-glove ] && pergunta "Devolver a tela de login que havia antes do Glove?" s; then
        sudo ./data/instalar-login.sh --remover
    fi
    exit 0
fi

echo "== Glove =="
if [ ! -x target/release/glove ] || [ ! -x target/release/glove-login ]; then
    if ! command -v cargo >/dev/null 2>&1; then
        echo "Precisa do Rust (cargo) para compilar: https://rustup.rs" >&2
        exit 1
    fi
    if pergunta "O Glove ainda não foi compilado. Compilar agora (alguns minutos)?" s; then
        ./compilar.sh
    else
        exit 1
    fi
elif pergunta "Recompilar antes de instalar (pega as mudanças mais novas)?" n; then
    ./compilar.sh
fi

# Programas de fora que os apps do Glove usam (só oferece os que faltam).
faltam=""
for par in mpv:mpv pdftoppm:poppler-utils Xwayland:xwayland pw-play:pipewire-bin wpctl:wireplumber xdg-mime:xdg-utils update-desktop-database:desktop-file-utils fc-cache:fontconfig dbus-update-activation-environment:dbus-bin; do
    cmd=${par%%:*}
    pacote=${par#*:}
    command -v "$cmd" >/dev/null 2>&1 || faltam="$faltam $pacote"
done
if [ -n "$faltam" ]; then
    echo
    echo "Faltam programas que alguns apps usam:$faltam"
    echo "  (mpv: Vídeo; poppler-utils: PDF no Visualizador; xwayland: apps X11;"
    echo "   pipewire/wireplumber: som; xdg-utils e desktop-file-utils: \"Abrir com\")"
    if command -v apt-get >/dev/null 2>&1 && pergunta "Instalar pelo apt?" s; then
        # shellcheck disable=SC2086
        sudo apt-get install -y $faltam || echo "Alguns pacotes não foram instalados; o resto continua."
    fi
fi

echo
echo "1) Sessão: põe \"Glove\" na lista de ambientes da tela de login"
echo "   (usa os programas deste projeto: recompilar já atualiza)."
if pergunta "Instalar a sessão do Glove?" s; then
    sudo ./data/instalar-sessao.sh
fi

echo
echo "2) Tela de login do Glove (greetd): lista de usuários com foto, senha e"
echo "   escolha do ambiente (Glove, GNOME…). Guarda a configuração anterior."
if pergunta "Configurar a tela de login do Glove?" n; then
    if ! command -v greetd >/dev/null 2>&1; then
        if command -v apt-get >/dev/null 2>&1 && pergunta "O greetd não está instalado. Instalar pelo apt?" s; then
            sudo apt-get install -y greetd
        else
            echo "Instale o greetd e rode de novo." >&2
            exit 1
        fi
    fi
    sudo ./data/instalar-login.sh
    if ! systemctl is-enabled greetd >/dev/null 2>&1; then
        atual=""
        for g in gdm gdm3 lightdm sddm lxdm; do
            if systemctl is-enabled "$g" >/dev/null 2>&1; then atual=$g; break; fi
        done
        echo "O gerenciador de login atual é ${atual:-desconhecido}."
        if [ -n "$atual" ] && pergunta "Trocar o $atual pelo greetd (vale a partir do próximo boot)?" n; then
            sudo systemctl disable "$atual"
            sudo systemctl enable greetd
            echo "Para voltar: ./instalar.sh --remover && sudo systemctl disable greetd && sudo systemctl enable $atual"
        fi
    fi
fi

if command -v plymouth >/dev/null 2>&1; then
    echo
    echo "3) Tela de início e desligamento (Plymouth) com a arte do Glove."
    if pergunta "Instalar o tema do Plymouth (refaz o initramfs)?" n; then
        [ -d data/plymouth/glove ] || ./target/release/glove-arte plymouth data/plymouth/glove
        sudo ./data/instalar-plymouth.sh
    fi
fi

echo
echo "Pronto. Saia da sessão e escolha \"Glove\" na tela de login."
echo "Para testar numa janela, sem sair: $raiz/target/release/glove"
