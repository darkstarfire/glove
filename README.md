# Glove

Ambiente gráfico leve para Linux, escrito em Rust, com visual inspirado na era
"gel" dos anos 2000: botões translúcidos, listras finas, metal escovado, sombras
suaves e um Dock com ampliação. Toda a arte é desenhada por código (sem imagens).

## Estado atual

- Compositor Wayland próprio (Smithay), em **sessão própria** (escolhida na tela de login, desenha
  direto na placa de vídeo) ou **dentro de uma janela** do seu desktop
- Nítido em telas HiDPI, inclusive escala fracionada (1.5×), para o Glove e para os apps
- Barra de menu global: menu principal, menu do app (Ocultar/Encerrar) e os **menus de cada app**
  (Arquivo, Editar…), enviados pelo app ao compositor por um socket local leve
- Dock em prateleira de vidro com ampliação, nome do app, indicador de app aberto, janelas
  minimizadas e lixeira
- Barras de título com fechar / minimizar / zoom, sombra, arrastar e redimensionar pela borda
- 4 temas trocados na hora: **Claro** (padrão, sóbrio como o fim da era), **Listras**, **Metal** e **Grafite**
- **glove-kit**: toolkit dos apps (botões em gel, caixas, rádio, slider, pop-up, campo de texto, abas, listas)
- **Preferências**: tema, papel de parede (Ondas, Aurora, Liso ou PNG de ~/Imagens), **Monitor**
  (qual tela, resolução com confirmação de 15 s, tamanho da interface), Dock (tamanho e ampliação),
  teclado (layout e repetição), **Mouse** (velocidade, aceleração, rolagem natural, canhoto),
  **Trackpad** (tocar para clicar, clique secundário, velocidade, rolagem natural, ignorar ao digitar)
  e atalhos editáveis. Tudo aplicado na hora.
- **Sobre o Glove**: versão, memória instalada e processador; "Mais informações…" abre o Monitor
- **Calculadora** (teclado ou mouse, formato brasileiro com vírgula) e **Relógio** (analógico + data
  por extenso; o ícone do Dock mostra a hora atual)
- **Editor de Texto**: janela só com o documento (comandos nos menus da barra), puxador de
  redimensionar, tamanho da fonte, abrir/salvar (gravação segura), seleção com mouse e teclado, clique duplo/triplo,
  desfazer/refazer por palavra, copiar/colar entre apps, aviso de alterações não salvas ao fechar
- **Arquivos**: visualização em colunas (com painel de detalhes), lista e ícones; voltar/avançar,
  lugares, nova pasta, mover para a Lixeira (padrão freedesktop), espaço livre, abrir no Editor
- **Visualizador** de imagens (PNG e JPEG): ajustar à janela, tamanho real, zoom, girar, arrastar,
  próxima/anterior da pasta; abre pelo clique duplo no Arquivos
- **Terminal**: núcleo de terminal do Alacritty (xterm-256color, truecolor, tela cheia como `top`/`nvim`),
  histórico de 5000 linhas, seleção, copiar/colar (Ctrl+Shift+C/V), mouse repassado aos programas,
  cursor em bloco/barra/sublinhado, sublinhado ondulado e colorido (LSP), teclas com modificadores
  no padrão xterm, OSC 52, fonte configurável e ícones de Nerd Font por fonte reserva
- **Lembretes**: notas adesivas coloridas, salvas sozinhas, reabertas ao abrir o app
- **Monitor**: gráfico de CPU, uso de memória, processos (encerrar/forçar)
- **Captura**: tela inteira na hora ou com atraso; salva em ~/Imagens e abre no Visualizador
- **Caracteres**: mapa por categoria (acentos, grego, setas, matemática, símbolos, ícones), copiar
- **Busca**: arquivos por nome em segundo plano, abrir ou mostrar no Arquivos
- **Discos**: discos montados, espaço livre, abrir, ejetar pendrives
- **Xadrez**: regras completas (testadas com contagem de lances de referência), 3 níveis, desfazer,
  jogar de pretas; o computador pensa em segundo plano
- Ícone do disco na mesa (clique duplo abre o Arquivos)
- Redesenha só o que mudou: 0% de CPU parado

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

Todos os apps do Dock são do próprio Glove. Apps GNOME abertos dentro dele desenham
a própria barra de título.

## Sessão própria (escolher "Glove" na tela de login)

```sh
cargo build --release
sudo ./data/instalar-sessao.sh        # põe "Glove" na lista de sessões (usa os arquivos do projeto)
./target/release/glove --monitores    # mostra qual tela, resolução e escala a sessão vai usar
```

Na tela de login, escolha **Glove** no ícone de engrenagem. Para testar antes sem sair da sessão
atual: Ctrl+Alt+F3, entre com usuário e senha no console e rode `./target/release/glove --sessao`.

- Sair: menu principal → **Encerrar Sessão…** ou Ctrl+Alt+Backspace
- Ctrl+Alt+F1…F12 trocam de console (e voltam ao Glove)
- Uma tela por vez: a escolhida em Preferências → Monitor, ou a externa se houver, ou a do notebook.
  Conectar ou desconectar o cabo troca na hora
- Teclado, mouse e touchpad pelo libinput, com as preferências aplicadas na hora
- O cursor é desenhado pelo Glove; os apps podem trocar o desenho dele (ex.: barra de texto)
- Log da sessão: `~/.local/state/glove/sessao.log`
- Tirar da tela de login: `sudo ./data/instalar-sessao.sh --remover`

Limitações atuais da sessão própria:

- Uma tela por vez (sem estender ou espelhar em dois monitores)
- A tela não apaga sozinha quando o computador fica parado
- Fechar a tampa do notebook não faz nada (a suspensão fica por conta do sistema)
- Teclas de brilho e volume não funcionam
- Apps só-X11 não abrem (falta o XWayland); apps Wayland abrem normalmente
- Ainda não há como abrir os apps instalados no sistema fora do Dock (a pasta Aplicativos
  está no plano, etapa 6.7)

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
| Super+Tab | Trocar de janela |
| F9 / F10 / F11 | Exposé: todas as janelas / só as do app / mostrar a mesa (de novo, ou Esc, volta) |
| Super+, | Preferências |
| Print | Captura de tela em ~/Imagens |
| Ctrl+Alt+Esc | Forçar Encerrar (lista os apps; marca os que não respondem) |
| Ctrl+Alt+Backspace | Sair do Glove (encerra a sessão) |
| Ctrl+Alt+F1…F12 | Trocar de console (só na sessão própria) |
| Teclas de volume e brilho | Mudam o volume (PipeWire) e o brilho (pelo logind, sem root), com aviso na tela |
| Super+Espaço | Busca Rápida (também na lupa do canto da barra): apps instalados, Preferências e arquivos; Enter abre o primeiro |
| Super+L | Bloquear a tela (também no menu principal; desbloqueia com a senha do usuário) |
| Ctrl+F2 | Abrir a barra de menu pelo teclado (setas navegam, → abre submenus, Enter escolhe, Esc fecha) |

## Configuração

`~/.config/glove/config.toml` (criado na primeira execução). As Preferências editam esse arquivo e o
Glove recarrega sozinho quando ele muda.

Tela apagada e bloqueio (até o painel Economizador de Energia, etapa 7.4, só pelo arquivo):

```toml
[energy]
screen_off_minutes = 10   # parado por N minutos apaga a tela (0 = nunca)
require_password = true   # pede a senha ao voltar (tela apagada, Repouso, tampa fechada)
```

Cantos ativos do Exposé (vazio = nada; "todas", "app" ou "mesa"):

```toml
[hot_corners]
top_left = ""
top_right = ""
bottom_left = ""
bottom_right = "mesa"
```

O desbloqueio confere a senha pelo PAM. `data/instalar-sessao.sh` instala o serviço
`/etc/pam.d/glove`; sem ele, o Glove usa o serviço `login`.

## Estrutura

```
crates/paint       desenho: Canvas com escala, gel, listras, metal, sombras, ícones, texto, temas
crates/config      configuração compartilhada (config.toml + aviso de mudança via inotify)
crates/kit         toolkit dos apps (widgets + janela Wayland)
crates/compositor  o compositor (binário `glove`; udev.rs = sessão própria, winit.rs = janela)
crates/about       app "Sobre o Glove"
crates/prefs       app Preferências (binário `glove-prefs`)
crates/calc        Calculadora (`glove-calc`)
crates/clock       Relógio (`glove-clock`)
crates/editor      Editor de Texto (`glove-edit`)
crates/files       Arquivos (`glove-files`)
crates/viewer      Visualizador (`glove-view`)
crates/term        Terminal (`glove-term`)
crates/notes       Lembretes (`glove-notes`)
crates/monitor     Monitor (`glove-monitor`)
crates/shot        Captura (`glove-shot`)
crates/chars       Caracteres (`glove-chars`)
crates/find        Busca (`glove-find`)
crates/disks       Discos (`glove-disks`)
crates/chess       Xadrez (`glove-chess`)
crates/sair        confirmação de Desligar/Reiniciar/Encerrar Sessão (`glove-sair`)
data/              sessão: glove-session, glove.desktop, instalar-sessao.sh
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
