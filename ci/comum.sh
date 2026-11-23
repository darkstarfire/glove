# Trechos comuns aos scripts que geram os pacotes das outras distribuições
# (ci/fedora.sh, ci/arch.sh, ci/slackware.sh). Para ser lido com ". ci/comum.sh".

# O Rust pelo rustup: a mesma versão em todas as distribuições (a de algumas é mais
# velha que o mínimo do Glove). Sem o curl (Slackware mínimo), baixa o rustup-init direto.
instalar_rust() {
    if ! command -v cargo >/dev/null 2>&1; then
        if command -v curl >/dev/null 2>&1; then
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
        else
            wget -q -O /tmp/rustup-init "https://static.rust-lang.org/rustup/dist/$(uname -m)-unknown-linux-gnu/rustup-init"
            chmod +x /tmp/rustup-init
            /tmp/rustup-init -y --profile minimal
        fi
    fi
    . "$HOME/.cargo/env"
}

# Depois de instalar o pacote gerado: os arquivos estão no lugar e os programas acham
# todas as bibliotecas de que precisam (um pacote com dependência errada falha aqui, e
# não no computador de quem baixou).
conferir() {
    for f in /usr/bin/glove /usr/bin/glove-apps /usr/bin/glove-portal /usr/bin/glove-escolher \
             /usr/bin/glove-session /usr/bin/glove-files /usr/share/wayland-sessions/glove.desktop \
             /usr/share/icons/Glove/index.theme /usr/share/glove/papeis/glove-os.png \
             /usr/share/glove/glove-os-repositorio.asc /etc/pam.d/glove; do
        [ -e "$f" ] || { echo "ERRO: o pacote não instalou $f"; exit 1; }
    done
    for b in glove glove-apps glove-portal glove-escolher; do
        falta=$(ldd "/usr/bin/$b" | grep "not found" || true)
        [ -z "$falta" ] || { echo "ERRO: /usr/bin/$b não acha:"; echo "$falta"; exit 1; }
    done
    echo "Pacote conferido: instalado, com todas as bibliotecas."
    ls -l pacotes/
}

# O pacote da Jangada (o navegador, repositório próprio) da versão fixada no
# glove-setup-tool.sh, baixado da release dela para pacotes/: entra no repositório desta
# distribuição ao lado do Glove (o dnf e o pacman o acham sem configurar outro).
#   baixar_jangada NOME   (NOME com {v} no lugar da versão)
baixar_jangada() {
    jv=$(sed -n 's/^jangada_version=//p' glove-setup-tool.sh | head -1)
    nome=$(echo "$1" | sed "s/{v}/$jv/g")
    url="https://github.com/fabioaacarneiro/jangada/releases/download/v$jv/$nome"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --retry 3 --max-time 900 "$url" -o "pacotes/$nome"
    else
        wget -q -O "pacotes/$nome" "$url"
    fi
    ls -l "pacotes/$nome"
}
