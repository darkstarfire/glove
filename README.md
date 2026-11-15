# Glove

*Read in English: [README.en.md](README.en.md)*

Ambiente gráfico leve para Linux, escrito em Rust, com visual inspirado na era
"gel" dos anos 2000: botões translúcidos, listras finas, metal escovado, sombras
suaves e um Dock com ampliação. Toda a arte é original e desenhada por código; as únicas imagens
prontas são a logo da luva e o papel de parede do Glove OS.

## Glove OS: uma distro brasileira

O **Glove OS** é uma distribuição Linux **feita no Brasil**. Por dentro, é Debian 13: os mesmos
pacotes, repositórios e atualizações, 100% compatível. Por fora, é o Glove: uma área de trabalho
escrita do zero em Rust (compositor Wayland, biblioteca de interface e 38 apps próprios), com o
visual "gel" dos anos 2000 e identidade própria, do menu de boot ao `fastfetch`. O português é a
língua de origem: os textos nascem em português no código, e o inglês vem por tradução.

Ela foi criada por um **engenheiro de software brasileiro com mais de 22 anos de estrada no Linux**
que estava entediado e quis praticar IA em "lugares diferentes": em vez de mais um site ou mais um
app, um sistema inteiro, com compositor, desenho de cada pixel, instalador e ISO, construído em
parceria com uma IA (Claude).

**Baixar:** a ISO de cada versão fica na página de [Releases](https://github.com/fabioaacarneiro/glove/releases)
(veja [Glove OS (a distro, em ISO)](#glove-os-a-distro-em-iso)).

## Estado atual

Base visual e de comportamento: **Mac OS X 10.4 Tiger** (com o tema Listras para o visual 10.0–10.2).
Fases 1 a 15 do [plano](docs/PLANO.md) feitas: a era inteira (1 a 9.16, com a interface em **português e
inglês**), a Fase 10 (além da era), o instalador e a ISO do **Glove OS** (11), o editor de documentos (12),
computadores mais simples (13), captura com desenho (14) e papel de parede moderno (15). Ficam em aberto
13.3, 13.4 e 14.3, e o instalador e a ISO ainda precisam ser testados em computadores de verdade. Como usar
cada recurso: [docs/USO.md](docs/USO.md).

**Sistema (compositor `glove`)**

- Compositor Wayland próprio (Smithay), em **sessão própria** (DRM/KMS + libinput) ou **dentro de uma janela**
- **Vários monitores**: estender, espelhar ou só um; resolução e escala por tela; nítido em HiDPI e escala fracionada
- **Apps X11** pelo Xwayland, com barra de título do Glove
- Barra de menu global: menu principal (Itens Recentes, Forçar Encerrar, Repouso, Desligar…),
  menu do app, os **menus de cada app**, submenus e navegação pelo teclado; extras de Wi-Fi,
  volume, bateria (com tempo restante, saúde e ciclos, também em baterias paralelas e Chromebooks em que o firmware não reconhece a bateria: o serviço `glove-bateria` a lê direto do EC e desliga o computador antes de ela zerar) e monitor, relógio e nome do usuário (troca rápida de usuário)
- **Dock** de vidro com ampliação, ícones que pulam ao abrir, menu de contexto com bico, ocultar
  sozinho, posição embaixo/esquerda/direita, efeitos de minimizar **Escala** e **Lâmpada Mágica**
  (deformação feita na placa de vídeo), miniaturas vivas e Lixo cheio/vazio
- Janelas com sombra, fechar/minimizar/zoom, arrastar (inclusive entre monitores, encolhendo para
  caber) e redimensionar; **Super+Tab** com a faixa de ícones do Tiger; **Exposé** e cantos ativos
- **Mesa como o Arquivos** do Tiger: os itens da pasta Mesa e os discos, com menus, arrastar (pela
  mesa, para pastas, para o Lixo, para janelas e das janelas para a mesa), renomear no lugar,
  Organizar por, Limpar e Opções de Visualização (Ctrl+J); com a mesa em foco a barra mostra os
  menus do Arquivos
- **Painel de Widgets** (F12): Relógio, Calendário, Calculadora, Clima (Open-Meteo), Lembrete,
  Monitor e Dicionário sobre a tela escurecida, com a barra de widgets do "+"
- **Todos os Apps** (F4 ou o foguete no Dock): todos os apps instalados em tela cheia, em páginas, sobre
  o papel de parede desfocado, com busca, pastas (a Utilitários e as que você criar) e arrumação
  arrastando (segure um ícone até todos balançarem)
- **Papel de parede** igual em todos os monitores, um por monitor ou em **panorama** (uma imagem
  atravessando os monitores, lado a lado ou empilhados), e **animado** (um vídeo, tocado numa cópia leve
  guardada em cache e pausado com a tela bloqueada, na bateria ou com app em tela cheia)
- **Mesas virtuais** (Ctrl+Super+setas; com Shift a janela vai junto), também na faixa do Exposé, onde dá
  para arrastar janelas de uma mesa para outra
- **Central de avisos**: balões das notificações dos apps (o serviço padrão do Linux, o do `notify-send`),
  a central no balãozinho da barra de menu e Não perturbe
- **Tela cheia** (Ctrl+Super+F) e **gestos** de três e quatro dedos no touchpad
- **Busca Rápida** (Super+Espaço): apps instalados, Preferências e arquivos
- **Captura** de tela inteira, área ou uma janela (clique), em qualquer monitor
- Forçar Encerrar com detecção de app que não responde; bloqueio de tela (PAM), tela que apaga,
  tampa do notebook, teclas de volume e brilho; troca rápida de usuário
- **Senha do sistema** (`glove-senha`, o agente do polkit): qualquer programa que pede permissão de
  administrador pelo polkit (Discos, gnome-disks, Flatpak, NetworkManager, `pkexec`) mostra a janela
  de senha do Glove
- Acesso Universal: zoom, cores invertidas, cursor grande
- Arrastar e soltar entre apps, área de transferência de texto e de arquivos
- Janela Abrir/Salvar do Glove também para outros apps (portal FileChooser: Firefox, Chromium…)
- Temas **Claro** (padrão, Tiger), **Listras**, **Metal** e **Grafite**; ícones pelo tema **Glove** em
  arquivos PNG (o padrão, gerado dos desenhos do Glove e publicado no glove-icons; mais leve que
  desenhar na hora) ou outro tema do sistema
- **Reduzir efeitos** (Preferências ▸ Aparência), para computadores mais simples
- Redesenha só o que mudou: 0% de CPU parado

**Sessão e login**

- Tela de login própria pelo greetd (`glove-login`), com escolha da sessão; splash do Plymouth
  desenhado por código; assistente de primeiro uso; Desligar/Reiniciar com contagem

**Apps (todos com a glove-ui e arte própria)**

| App | O que faz |
|---|---|
| Arquivos | Finder do Tiger: metal, barra lateral, colunas/lista/ícones, prévia com zoom, copiar/mover com progresso, Lixo, Obter Informações, pastas inteligentes, compactar, ejetar (⏏ na barra lateral, ou arrastar o disco para o Lixo), rótulos de cor, Abrir com, Conectar ao Servidor (smb, sftp, ftp, webdav) e Rede, Preferências do Arquivos e Opções de Visualização |
| Preferências | Grade "Mostrar Todos" com os painéis da era (Aparência, Mesa e Protetor, Dock, Monitores, Som, Rede, Energia, Teclado, Mouse, Trackpad, Data e Hora, Contas, Internacional, Itens de Início, Acesso Universal, Terminal…) |
| Terminal | Núcleo do Alacritty: truecolor, nvim/htop, mouse, OSC 52, temas, Nerd Font; arrastar um arquivo escreve o caminho |
| Editor de Texto | Documentos em RTF (abre no Word e no LibreOffice): fontes, parágrafos e régua, estilos, listas, tabelas, imagens, páginas com cabeçalho, rodapé e notas, modelo ABNT, exportar PDF; também texto simples; Localizar/Substituir |
| Visualizador | Imagens (PNG, JPEG, WebP) e PDF, miniaturas, zoom, girar; desenhar sobre as capturas de tela |
| Navegador | Abre o navegador que cabe na máquina (NetSurf com pouca RAM, Firefox acima) |
| Correio | IMAP e SMTP com TLS, caixas, escrever, responder e encaminhar |
| Agenda e Contatos | Mês em grade com eventos (.ics) e fichas (vCard) |
| Música | Biblioteca (a pasta Música e as que você juntar), Artistas e Álbuns, listas (M3U), aleatório e repetir; tocador leve em Rust puro |
| Vídeo | O mpv mostra a imagem, o Vídeo controla: menus, legendas, faixas de áudio, velocidade, continuar de onde parou |
| Lembretes | Notas adesivas coloridas, todas num processo só |
| Dicionário | Dicionários livres StarDict e DICT |
| Livro de Fontes e Caracteres | Ver, instalar e desativar fontes; mapa de caracteres |
| Acesso às Chaves | Senhas guardadas no chaveiro do sistema: ver, copiar, apagar, trancar e mudar a senha do chaveiro |
| Utilitário de Rede | Informações das interfaces, Ping, Lookup, Traceroute, Whois, Finger e varredura de portas |
| Monitor, Registros, Informações do Sistema | Processos e gráficos, logs do journal, hardware |
| Discos | Pelo udisks2 (como o gnome-disks): discos, imagens e o mapa dos volumes (partições e espaço livre), montar, desmontar e ejetar; formatar, partições, redimensionar e gravar ISO vêm na Fase 16 |
| Busca, Captura, Utilitário de Arquivo | Achar arquivos, capturar a tela, compactar/expandir |
| Calculadora, Relógio, Xadrez, Ajuda, Sobre o Glove | Calculadora científica, relógio no Dock, xadrez com 3 níveis, ajuda com busca |
| Cópias de Segurança | Cópias de hora em hora da pasta pessoal num disco externo, linha do tempo e Restaurar |
| Central de Apps | Procurar, instalar, remover e atualizar pelo apt e pelo Flatpak (Flathub) |
| Fotos | A pasta Imagens em miniaturas por mês, álbuns pelas subpastas e apresentação |
| Gravador | Grava a voz em WAV com nível e onda; ouvir, apagar |
| Mensagens | Bate-papo no IRC (Libera.Chat por padrão, com TLS) em balões |
| Instalar o Glove OS | Do pendrive (a ISO) para o disco: o disco inteiro ou ao lado de outro sistema |

O Arquivos, o Editor e o Terminal têm **abas**; o Arquivos tem a **Olhada Rápida** (Espaço).

## Leveza

Os 38 apps são um executável só (`glove-apps`; `glove-files`, `glove-prefs`… são atalhos para ele, e
ele abre o app pelo nome com que foi chamado, como o BusyBox). Por quê:

- **Disco:** todos os apps usam a mesma biblioteca de interface e de desenho (botões, ícones, fontes,
  temas). Com um executável por app, esse código se repetia em cada um: eram 166 MB. Juntos, ele
  existe uma vez só: 44 MB (cerca de 55 MB com o compositor e o portal). Menos para baixar na ISO e
  nas atualizações.
- **Memória:** o Linux carrega as páginas de código de um executável uma vez e as divide entre
  todos os processos que o usam. Cada app aberto deixa de trazer a sua cópia do código; só os dados
  dele (janela, arquivos, texto) são próprios. Com Arquivos, Preferências e Boas-vindas abertos, a
  memória caiu de 35 para 26 MB (−26%), e a economia cresce a cada app aberto.
- **Compilação:** a biblioteca é ligada uma vez, não 38: de ~7 para ~2,5 minutos.
- **O que não muda:** cada app continua sendo um processo separado. Se um trava, os outros seguem, e
  o Forçar Encerrar fecha só aquele.
- **O custo:** qualquer mudança gera de novo o executável inteiro (31 MB), e não só o do app mudado.

As fontes também são mapeadas do arquivo em vez de copiadas (as mesmas páginas para todos os
processos). Parado, o Glove usa 0% do processador: só redesenha o que muda.

Memória própria de cada processo (RssAnon), medida em 15/11/2026 numa sessão de verdade (tela 1920×1200
em escala 1.5):

| Processo | Memória |
|---|---|
| `glove` (compositor) | ~32 MB, com o heap do driver de vídeo (Mesa), as texturas e o papel de parede |
| `glove-prefs` | ~4 MB |
| `glove-calc` | ~2,6 MB |
| `glove-term` | ~1,5 MB parado; 200 mil linhas em 0,16 s |

Detalhes e como medir: [docs/MEMORIA.md](docs/MEMORIA.md). Regras para manter assim: fontes lidas sob
demanda, peças da interface vão para a GPU sem cópia na RAM, buffers reaproveitados entre quadros e o
alocador devolve memória ao sistema.

## Como rodar

A biblioteca dos apps ([glove-ui](https://github.com/fabioaacarneiro/glove-ui)) vem como submódulo
em `ui/`: clone com `git clone --recursive` (ou rode `git submodule update --init` depois). **Depois de
cada `git pull`**, atualize também a glove-ui com `git submodule update --init` (o
`./glove-setup-tool.sh` já faz isso sozinho); com a glove-ui velha, a compilação falha.

Precisa do **Rust 1.89 ou mais novo** (o mínimo das dependências). O Rust de algumas distros é mais
velho (o Debian 13 traz o 1.85): instale pelo [rustup](https://rustup.rs) ou rode `rustup update`. As
versões novas do Rust compilam o Glove; a CI de cada versão usa sempre a estável mais recente.

```sh
cargo build --release
./target/release/glove            # abre o Glove numa janela
./target/release/glove -c foot    # já abrindo um programa
```

Todos os apps do Dock são do próprio Glove. Apps GTK/Qt abertos dentro dele podem desenhar
a própria barra de título; apps X11 usam a do Glove.

## Instalar no sistema

O `glove-setup-tool.sh` gera os pacotes e também instala direto, em qualquer Linux. O pacote traz
tudo: os programas, a sessão "Glove" na tela de login, a tela de login do Glove (serviço
`glove-login`, sobre o greetd, que vem como dependência), o tema de ícones e a documentação.

```sh
./glove-setup-tool.sh                 # menu com todas as opções
./glove-setup-tool.sh deb             # pacotes/glove_VERSÃO_amd64.deb (Debian, Ubuntu, Zorin, Mint)
./glove-setup-tool.sh rpm             # Fedora/openSUSE (precisa do rpmbuild; sem ele, deixa o .spec)
./glove-setup-tool.sh arch            # Arch (.pkg.tar.zst e o PKGBUILD para o makepkg)
./glove-setup-tool.sh slackware       # Slackware (.tgz)
./glove-setup-tool.sh todos           # os quatro (em inglês: all)
./glove-setup-tool.sh instalar        # sem pacote: compila e instala em /usr/local
./glove-setup-tool.sh desinstalar     # o inverso do "instalar" (ou chama o gerenciador de pacotes)
./glove-setup-tool.sh dependencias    # o que falta para compilar (apt, dnf, pacman, zypper, xbps, apk)
./glove-setup-tool.sh grub            # papel de parede do Glove OS na tela do GRUB ("grub remover" desfaz)
```

Instalar o pacote (exemplo no Debian/Ubuntu/Zorin):

```sh
sudo apt install ./pacotes/glove_VERSÃO_amd64.deb
```

Depois, na tela de login de sempre (GDM, LightDM, SDDM…), escolha **Glove** na engrenagem.

**Tela de login do Glove como a principal** (vale a partir do próximo boot):

```sh
sudo systemctl enable --force glove-login   # ativa (substitui o GDM/LightDM/SDDM)
sudo systemctl enable --force gdm           # volta ao GDM (ou lightdm, sddm…)
```

`./glove-setup-tool.sh login` faz o mesmo que o primeiro comando, mas guarda qual era a tela de
login de antes e leva para a tela do Glove os seus ajustes de monitor, escala, teclado e
tema; `./glove-setup-tool.sh login remover` volta à de antes. O serviço usa uma configuração
própria do greetd (não mexe em `/etc/greetd/config.toml`); para usar uma sua, crie
`/etc/glove/greetd.toml`.

**Remover:** `sudo apt remove glove` (ou `rpm -e`, `pacman -R`, `removepkg`). Se a tela de
login do Glove era a principal, a remoção volta sozinha à anterior (GDM, LightDM, SDDM…),
para o computador não ficar sem tela de login. A configuração pessoal (`~/.config/glove`)
fica.

## Glove OS (a distro, em ISO)

**Glove OS** é a distro: Debian 13 por dentro (os mesmos pacotes, repositórios e atualizações, 100%
compatível) com o Glove como área de trabalho e identidade própria: o nome "Glove OS" no
`os-release` (com `ID=debian`, para os programas), a logo da luva, o fastfetch e
o neofetch com a luva, o console e o menu do GRUB. Essa identidade vem no pacote `glove-os`, que só
vai na ISO; o pacote `glove` instalado em outra distro não muda o nome dela.

A ISO serve para pendrive ou DVD, com o Glove, o Firefox e o instalador. Ao ligar pelo pendrive, ele
entra sozinho no Glove (em português, teclado ABNT2) e abre o **Instalar o Glove OS**, que copia o
sistema para o disco (inteiro ou ao lado de outro sistema). Como usar o instalador:
[docs/USO.md](docs/USO.md#instalar-o-glove-pela-iso).

```sh
./glove-setup-tool.sh iso          # monta a ISO em iso/ (pede a senha do sudo; 15 a 40 min)
./glove-setup-tool.sh iso testar   # liga a ISO numa máquina virtual (QEMU) com um disco de teste
```

Para montar a ISO: um sistema da família Debian (Debian, Ubuntu…) em x86_64, com `dpkg-deb`, `sudo`,
o `debootstrap` instalado (`sudo apt install debootstrap`; o live-build exige o do sistema) e o Rust
1.89 ou mais novo. O script baixa o live-build e o debootstrap oficiais do Debian para
`iso/ferramentas` (o live-build de outras distribuições costuma ser velho demais). Tudo o que é
pesado fica em `iso/`, fora do Git.

No sistema ao vivo, o usuário é `glove` e a senha é **`live`** (para o `sudo` e para entrar pelo
console); o `root` não tem senha.

**ISO pronta:** a cada versão nova (tag `v…`), o GitHub Actions monta a ISO num Debian 13 e a publica,
com os pacotes `.deb` e o `SHA256SUMS`, na página de [Releases](https://github.com/fabioaacarneiro/glove/releases)
([.github/workflows/iso.yml](.github/workflows/iso.yml); a montagem leva uns 15 minutos).

## Sessão própria (escolher "Glove" na tela de login)

```sh
cargo build --release
./install.sh                          # pergunta passo a passo: sessão, tela de login, Plymouth
sudo ./data/install-session.sh        # põe "Glove" na lista de sessões (usa os arquivos do projeto)
./target/release/glove --monitores    # mostra qual tela, resolução e escala a sessão vai usar
```

Na tela de login, escolha **Glove** no ícone de engrenagem. Para testar antes sem sair da sessão
atual: Ctrl+Alt+F3, entre com usuário e senha no console e rode `./target/release/glove --sessao`.

- Sair: menu principal → **Encerrar Sessão…** ou Ctrl+Alt+Backspace
- Ctrl+Alt+F1…F12 trocam de console (e voltam ao Glove)
- Monitores em Preferências → Monitores (estender, espelhar ou só um); conectar ou desconectar
  o cabo ajusta na hora
- Teclado, mouse e touchpad pelo libinput, com as preferências aplicadas na hora
- O cursor é desenhado pelo Glove; os apps podem trocar o desenho dele (ex.: barra de texto)
- Log da sessão: `~/.local/state/glove/sessao.log`
- Tirar da tela de login: `sudo ./data/install-session.sh --remove`

Imprimir (Ctrl+P) no Editor, no Visualizador e no Correio: impressoras do CUPS, Salvar PDF e Pré-visualizar.

## Atalhos (padrão, editáveis nas Preferências)

Dentro de outro desktop, Alt vale como Super (o desktop por baixo costuma capturar o Super).

| Atalho | Ação |
|---|---|
| Super+Enter | Novo terminal |
| Super+Q | Fechar janela |
| Super+M | Minimizar |
| Super+H | Ocultar o app (some sem ir para o Dock; volta pelo Dock ou por Mostrar Todos) |
| Super+Alt+H | Ocultar os outros apps |
| Super+↑ | Zoom |
| Ctrl+clique no Dock | Abre mais uma instância do app (o clique simples traz o que já está aberto) |
| Super+Tab | Trocar de app: faixa de ícones no meio da tela (Tab/← → andam, Q encerra, H oculta, Esc cancela; soltar o Super troca) |
| Alt+Tab | Trocar entre as janelas do app da frente |
| Ctrl+Super+F | Tela cheia |
| F12 | Painel de Widgets (de novo, Esc ou clique fora fecha) |
| F4 | Todos os Apps (de novo, Esc ou clique fora fecha) |
| Ctrl+Super+← / → | Mesa virtual anterior / próxima (com Shift, leva a janela) |
| F9 / F10 / F11 | Exposé: todas as janelas / só as do app / mostrar a mesa (de novo, ou Esc, volta) |
| Super+, | Preferências |
| Print / Shift+Print | Captura da tela inteira / de uma área (ou de uma janela, pelo app Captura) |
| Ctrl+Alt+Esc | Forçar Encerrar (lista os apps; marca os que não respondem) |
| Ctrl+Alt+Backspace | Sair do Glove (encerra a sessão) |
| Ctrl+Alt+F1…F12 | Trocar de console (só na sessão própria) |
| Teclas de volume e brilho | Mudam o volume (PipeWire) e o brilho (pelo logind, sem root), com aviso na tela |
| Super+Espaço | Busca Rápida (também na lupa do canto da barra): apps instalados, Preferências e arquivos; Enter abre o primeiro |
| Super+L | Bloquear a tela (também no menu principal; desbloqueia com a senha do usuário) |
| Super+Alt+Espaço | Próximo layout de teclado |
| Super+Alt+8 / = / − | Zoom da tela: ligar, aproximar, afastar (Ctrl+Super+Alt+8 inverte as cores) |
| Ctrl+F2 | Abrir a barra de menu pelo teclado (setas navegam, → abre submenus, Enter escolhe, Esc fecha) |

## Tela de login (opcional, greetd)

O Glove tem tela de login própria (`glove --greeter` + `glove-login`), feita para o
[greetd](https://sr.ht/~kennylevinsen/greetd/). Com o Glove instalado (pacote ou
`./glove-setup-tool.sh instalar`), ela é o serviço `glove-login`: veja "Instalar no sistema" acima
(`sudo systemctl enable --force glove-login`).

No modo de desenvolvimento (a sessão usa os programas do projeto):

```sh
cargo build --release
sudo ./data/install-login.sh           # guarda a configuração atual do greetd e instala
sudo ./data/install-login.sh --remove  # devolve a configuração anterior
```

Para ver a tela numa janela, sem instalar: `glove --greeter -c "glove-login --fake"` (senha de
teste: `glove`).

## Tela do GRUB (opcional)

O `./glove-setup-tool.sh instalar` pergunta, no fim, se você quer trocar também a tela do GRUB (o
menu que aparece ao ligar o computador) pelo papel de parede do Glove OS, com o texto do menu em
branco. Dá para fazer depois, ou desfazer:

```sh
./glove-setup-tool.sh grub            # pergunta e troca (refaz o menu do GRUB)
./glove-setup-tool.sh grub remover    # volta à tela de antes
```

Ele copia a imagem para `/boot/grub/glove-os.png`, aponta o `GRUB_BACKGROUND` para ela (em
`/etc/default/grub.d/glove.cfg`; nas distros sem essa pasta, em linhas marcadas no fim de
`/etc/default/grub`) e põe as cores do texto em `/etc/grub.d/06_glove`. Só vale para o GRUB; em
sistemas com outro carregador (systemd-boot…), o comando avisa e não muda nada. Na ISO do Glove OS
isso já vem feito: no menu de boot do pendrive e no GRUB do sistema instalado.

## Tela de início do sistema (opcional, Plymouth)

Tema com fundo cinza claro, a luva do Glove e o giratório, gerado por código (`glove-arte`):

```sh
./target/release/glove-arte plymouth data/plymouth/glove   # (re)gera a arte
sudo ./data/install-plymouth.sh           # instala e ativa (refaz o initramfs)
sudo ./data/install-plymouth.sh --remove  # volta ao tema anterior
```

Na ISO do Glove OS isso já vem feito, no pendrive e no sistema instalado: o Plymouth está na lista
de pacotes, o tema vem no pacote `glove-os` (que o ativa com `/usr/share/glove-os/tela-de-inicio`) e
o GRUB do sistema instalado passa `splash` ao kernel.

## Idiomas

O Glove segue o idioma do sistema (`LANG`, que o painel Internacional grava); `GLOVE_LANG`
força um. O texto em português fica no código e é a chave (`tr!("Abrir")`); as traduções ficam
em `locales/<idioma>.po` (formato do gettext). Idioma sem tradução cai no inglês. Datas,
separador decimal e nomes dos apps seguem o idioma; o `glove-setup-tool.sh` também fala os dois.

Para acrescentar um idioma: copie `locales/en.po` para `locales/<código>.po` (ex.: `es.po`),
traduza as linhas `msgstr` e ponha em `~/.local/share/glove/locales/` para testar sem
recompilar; para vir junto com o Glove, deixe o arquivo em `locales/`: todo `.po` dessa pasta entra no
programa na compilação (o `build.rs` da `glove-i18n`, em `ui/crates/i18n`).
`cargo test -p glove-i18n` falha se algum texto marcado ficar sem tradução em inglês, e
`tools/i18n.py` ajuda a marcar textos novos.

## Configuração

`~/.config/glove/config.toml` (criado na primeira execução). As Preferências editam esse arquivo e o
Glove recarrega sozinho quando ele muda.

Tudo que existe no arquivo também se ajusta nas Preferências. Exemplos:

```toml
[energy]
screen_off_minutes = 10   # parado por N minutos apaga a tela (0 = nunca)
require_password = true   # pede a senha ao voltar (tela apagada, repouso, hibernação, tampa fechada)

[hot_corners]             # Exposé nos cantos: "", "todas", "app" ou "mesa"
bottom_right = "mesa"
```

O desbloqueio confere a senha pelo PAM. `data/install-session.sh` instala o serviço
`/etc/pam.d/glove`; sem ele, o Glove usa o serviço `login`.

## Estrutura

```
ui/ (glove-ui)     submódulo: glove-ui (o antigo glove-kit), glove-paint, glove-config e glove-i18n
crates/compositor  o compositor (`glove`; udev.rs = sessão própria, winit.rs = janela, `--greeter` = login)
crates/apps        `glove-apps`: um executável só para todos os apps (glove-files, glove-prefs… são atalhos para ele; ver docs/MEMORIA.md)
crates/portal      `glove-portal` (portal FileChooser) e `glove-escolher`
crates/login       tela de login (`glove-login`, pelo greetd; `--fake` para testar)
crates/boasvindas  assistente de primeiro uso (`glove-boasvindas`)
crates/sair        confirmação de Desligar/Reiniciar/Encerrar Sessão (`glove-sair`)
crates/arte        `glove-arte`: arte usada fora do Glove (tema do Plymouth, o tema de ícones "Glove" com `glove-arte icones <pasta>` e a amostra dele com `glove-arte previa <arquivo.png>`)
crates/energia     bateria (biblioteca usada pelo compositor e pelas Preferências)
crates/instalador  Instalar o Glove OS (`glove-instalador`)
crates/senha       senha do sistema: o agente do polkit (`glove-senha`)
crates/files       Arquivos (`glove-files`)          crates/prefs      Preferências (`glove-prefs`)
crates/term        Terminal (`glove-term`)           crates/editor     Editor de Texto (`glove-edit`)
crates/viewer      Visualizador (`glove-view`)       crates/navegador  Navegador (`glove-navegador`)
crates/correio     Correio (`glove-correio`)         crates/agenda     Agenda (`glove-agenda`)
crates/contatos    Contatos (`glove-contatos`)       crates/musica     Música (`glove-musica`)
crates/video       Vídeo (`glove-video`)             crates/notes      Lembretes (`glove-notes`)
crates/dicionario  Dicionário (`glove-dicionario`)   crates/fontes     Livro de Fontes (`glove-fontes`)
crates/chars       Caracteres (`glove-chars`)        crates/monitor    Monitor (`glove-monitor`)
crates/registros   Registros (`glove-registros`)     crates/info       Informações do Sistema (`glove-info`)
crates/disks       Discos (`glove-disks`)            crates/find       Busca (`glove-find`)
crates/shot        Captura (`glove-shot`)            crates/arquivo    Utilitário de Arquivo (`glove-arquivo`)
crates/calc        Calculadora (`glove-calc`)        crates/clock      Relógio (`glove-clock`)
crates/chess       Xadrez (`glove-chess`)            crates/ajuda      Ajuda (`glove-ajuda`)
crates/about       Sobre o Glove (`glove-about`)       crates/chaves     Acesso às Chaves (`glove-chaves`)
crates/utilrede    Utilitário de Rede (`glove-utilrede`) crates/copias      Cópias de Segurança (`glove-copias`)
crates/central     Central de Apps (`glove-central`)  crates/fotos       Fotos (`glove-fotos`)
crates/gravador    Gravador (`glove-gravador`)        crates/mensagens   Mensagens (`glove-mensagens`)
data/              sessão, greetd, Plymouth, GRUB, papéis de parede, a identidade do Glove OS e a ISO
ci/iso.sh          monta a ISO na integração contínua (.github/workflows/iso.yml)
```

## Próximas etapas

O plano completo, em fases e etapas (cada uma com seu commit), está em [docs/PLANO.md](docs/PLANO.md),
com as análises detalhadas de cada área em [docs/analise/](docs/analise/).

## Neovim com Nerd Font

O Terminal mostra ícones de Nerd Font sozinho se houver uma instalada (ele procura uma fonte
reserva só quando aparece um caractere que a fonte principal não tem). Para instalar:

```sh
mkdir -p ~/.local/share/fonts && cd ~/.local/share/fonts
curl -LO https://github.com/ryanoasis/nerd-fonts/releases/latest/download/JetBrainsMono.zip
unzip JetBrainsMono.zip && fc-cache -f
```

Depois, em Preferências → Terminal, escreva `JetBrainsMono Nerd Font` (ou deixe vazio para
usar a fonte padrão com os ícones vindos da Nerd Font).

## Licença

GPL-3.0-ou-posterior (veja [LICENSE](LICENSE)). O tema de ícones também é publicado à parte:
[glove-icons](https://github.com/fabioaacarneiro/glove-icons).
