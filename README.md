# Glove

Ambiente gráfico leve para Linux, escrito em Rust, com visual inspirado na era
"gel" dos anos 2000: botões translúcidos, listras finas, metal escovado, sombras
suaves e um Dock com ampliação. Toda a arte é desenhada por código (sem imagens).

## Estado atual

Base visual e de comportamento: **Mac OS X 10.4 Tiger** (com o tema Listras para o visual 10.0–10.2).
Fases 1 a 9 do [plano](docs/PLANO.md) feitas; a 9.10 (o que ainda falta do Tiger) é a próxima.

**Sistema (compositor `glove`)**

- Compositor Wayland próprio (Smithay), em **sessão própria** (DRM/KMS + libinput) ou **dentro de uma janela**
- **Vários monitores**: estender, espelhar ou só um; resolução e escala por tela; nítido em HiDPI e escala fracionada
- **Apps X11** pelo Xwayland, com barra de título do Glove
- Barra de menu global: menu principal (Itens Recentes, Forçar Encerrar, Repouso, Desligar…),
  menu do app, os **menus de cada app**, submenus e navegação pelo teclado; extras de Wi-Fi,
  volume, bateria e monitor, relógio e nome do usuário (troca rápida de usuário)
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
- **Busca Rápida** (Super+Espaço): apps instalados, Preferências e arquivos
- **Captura** de tela inteira, área ou uma janela (clique), em qualquer monitor
- Forçar Encerrar com detecção de app que não responde; bloqueio de tela (PAM), tela que apaga,
  tampa do notebook, teclas de volume e brilho; troca rápida de usuário
- Acesso Universal: zoom, cores invertidas, cursor grande
- Arrastar e soltar entre apps, área de transferência de texto e de arquivos
- Janela Abrir/Salvar do Glove também para outros apps (portal FileChooser: Firefox, Chromium…)
- Temas **Claro** (padrão, Tiger), **Listras**, **Metal** e **Grafite**; tema de ícones do sistema escolhível
- Redesenha só o que mudou: 0% de CPU parado

**Sessão e login**

- Tela de login própria pelo greetd (`glove-login`), com escolha da sessão; splash do Plymouth
  desenhado por código; assistente de primeiro uso; Desligar/Reiniciar com contagem

**Apps (todos com o glove-kit e arte própria)**

| App | O que faz |
|---|---|
| Arquivos | Finder do Tiger: metal, barra lateral, colunas/lista/ícones, prévia com zoom, copiar/mover com progresso, Lixo, Obter Informações, pastas inteligentes, compactar, ejetar (⏏ na barra lateral, ou arrastar o disco para o Lixo), rótulos de cor, Abrir com, Conectar ao Servidor (smb, sftp, ftp, webdav) e Rede, Preferências do Arquivos e Opções de Visualização |
| Preferências | Grade "Mostrar Todos" com os painéis da era (Aparência, Mesa e Protetor, Dock, Monitores, Som, Rede, Energia, Teclado, Mouse, Trackpad, Data e Hora, Contas, Internacional, Itens de Início, Acesso Universal, Terminal…) |
| Terminal | Núcleo do Alacritty: truecolor, nvim/htop, mouse, OSC 52, temas, Nerd Font; arrastar um arquivo escreve o caminho |
| Editor de Texto | Régua, alinhamento, Localizar/Substituir, desfazer, aviso de não salvo |
| Visualizador | Imagens (PNG, JPEG, WebP) e PDF, miniaturas, zoom, girar |
| Navegador | Abre o navegador que cabe na máquina (NetSurf com pouca RAM, Firefox acima) |
| Correio | IMAP e SMTP com TLS, caixas, escrever, responder e encaminhar |
| Agenda e Contatos | Mês em grade com eventos (.ics) e fichas (vCard) |
| Música e Vídeo | Biblioteca e tocador leve (Rust puro); vídeo pelo mpv com controles próprios |
| Lembretes | Notas adesivas coloridas, todas num processo só |
| Dicionário | Dicionários livres StarDict e DICT |
| Livro de Fontes e Caracteres | Ver, instalar e desativar fontes; mapa de caracteres |
| Acesso às Chaves | Senhas guardadas no chaveiro do sistema: ver, copiar, apagar, trancar e mudar a senha do chaveiro |
| Utilitário de Rede | Informações das interfaces, Ping, Lookup, Traceroute, Whois, Finger e varredura de portas |
| Monitor, Registros, Informações do Sistema, Discos | Processos e gráficos, logs do journal, hardware, discos e pendrives |
| Busca, Captura, Utilitário de Arquivo | Achar arquivos, capturar a tela, compactar/expandir |
| Calculadora, Relógio, Xadrez, Ajuda, Sobre o Glove | Calculadora científica, relógio no Dock, xadrez com 3 níveis, ajuda com busca |

## Leveza

Memória própria de cada processo (RssAnon), com tela 1920×1200 em escala 1.5:

| Processo | Memória |
|---|---|
| `glove` (compositor) | ~17 MB (+ bibliotecas do driver de vídeo, compartilhadas) |
| `glove-prefs` | ~5 MB |
| `glove-about` | ~3 MB |
| `glove-calc` | ~3 MB, 0% de CPU parado |
| `glove-edit` | ~8 MB com um arquivo de 400 KB (5 mil linhas), 0% de CPU parado |
| `glove-files` | ~7 MB, 0% de CPU parado |
| `glove-view` | ~20 MB com uma imagem de 1920×1200 (a imagem em si ocupa 9 MB); 0% de CPU parado |
| `glove-term` | ~8 MB parado (~16 MB com histórico cheio); 0% de CPU parado; 200 mil linhas em 0,16 s |
| `glove-notes` | ~4 MB por lembrete |
| `glove-monitor` | ~7,5 MB, ~1% de CPU (lê os processos a cada 2 s) |
| `glove-chars` | ~7 MB |
| `glove-find` | ~7 MB |
| `glove-disks` | ~4 MB |
| `glove-chess` | ~5 MB |
| `glove-shot` | ~4 MB |
| `glove-clock` | ~4 MB, ~0,25% de CPU (só os ponteiros são redesenhados a cada segundo) |

Regras para manter assim: fontes lidas sob demanda, peças da interface vão para a GPU sem cópia
na RAM, buffers reaproveitados entre quadros e o alocador devolve memória ao sistema.

## Como rodar

```sh
cargo build --release
./target/release/glove            # abre o Glove numa janela
./target/release/glove -c foot    # já abrindo um programa
```

Todos os apps do Dock são do próprio Glove. Apps GTK/Qt abertos dentro dele podem desenhar
a própria barra de título; apps X11 usam a do Glove.

## Sessão própria (escolher "Glove" na tela de login)

```sh
cargo build --release
./instalar.sh                         # pergunta passo a passo: sessão, tela de login, Plymouth
sudo ./data/instalar-sessao.sh        # põe "Glove" na lista de sessões (usa os arquivos do projeto)
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
- Tirar da tela de login: `sudo ./data/instalar-sessao.sh --remover`

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
| F12 | Painel de Widgets (de novo, Esc ou clique fora fecha) |
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
[greetd](https://sr.ht/~kennylevinsen/greetd/):

```sh
cargo build --release
sudo ./data/instalar-login.sh            # guarda a configuração atual do greetd e instala
sudo ./data/instalar-login.sh --remover  # devolve a configuração anterior
```

O instalador nunca troca o gerenciador de login sozinho; se o greetd não estiver ativo, ele
mostra os comandos para trocar e para voltar. Para ver a tela numa janela, sem instalar:
`glove --greeter -c "glove-login --fake"` (senha de teste: `glove`).

## Tela de início do sistema (opcional, Plymouth)

Tema com fundo cinza claro, orbe e giratório, desenhado por código (`glove-arte`):

```sh
./target/release/glove-arte plymouth data/plymouth/glove   # (re)gera a arte
sudo ./data/instalar-plymouth.sh             # instala e ativa (refaz o initramfs)
sudo ./data/instalar-plymouth.sh --remover   # volta ao tema anterior
```

## Configuração

`~/.config/glove/config.toml` (criado na primeira execução). As Preferências editam esse arquivo e o
Glove recarrega sozinho quando ele muda.

Tudo que existe no arquivo também se ajusta nas Preferências. Exemplos:

```toml
[energy]
screen_off_minutes = 10   # parado por N minutos apaga a tela (0 = nunca)
require_password = true   # pede a senha ao voltar (tela apagada, Repouso, tampa fechada)

[hot_corners]             # Exposé nos cantos: "", "todas", "app" ou "mesa"
bottom_right = "mesa"
```

O desbloqueio confere a senha pelo PAM. `data/instalar-sessao.sh` instala o serviço
`/etc/pam.d/glove`; sem ele, o Glove usa o serviço `login`.

## Estrutura

```
crates/paint       desenho: Canvas com escala, gel, metal, sombras, ícones no estilo do Tiger (tiger.rs, tiger_prefs.rs e tiger_real.rs, guardados prontos por tamanho) e do sistema, texto, temas
crates/config      configuração compartilhada (config.toml + aviso de mudança via inotify)
crates/kit         toolkit dos apps (widgets, janela Wayland, Abrir/Salvar, arrastar e soltar)
crates/compositor  o compositor (`glove`; udev.rs = sessão própria, winit.rs = janela, `--greeter` = login)
crates/apps        `glove-apps`: um executável só para todos os apps (glove-files, glove-prefs… são atalhos para ele; ver docs/MEMORIA.md)
crates/portal      `glove-portal` (portal FileChooser) e `glove-escolher`
crates/login       tela de login (`glove-login`, pelo greetd; `--fake` para testar)
crates/boasvindas  assistente de primeiro uso (`glove-boasvindas`)
crates/sair        confirmação de Desligar/Reiniciar/Encerrar Sessão (`glove-sair`)
crates/arte        `glove-arte`: arte usada fora do Glove (tema do Plymouth e o tema de ícones "Glove": `glove-arte icones <pasta>`)
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
crates/utilrede    Utilitário de Rede (`glove-utilrede`)
data/              sessão, greetd, Plymouth, portais e instaladores
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
