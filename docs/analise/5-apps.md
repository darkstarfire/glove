# 5 — Aplicativos do Glove × referências (10.0–10.3)

Legenda: prioridade **Alta/Média/Baixa** · esforço **P** (≤1 dia) / **M** (2–5 dias) / **G** (>1 semana).
Imagens: `referencias/<versão>/…`; Glove: `scratchpad/glove/glove-*.png`. Toda arte nova é desenhada por código e original. Só copiamos o comportamento e o layout, nunca logotipos nem ícones.

## 0. Base comum (glove-kit): falta muita peça

Nas referências, as mesmas peças aparecem em quase todo app, e o `crates/kit/src/ui.rs` ainda não tem várias delas (hoje tem: button, gel_button, checkbox, radio, slider, popup, text_field, tabs, list, text_area, icon_tile…):

| Peça | Onde aparece nas referências | Tarefa |
|---|---|---|
| **Barra de ferramentas** com ícone grande + rótulo e separador pontilhado | Preview, Mail, Address-Book-New, Activity Monitor (`jaguar/10-2-Jaguar-Preview`, `panther/system-managers-tasks_-1-1`) | `ui.toolbar(items)` com estado desabilitado (rótulo cinza) · **Alta/M** |
| **Tabela com colunas** (cabeçalho em gel, coluna ordenada destacada em azul com ▲/▼, linhas zebradas azul-claro, colunas redimensionáveis) | Process Viewer, iTunes, Mail, Finder-Search | `ui.table(cols, rows, sort)` no kit, para Monitor, Busca, Música, Correio usarem · **Alta/M** |
| **Campo de busca arredondado** (lupa com ▾, "x" para limpar) | Mail, iTunes, iCal, Font panel | `ui.search_field` · **Alta/P** |
| **Controle segmentado** (Day/Week/Month, alinhamento do TextEdit) | `panther/applications-office-calendar_-1-1`, `panther/applications-office-texteditor_` | `ui.segmented` · **Média/P** |
| **Lista de fontes** (source list) com ícone e cabeçalho "Source/Group" | iTunes, Address Book, iCal | variante de `list` com ícones · **Média/P** |
| **Divisor arrastável** (split view com "puxador" de pontos) | Mail (`10-2-Jaguar-Mail` ≡ no centro), Address Book | `ui.split(h/v)` · **Média/M** |
| **Triângulo de expandir** ("More Info", "Description") | Process Viewer, AppleScript | `ui.disclosure` · **Baixa/P** |
| **Stepper** (setinhas ▲▼ ao lado de um campo) | Process Viewer "Sample every 20 seconds" | `ui.stepper` · **Baixa/P** |
| **Janela "metal escovado"** por app (não só como tema global) | Calculator, Address Book, iTunes, QuickTime, iCal, Safari | flag `WindowStyle::Metal` em `kit/src/platform.rs` + `paint/src/chrome.rs` · **Média/M** |
| **Diálogo Abrir/Salvar** como *sheet* com navegador em colunas, "De:", "Ir para:", "Adicionar aos Favoritos", Cancelar/Abrir | `jaguar/10-2-Jaguar-TextEdit-File-Open` | `kit::file_sheet` reaproveitando o modo colunas do `crates/files` · **Alta/G**. Hoje o Editor pede para **digitar o caminho**, que é o maior defeito visível entre os apps |
| **Painel de Fontes** e **Painel de Cores** compartilhados | ver §3 | **Média/M** cada |

---

## 1. Apps existentes

### 1.1 Editor de Texto (`crates/editor`)
**(a) Referência.** `jaguar/10-2-Jaguar-TextEdit`, `cheetah/10-0-Cheetah-TextEdit`, `panther/applications-office-texteditor_`: régua com marcas de tabulação ▶ a cada polegada e régua numerada 0–6; logo acima, uma faixa de botões: 4 alinhamentos agrupados, campo "0 pt" de espaçamento, entrelinhas, botões de cadeado e uma paleta de tabulações ▶◆◀●. No Panther a faixa vira os pop-ups "Styles" e "Spacing" mais um segmentado de alinhamento. A barra de rolagem fica sempre visível, com puxador de redimensionar. O ícone do documento aparece na barra de título (proxy icon). O diálogo Abrir é um sheet em colunas, com "Plain Text Encoding" e "Ignore rich text commands".
**(b) Glove.** Só a área de texto (`glove-edit.png`, "Sem título"), plain text, tamanho de fonte, abrir/salvar, desfazer, aviso ao fechar. Abrir e salvar pedem que você digite o caminho.
**(c) Lacuna.** Faltam a régua, as opções de formato, o ícone de documento no título, o diálogo de arquivos, buscar/substituir, a quebra de linha configurável e a codificação.
**(d) Tarefas.**
- Trocar o sheet "digite o caminho" pelo `kit::file_sheet`, com pop-up de codificação (UTF-8 / Latin-1 / Automática) · **Alta/G** (é infraestrutura do kit)
- Formatar ▸ "Mostrar régua" com régua desenhada por código (polegadas ou cm), segmentado de alinhamento e pop-up de espaçamento. No modo texto simples só alinhamento e tabulação valem para o documento inteiro · **Média/M**
- Editar ▸ Localizar… (painel com Localizar/Substituir/Todos), Ctrl+F/G · **Alta/M**
- Formatar ▸ "Quebrar no tamanho da janela / da página" e "Converter em texto simples" (reservado para um RTF futuro) · **Média/P**
- Proxy icon (ícone de documento) e ponto de "não salvo" no título, via `set_title` + ícone `document` de `paint/src/icons.rs` · **Baixa/P**
- Rich text (negrito, itálico, cor, fonte) só com um subconjunto de RTF. Fica para depois do Painel de Fontes · **Baixa/G**

### 1.2 Calculadora (`crates/calc`)
**(a) Referência.** Cheetah (`10-0-Cheetah-Calculator`) é pequena e cinza, com visor LCD verde-amarelado e todos os botões brancos em gel. No Jaguar (`10-2-Jaguar-Calculator`) a janela é metal escovado, o visor é **verde-claro com "Deg"**, a linha de cima tem Basic/MC/M+/M−/MR/"Paper Tape", e há o modo científico (1/x, x², x³, yˣ, x!, √, sin/cos/tan, ln, log, eˣ, π, EE, Rad, %). No Panther (`panther/applications-office-calculator_-1-2`) aparecem 2nd, parênteses, sinh/cosh/tanh, RN e 10ˣ. O "=" é alto e ocupa duas linhas.
**(b) Glove.** Só o modo básico, com visor azul-acinzentado brilhante, operadores **azuis** e "=" **laranja** (`glove-calc.png`), vírgula decimal e teclado.
**(c) Lacuna.** Faltam a memória, o modo científico e a fita de papel. As cores dos botões fogem da referência, onde todos são neutros.
**(d) Tarefas.**
- Visor em "LCD" verde-claro com indicador Deg/Rad e operadores em gel neutro (cor só no pressionado), em `calc/src/main.rs` `ui()` · **Média/P**
- Memória MC/M+/M−/MR · **Média/P**
- Visualizar ▸ Básica/Científica: a janela se alarga para a esquerda com a grade científica (trig, potências, fatorial, ln/log, π, EE, parênteses com precedência) · **Média/M**
- "Fita de papel": janela-irmã com o histórico das contas, que se pode copiar · **Baixa/P**
- Opcional: estilo metal escovado na janela (depende do kit) · **Baixa/P**

### 1.3 Lembretes (`crates/notes`)
**(a) Referência.** `jaguar/10-2-Jaguar-Stickies` e `cheetah/10-0-Cheetah-Stickies`: as notas **não têm barra de título normal**, só uma faixa fina da mesma cor, um pouco mais escura, com uma caixinha de fechar à esquerda e uma de recolher à direita, além de um puxador no canto. As cores são amarelo, verde, azul, rosa, roxo e cinza. A sombra é suave e não há barra de rolagem. Os menus são File/Edit/Note/Color/Window. Clicar duas vezes na faixa recolhe a nota até ficar só a faixa. Há texto com estilo (negrito, itálico, cor, fonte) e busca em todas as notas.
**(b) Glove.** Janela padrão com semáforo, barra de título cinza e barra de rolagem cinza (`glove-notes.png`). Seis cores, salvamento automático e **um processo por nota** (~4 MB cada).
**(c) Lacuna.** A moldura é a maior diferença visual. O processo por nota multiplica a RAM. Falta recolher a nota e falta buscar.
**(d) Tarefas.**
- Moldura "nota": decoração mínima colorida (faixa de 12 px, botões quadrados), sem barra de rolagem quando cabe. Exige um modo `WindowStyle::Note` (client-side ou pedido ao compositor) em `kit/src/platform.rs` e `paint/src/chrome.rs` · **Alta/M**
- **Um único processo** com várias janelas (várias superfícies xdg_toplevel no kit). Economiza ~3 MB por nota adicional · **Alta/M**
- Recolher/expandir com clique duplo na faixa · **Média/P**
- Menu "Nota" com Localizar (em todas), Flutuar acima das janelas e Translúcida · **Baixa/M**
- Negrito/itálico simples (marcado em linha num formato próprio) · **Baixa/M**

### 1.4 Caracteres (`crates/chars`)
**(a) Referência.** O Key Caps (`jaguar/10-2-Jaguar-Key-Caps`, `cheetah/…Key-Caps`) é um **teclado desenhado**: teclas cinza-escuras com legenda branca e um campo de texto no alto. Segurar Shift, Option ou Ctrl troca as legendas, e há menu de fonte. O Panther (`panther/applications-accessibility-keyboardmap_`) mostra o "Keyboard Viewer" com teclas F1–F12, setas e teclado numérico, os modificadores desenhados como ⇧ ⌥ ⌘ e o pop-up "Font:".
**(b) Glove.** É uma paleta de caracteres, com categorias à esquerda, grade e pré-visualização (`glove-chars.png`). Funciona bem, mas a pré-visualização vazia ocupa muito espaço.
**(c) Lacuna.** Falta o modo teclado. A paleta corresponde à "Character Palette" do Panther, o que é válido.
**(d) Tarefas.**
- Modo **Teclado** (abas "Paleta | Teclado" ou app `glove-keys` à parte): desenhar o layout ativo lendo o keymap xkb (xkbcommon já está no compositor), legendas que mudam com Shift/AltGr, campo de texto no alto e clique para inserir. Mostra as teclas mortas (´ ~ ^) do ABNT2 destacadas, o que é útil em pt-BR · **Média/M**
- Pop-up "Fonte:" para ver o glifo em outras fontes · **Baixa/P**
- Pré-visualização: mostrar nome Unicode e código lado a lado, num painel mais baixo · **Baixa/P**
- Campo de busca por nome ("seta", "grau") · **Baixa/M** (exige uma tabela de nomes compacta)

### 1.5 Visualizador (`crates/viewer`)
**(a) Referência.** `jaguar/10-2-Jaguar-Preview`: barra de ferramentas com ícones grandes e rótulo (Miniaturas | Ampliar, Reduzir | Girar à esquerda, Girar à direita | Voltar, Avançar (cinza quando não dá) | campo "Número da página"), botão-pílula de esconder a barra no título e rolagem horizontal e vertical em gel azul. Abre PDF com várias páginas e gaveta de miniaturas.
**(b) Glove.** Fundo cinza com a mensagem "Abra uma imagem pelo Arquivos" e nada mais (`glove-view.png`). O zoom, o giro e a navegação ficam nos menus. Só PNG/JPEG.
**(c) Lacuna.** Faltam a barra de ferramentas, as miniaturas, o PDF e um botão Abrir. As barras de rolagem não aparecem.
**(d) Tarefas.**
- Barra de ferramentas (kit `toolbar`) com os mesmos 8 itens, ícones originais (lupa +/−, seta curva) · **Alta/M**
- Arquivo ▸ Abrir… (file_sheet) e estado vazio com botão "Abrir…" · **Alta/P** (depois do file_sheet)
- PDF: **empacotar** `mutool draw -o - -F png` (MuPDF) renderizando só a página atual. O campo de página, Voltar e Avançar passam a valer para páginas · **Média/M**
- Gaveta lateral de miniaturas (imagens da pasta ou páginas do PDF), geradas sob demanda e com limite de RAM · **Média/M**
- GIF (1º quadro), WebP e BMP via crates puros · **Baixa/P**
- Barras de rolagem visíveis quando a imagem passa do tamanho da janela · **Média/P**

### 1.6 Terminal (`crates/term`)
**(a) Referência.** `panther/system-utilities-commandprompt_` e `jaguar/10-2-Jaguar-Console`: fundo branco com texto preto monoespaçado, título "Terminal — bash — 80x24", ícone de divisão de painel no topo da rolagem e cursor em bloco cinza.
**(b) Glove.** Muito fiel (`glove-term.png`): o título segue o mesmo padrão e o núcleo é o do Alacritty. A diferença é o prompt verde e azul, que vem das cores do bash do usuário.
**(c) Lacuna.** Pequena. Faltam "Configurações da janela" (cores e cursor por interface), abas ou divisão, e um "Console" para ver registros.
**(d) Tarefas.**
- Janela "Inspetor do Terminal" com cor de fundo/texto, transparência, perfis (Básico, Gel, Grafite) e tamanho da fonte · **Média/M**
- Botão de dividir a janela no topo da rolagem (dois viewports sobre o mesmo PTY, com histórico) · **Baixa/M**
- Arrastar arquivo do Arquivos para o terminal insere o caminho com aspas · **Baixa/P**

### 1.7 Monitor (`crates/monitor`)
**(a) Referência.** O Process Viewer do Jaguar (`10-2-Jaguar-Process-Viewer`) tem "Find:" com campo, pop-up "Show: All Processes", colunas Name/User/Status/% CPU/% Memory com **a coluna ordenada destacada em azul**, "36 processes", "Sample every [20] seconds ▲▼" e o triângulo "More Info". O Activity Monitor do Panther (`panther/system-managers-tasks_-1-1`, `-1-3`) tem barra de ferramentas (Encerrar processo em sinal vermelho de STOP, Inspecionar, Filtro, pop-up Mostrar "Meus processos"), as colunas PID/Nome com ícone/Usuário/%CPU/Threads/Memória real, e **abas embaixo: CPU, Memória do sistema, Atividade de disco, Uso do disco, Rede**, cada uma com legenda de cor e gráfico em blocos verde/vermelho sobre fundo preto.
**(b) Glove.** Gráfico de CPU em linha verde sobre preto, barra de memória, tabela ordenada fixa por CPU, nada para filtrar e o rodapé "254 processos" (`glove-monitor.png`).
**(c) Lacuna.** Não dá para ordenar pelas colunas nem filtrar. Faltam o pop-up "Mostrar", o painel com abas, as threads e o detalhamento de memória (ativa/inativa/cache, swap). O gráfico fica no alto, e nas referências fica embaixo.
**(d) Tarefas.**
- Tabela do kit com ordenação por clique no cabeçalho e destaque azul · **Alta/M** (vem do kit)
- Campo "Filtro" e pop-up "Mostrar: Meus processos / Todos / Janelas (apps Glove)" · **Alta/P**
- Barra de ferramentas Encerrar processo / Inspecionar (folha com caminho, memória, arquivos abertos em `/proc/<pid>`) · **Média/M**
- Painel inferior com abas CPU (usuário/sistema/ocioso, gráfico em blocos), Memória (pizza desenhada por código: livre/ativa/cache/swap), Disco (leituras/escritas de `/proc/diskstats`), Rede (`/proc/net/dev`). Só a aba visível é lida · **Média/M**
- Stepper "Atualizar a cada N s" (economia em máquinas lentas) · **Baixa/P**
- Opção do Dock: ícone com gráfico de CPU ao vivo (como o Relógio) · **Baixa/M**

### 1.8 Xadrez (`crates/chess`)
**(a) Referência.** `jaguar/10-2-Jaguar-Chess`: tabuleiro **em perspectiva 3D** sobre **mesa de madeira**, com fundo azul-escuro em degradê, casas azul-ardósia claras e escuras com reflexo, peças metálicas prateadas e grafite, e coordenadas grandes e translúcidas fora do tabuleiro. Tem também janela de registro de lances e opção de jogar por voz.
**(b) Glove.** Tabuleiro 2D marrom e bege com peças de glifo e a mensagem "Sua vez." embaixo (`glove-chess.png`). A lógica está completa.
**(c) Lacuna.** Só o visual: paleta, mesa, perspectiva e o histórico de lances.
**(d) Tarefas.**
- Tema "Ardósia": casas azul-acinzentadas com brilho, moldura escura biselada, mesa de madeira (veios em degradê procedural) e fundo azul-noite. Mantém o 2D e fica leve · **Média/P**
- Vista inclinada opcional (transformação trapezoidal do tabuleiro em `paint/draw`, peças em escala por fileira). Sem 3D real · **Baixa/M**
- Janela "Registro do jogo" com notação algébrica em pt-BR (R, D, T, B, C), salvar em PGN · **Média/P**
- Peças desenhadas por código com gradiente metálico, em vez dos glifos Unicode da fonte · **Baixa/M**

### 1.9 Relógio (`crates/clock`)
**(a) Referência.** Não há janela de relógio nas imagens. O 10.0/10.1 tinha um Clock.app com o ícone do Dock mostrando a hora e um relógio flutuante opcional. No Panther, o relógio da barra de menus abre um menu com a data.
**(b) Glove.** Janela analógica bonita com hora digital e data por extenso (`glove-clock.png`). O ícone do Dock mostra a hora.
**(c) Lacuna.** Pequena.
**(d) Tarefas.** Preferências: digital/analógico, flutuante sem moldura e translúcido, segundos sim/não · **Baixa/P**. Clicar no relógio da barra de menus abre um mini-calendário do mês, que liga com a futura Agenda · **Média/P** (área do compositor).

### 1.10 Busca (`crates/find`)
**(a) Referência.** `jaguar/10-2-Jaguar-Finder-Search`: janela de resultados com a barra de ferramentas do Finder e o campo de busca arredondado com "x", contagem "2 itens", colunas Nome (ordenada, azul) / Data de modificação / Tamanho / Tipo com ícones, e **um painel inferior com o caminho do item selecionado**. No 10.0/10.1, o Sherlock fazia busca por conteúdo.
**(b) Glove.** Barra com lupa, campo, "em [Início]" e Buscar, mais uma lista de nomes (`glove-find.png`).
**(c) Lacuna.** Faltam as colunas (data, tamanho, tipo), a ordenação, o painel de caminho e a busca por conteúdo e por tipo.
**(d) Tarefas.**
- Resultados na tabela do kit com colunas Nome/Data/Tamanho/Tipo e ordenação · **Alta/M**
- Painel de caminho embaixo (pasta › subpasta › arquivo, cada pedaço clicável) · **Média/P**
- Critérios: "conteúdo contém" (lê só arquivos de texto até N MB, em segundo plano), "tipo" (imagem, texto, PDF, áudio), "modificado há" · **Média/M**
- Campo arredondado com "x" e busca enquanto digita (com atraso) · **Baixa/P**

### 1.11 Discos (`crates/disks`)
**(a) Referência.** `jaguar/10-2-Jaguar-Disk-Utility`: à esquerda, a árvore disco → volumes ("38.18 GB" › "Macintosh HD") com a frase "0 discos e 1 volume selecionados". À direita, abas **Informações / Primeiros Socorros / Apagar / Partição / RAID**. A aba de informações mostra ícone grande e rótulos alinhados à direita (Ponto de montagem, Formato, Capacidade com bytes, Disponível, Usado, Número de arquivos e pastas). Embaixo, o cadeado "Clique no cadeado para impedir alterações".
**(b) Glove.** Cartões com ícone, nome, ponto de montagem, dispositivo, sistema de arquivos e barra de uso (`glove-disks.png`). Abrir e ejetar.
**(c) Lacuna.** Falta a hierarquia disco/partição, a aba de informações detalhada e as ações de manutenção.
**(d) Tarefas.**
- Layout de dois painéis: árvore (de `lsblk -J` ou `/sys/block`) e abas · **Média/M**
- Aba Informações com capacidade em GB e bytes, modelo, barramento (USB/SATA/MMC), UUID e rótulo · **Média/P**
- Aba **Apagar** (só removíveis): formatar em FAT32/exFAT/ext4 via `udisksctl`/`mkfs`, com confirmação forte · **Baixa/M**
- Aba **Primeiros Socorros**: verificar (`fsck -n`) só volumes desmontados, com saída num log · **Baixa/M**
- Cadeado de privilégio (polkit via udisks) · **Baixa/M**

### 1.12 Captura (`crates/shot`)
**(a) Referência.** O Grab do 10.x (não há imagem nas pastas) tinha os modos Seleção, Janela, Tela e Tela com atraso, e um Inspetor.
**(b) Glove.** Janela com "Quando: Agora/5 s/10 s" e "Abrir no Visualizador" (`glove-shot.png`). Coerente e limpa.
**(c) Lacuna.** Só captura a tela inteira.
**(d) Tarefas.** Modos **Seleção** (retângulo com cursor em cruz) e **Janela** (clique numa janela), que pedem um protocolo ao compositor via `request_screenshot` · **Média/M**. Copiar para a área de transferência em vez de salvar · **Baixa/P**. Atalhos globais Ctrl+Shift+3/4 na área do compositor · **Média/P**.

### 1.13 Sobre (`crates/about`)
**(a) Referência.** `jaguar/10-2-Jaguar-About-This-Mac`: só o botão fechar está ativo (os outros dois ficam vazios). Um logotipo grande, o nome do sistema, "Versão 10.2", **"Memória: 320 MB", "Processador: 400 MHz PowerPC G4"**, o botão **"Mais informações…"** e o copyright.
**(b) Glove.** Esfera azul, "Glove", versão, frase e OK (`glove-about.png`).
**(c) Lacuna.** Faltam memória e CPU, que importam num sistema para máquinas fracas, e o botão "Mais informações…". Minimizar e zoom deveriam estar desativados.
**(d) Tarefas.** Mostrar memória total (de `/proc/meminfo`), modelo da CPU, núcleos e kernel · **Alta/P**. "Mais informações…" abre o app **Informações do Sistema** (§2) · **Média/P**. Janela sem zoom nem minimizar · **Baixa/P**.

---

## 2. Apps que faltam: decisão

Orçamento de referência: cada app ≤ 10 MB e 0% de CPU parado. Em 500 MB, tudo o que passa de ~80 MB só entra como opção.

| App (ref.) | Nome Glove | Decisão | Por quê | MVP | Prior./Esforço |
|---|---|---|---|---|---|
| Address Book (`jaguar/10-2-Jaguar-Address-Book`, `panther/applications-office-addressbook_-1-2`) | **Contatos** | **Construir** | Só dados locais (vCard), UI de três colunas que o kit já quase cobre, ~4 MB | Janela metal: colunas Grupo / Nome / cartão. Cartão com foto (silhueta desenhada), nome grande, rótulos cinza alinhados à direita ("trabalho", "celular", "e-mail"), Nota. Botões +, Editar, ◀▶. Busca. Arquivos `.vcf` em `~/.local/share/glove/contatos/`, importar e exportar vCard | Média/M |
| iCal (`panther/applications-office-calendar_-1-1…5`) | **Agenda** | **Construir** | Também local (.ics). Muito visível no estilo da época. Encaixa com o relógio da barra de menus | Lista "Calendários" com caixas coloridas, mini-mês com setas ▲◆▼, vistas Dia/Semana/Mês (segmentado), grade de horas com "Meio-dia", hoje destacado em azul, criar evento com clique duplo e arrastar, aviso por notificação, busca. Armazena RFC 5545 simples, com recorrência só semanal e mensal | Média/G |
| iTunes (`jaguar/10-2-Jaguar-iTunes-v3`, `puma/…iTunes-v1`, `panther/applications-multimedia-cdplayer_-1-1`) | **Música** | **Construir** (UI em Rust; decodificação com `symphonia` em Rust puro; som com `cpal`/ALSA ou PipeWire) | Toda máquina leve toca música. O mpv como backend custaria ~25 MB. Com Rust puro a meta é ~10 MB | Janela metal: botões redondos ◀◀ ▶ ▶▶ e volume, **visor LCD verde-claro** com título, artista, tempo e barra de progresso (arte própria, sem logotipo), busca. Fonte: Biblioteca (lê ~/Música, tags ID3/Vorbis), Listas (.m3u), "Tocadas recentemente". Tabela Nome/Tempo/Artista/Álbum/Gênero com caixinha de marcar e zebra azul. Rodapé "11 músicas, 48,1 minutos, 55,7 MB", aleatório e repetir. Formatos: MP3, OGG, FLAC, WAV | Média/G |
| CD player (`panther/applications-multimedia-cdplayer_-1-*`) | — | **Pular** | Quase não há leitor de CD no hardware alvo. Se um dia fizer falta, a Música lê `cdda://` via libcdio | — | — |
| QuickTime (`jaguar/10-2-Jaguar-QuickTime`, `panther/applications-multimedia-mediaplayer_`) | **Vídeo** | **Empacotar o mpv** (no Wayland, com `--title` e um OSC próprio em Lua, pt-BR e cores gel) | Decodificar vídeo em Rust puro não é viável. O mpv usa a GPU e é o mais leve que dá conta | Iniciador `glove-video` que roda o mpv com configuração Glove: OSC parecido com o da referência (moldura metal, linha de tempo LCD "00:00:00", botões ⏮ ◀◀ ▶ ▶▶ ⏭, volume), Arquivos abre vídeos nele, e ícone próprio | Média/P–M |
| Volume (`panther/applications-multimedia-volume_`) | **Volume** (item da barra de menus) | **Construir** no compositor | Essencial. O menu da referência é um slider vertical com bola em gel azul | Ícone de alto-falante na barra, slider vertical, mudo e teclas de mídia. Backend `wpctl` (PipeWire), com `amixer` como alternativa | **Alta/M** (coordenar com quem cuida da barra de menus) |
| Internet Explorer / Safari (`jaguar/10-2-Jaguar-Internet-Explorer`, `panther/applications-internet-browser_-1-1`) | **Navegador** | **Empacotar, escolhendo pela RAM** | Construir navegador está fora de questão. Hoje o Dock chama o **Firefox** (`crates/config/src/lib.rs:242`), que sozinho passa de 300 MB e não cabe em 500 MB | Script `glove-browser`: com <1 GB de RAM usa **NetSurf** (GTK, ~30–50 MB, JS limitado) ou outro WebKit leve. Senão, Firefox com perfil enxuto (sem telemetria, `browser.sessionhistory` pequeno, 1 processo de conteúdo). Tratar a falta de cada um com aviso | **Alta/P** |
| Mail (`jaguar/10-2-Jaguar-Mail`, `-New-Message`, `panther/applications-internet-mail_-1-1`) | **Correio** | **Adiar** e então **construir** um MVP. No curto prazo, empacotar o Claws Mail se estiver instalado | IMAP/SMTP com TLS (rustls, imap, lettre) é viável em ~10–15 MB, mas tem muita borda (OAuth do Gmail, MIME). Para a maioria dos usuários, webmail resolve | Futuro: gaveta de caixas (Entrada, Saída, Rascunhos, Enviados, Lixo), barra Apagar/Responder/Resp. a todos/Encaminhar/Nova/Obter, linha de status "Trabalhando offline — 0 mensagens", tabela De/Assunto/Data com divisor e painel de leitura, composição com Para/Cc/Assunto e anexar. Só texto simples, HTML convertido em texto | Baixa/G |
| iChat (`jaguar/10-2-Jaguar-iChat`) | — | **Pular** | AIM e Rendezvous-chat morreram. Um cliente XMPP/Matrix não compensa o esforço | — | — |
| Key Caps / Keyboard Viewer | (modo **Teclado** do Caracteres) | **Construir** dentro do `crates/chars` | Ver §1.4 | — | Média/M |
| Font panel (`panther/interface-dialogs-font_-1-1`, `-1-2`) | **Painel de Fontes** (kit) | **Construir** como componente do kit | Editor, Terminal, Lembretes e Preferências precisam de um seletor | Colunas Coleções (Todas, Favoritas, Recentes) / Família / Estilo / Tamanho (lista, campo e slider vertical), busca, botões + − ⚙. Lista via fontconfig (`fc-list`, em cache), prévia sob demanda | Média/M |
| Color picker (`panther/interface-dialogs-colourselector_-1-1…6`) | **Painel de Cores** (kit) | **Construir** (só roda + barra) | Lembretes (cores próprias), papel de parede liso, Terminal | Roda de cores HSV desenhada por código, slider vertical de brilho, conta-gotas (via captura do compositor), faixa de amostras salvas e campo hex. Pular as abas de lápis, imagem e paletas | Baixa/M |
| AppleScript Editor (`jaguar/10-2-Jaguar-AppleScript-Editor`) | — | **Pular** | Não há equivalente de automação por eventos de apps no Glove. O Terminal e os scripts de shell bastam | (Se um dia houver, "Automação" rodaria .sh com Gravar/Parar/Executar) | — |
| Console (`jaguar/10-2-Jaguar-Console`) | **Registros** | **Construir** (trivial) | Ajuda o usuário leigo a diagnosticar. Lê `journalctl -f -o short` ou `~/.local/state/glove/*.log` | Janela branca monoespaçada, título com ícone de documento, filtro, "Limpar" e "Marcar". Pode reaproveitar o `text_area` | Baixa/P |
| System Profiler (ref. nas pastas, `*System-Profiler*`) | **Informações do Sistema** | **Construir** (a partir do "Mais informações…" do Sobre) | Mostra o que importa em máquina antiga: RAM, CPU, disco, GPU, rede, USB | Lista à esquerda (Hardware, Memória, Discos, Rede, USB, Software), detalhes à direita (`/proc`, `/sys`, `lspci -mm` se houver) | Baixa/M |
| Internet Connect, Network Utility, Keychain, Print Center, DVD Player, Image Capture | — | **Pular ou deixar para outras áreas** | Rede e impressora ficam na barra de menus e nas Preferências. O resto não se aplica | — | — |

---

## 3. Ordem sugerida
1. **Kit:** tabela ordenável, barra de ferramentas, campo de busca e **diálogo de arquivo em colunas**. Destrava Editor, Visualizador, Monitor e Busca.
2. **Volume** na barra e **Navegador** escolhido pela RAM, ambos rápidos e essenciais em 500 MB. Sobre com memória e CPU.
3. Lembretes com moldura própria e um só processo. Visualizador com barra de ferramentas e PDF. Monitor com filtro e abas.
4. **Música**, depois **Contatos** e **Agenda**. Painel de Fontes.
5. Calculadora científica, modo Teclado no Caracteres, tema Ardósia no Xadrez, Discos em dois painéis, Captura de seleção.
6. Vídeo (mpv), Registros, Informações do Sistema, Painel de Cores. Correio no fim.
