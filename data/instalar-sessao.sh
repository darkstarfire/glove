#!/bin/sh
# Põe o Glove na lista de sessões da tela de login, usando os arquivos deste projeto
# (sem copiar binários: basta compilar de novo para atualizar).
#   sudo ./data/instalar-sessao.sh            instala
#   sudo ./data/instalar-sessao.sh --remover  tira da lista
set -e
dir="$(cd "$(dirname "$0")" && pwd)"
dest=/usr/share/wayland-sessions/glove.desktop
pam=/etc/pam.d/glove
lancador=/usr/local/bin/glove-session
if [ "$(id -u)" != 0 ]; then
    echo "Rode com sudo." >&2
    exit 1
fi
if [ "$1" = "--remover" ]; then
    rm -f "$dest" "$pam" "$lancador"
    echo "Glove removido da tela de login."
    exit 0
fi
if [ ! -x "$dir/../target/release/glove" ]; then
    echo "Compile antes: cargo build --release" >&2
    exit 1
fi
# A tela de login roda como outro usuário (ex.: _greetd), que não entra na pasta pessoal:
# um Exec apontando para lá some da lista (ou falha). O lançador fica num lugar que
# todos enxergam e só chama o script do projeto, que roda já como o usuário da sessão.
printf '#!/bin/sh\n# Gerado por data/instalar-sessao.sh: inicia a sessão do Glove do projeto.\nexec "%s/glove-session" "$@"\n' "$dir" > "$lancador"
chmod 755 "$lancador"
sed "s|^Exec=.*|Exec=$lancador|" "$dir/glove.desktop" > "$dest"
chmod 644 "$dest"
# Serviço do PAM para desbloquear a tela (só confere a senha). Sem ele, o Glove usa o "login".
if [ -f /etc/pam.d/common-auth ]; then
    # Debian, Ubuntu e derivados
    printf '# Desbloqueio da tela do Glove\nauth    optional  pam_faildelay.so delay=2000000\n@include common-auth\n@include common-account\n' > "$pam"
elif [ -f /etc/pam.d/system-auth ]; then
    # Fedora, Arch e outros
    printf '# Desbloqueio da tela do Glove\nauth     include  system-auth\naccount  include  system-auth\n' > "$pam"
fi
[ -f "$pam" ] && chmod 644 "$pam"
echo "Pronto: saia da sessão e escolha \"Glove\" na tela de login (ícone de engrenagem)."
