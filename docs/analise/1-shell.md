# Análise 1 — Shell do Glove (barra de menu, menus, Dock, janelas, widgets)

Referências medidas em pixels e convertidas para **pontos (pt)**. As capturas do Jaguar estão em 2400 px de largura sobre uma tela de 1152×870 (fator ≈2,08); as do Panther estão em 1:1 (1024×768). O Glove foi visto a 1920×1200 com escala 1,5. Tudo o que vem abaixo é **comportamento, proporção e linguagem visual**. A arte continua original e desenhada em código (orbe azul próprio, "Arquivos", "Preferências").

---

## 1. Barra de menu

**(a) Referências.** `jaguar/10-2-Jaguar-Desktop`, `panther/desktop-empty_.png`
- Altura de 22 pt. O fundo do Jaguar tem listras horizontais claras com período de ~4 pt (#FFFFFF/#EDEFEF). O Panther usa um degradê cinza claro (#FAFAFA no topo, #E5E5E5 embaixo) com listras quase invisíveis e linha inferior #ABABAB. Logo abaixo da linha há uma sombra curta de 1–2 pt sobre o papel de parede.
- O logo fica numa faixa de ~32 pt. O nome do app vem em **negrito** 14 pt e os outros títulos em regular 14 pt, com ~20 pt de espaço entre eles. O título aberto ganha um realce azul **sólido** em toda a altura, com texto branco (`Finder-Go-Menu`).
- À direita aparecem os **extras de menu**: monitor, volume (e Bluetooth no Panther), depois o relógio "Qui 15:29". No Panther ainda vem o nome do usuário em negrito (`desktop-full_-1-2`). Cada extra é clicável e abre um menu no mesmo estilo dos outros (`Menu-Bar-Displays`: resoluções com ✓, "Milhares/Milhões", "Abrir Monitores…").

**(b) Glove hoje.** `chrome::menubar` desenha as listras com `menu_bg_a/b`, o logo, os títulos (o primeiro em negrito), um realce em degradê e só o relógio.

**(c) Lacuna.** Faltam os extras (volume, monitor, bateria, rede). O realce usa degradê com `lighten(accent, .15)`, quando nas referências é praticamente chapado (#3B76C7 no Jaguar, #3165B5 no Panther). Também falta a sombra sob a barra.

**(d) Tarefas**
- Criar a API de "extras" em `chrome::menubar`: receber `&[MenuExtra{icon, text}]` e devolver as faixas em `MenuBarLayout.extras`. Em `shell.rs`, `BarTarget::Extra(i)` abriria menus de Volume (slider num item), Monitor (resoluções vindas de `udev.rs`) e Bateria (/sys/class/power_supply). **Alta / M**
- Usar realce de cor chapada (`accent` com no máximo 6% de variação) na barra e nos menus. **Média / P**
- Pôr uma sombra de 2 pt (alfa 0,25→0) sob a barra, desenhada no próprio canvas ou como faixa no `render.rs`. **Baixa / P**
- Criar uma opção de relógio "Qua 15:29" ou "15:29" e, opcional, o nome do usuário (estilo Panther). **Baixa / P**

## 2. Menu principal (equivalente ao menu da maçã) e menus de app

**(a) Referências.** `jaguar/10-2-Jaguar-Apple-Menu`, `cheetah/...-Apple-Menu`, `*-Finder-Menu`
- O menu do sistema tem uma ordem fixa: Sobre este computador · Obter software… · ― · Preferências do Sistema… · Dock ▸ · Local ▸ · ― · Itens Recentes ▸ · ― · Forçar Encerrar… · ― · Repouso · Reiniciar… · Desligar… · ― · Encerrar Sessão… ⇧⌘Q. Os submenus usam um triângulo cheio ▶ alinhado à direita.
- O menu do app segue o padrão: Sobre X · ― · Preferências… ⌘, · ― · Serviços ▸ · ― · Ocultar X ⌘H · Ocultar Outros · Mostrar Tudo · ― · Encerrar X ⌘Q. Os apps têm Arquivo, Editar, Janela e Ajuda; o menu Janela traz "Minimizar ⌘M" e a lista de janelas.

**(b) Glove hoje.** `Shell::open_main_menu` tem Sobre, Preferências, Novo Terminal, 3 itens "Tema:" e Sair. O menu do app (`open_bar_menu`, `Title(0)`) só tem Ocultar/Encerrar. Não existem submenus.

**(c) Lacuna.** A estrutura está longe da referência: os temas ocupam espaço do menu principal, faltam Forçar Encerrar, Repouso/Reiniciar/Desligar, Itens Recentes, submenu Dock e os menus Janela/Ajuda.

**(d) Tarefas**
- Implementar **submenus** em `chrome::MenuItem` (`submenu: Vec<MenuItem>` e a seta ▶ desenhada em vetor). Em `shell.rs`, guardar uma pilha de `OpenMenu` e abrir com o mouse parado (~0,2 s) ou com a seta → no `input.rs`. **Alta / M**
- Reestruturar o menu principal: Sobre este computador · Preferências… · Dock ▸ (Ampliação liga/desliga, Ocultar automaticamente, Preferências do Dock…) · Itens Recentes ▸ · Forçar Encerrar… ⌥⌘Esc · Repouso/Reiniciar…/Desligar… (logind via `systemctl`) · Encerrar Sessão… ⇧⌘Q. Os temas passam para as Preferências. **Alta / M**
- Montar o menu do app completo: Sobre {app}, Preferências…, Ocultar {app}, **Ocultar Outros**, **Mostrar Tudo** e Encerrar. Acrescentar um menu **Janela** gerado pelo compositor (Minimizar, Zoom, ―, lista de janelas do app com ✓ na ativa e ◆ nas minimizadas), juntado aos menus do cliente em `bar_titles`. **Alta / M**

## 3. Desenho dos menus

**(a) Referências.** `Finder-Go-Menu`, `Menu-Bar-Displays`, `panther/desktop-full_-1-2`
- O fundo é de **listras** (#F6F7F9 ↔ #E5E7E8, período de ~4 pt), com leve translucidez (~95%). Os cantos são **retos**, sem arredondamento, e a sombra é forte para baixo (~8 pt de desfoque e alfa de ~0,45).
- Cada item tem **19 pt** de altura (Panther 1:1) e fonte de 14 pt. O texto começa ~20 pt depois da borda; o ✓ fica em ~8 pt e os ícones pequenos de 16 pt ficam antes do texto (menu Ir).
- O separador do Jaguar é **só um vão de ~11 pt, sem linha**. O do Panther é uma linha #D4D4D4 de 1 px. Itens desabilitados ficam em cinza #A0A0A0.
- Os atalhos têm **duas colunas**: os modificadores (⇧⌘) alinhados à direita e a tecla numa coluna fixa à esquerda (`⇧⌘C`, `⌘K`).

**(b) Glove hoje.** `chrome::menu` usa um corpo branco com alfa de 235, listras fracas (alfa 120), cantos inferiores de raio 5, `MENU_ITEM_H = 20`, linha separadora e atalhos como "Ctrl ⇧ S" numa única string.

**(c) Lacuna.** As listras quase não aparecem (veja `menu-principal.png`). Os cantos são arredondados, a sombra é fraca, faltam ícones nos itens e falta o alinhamento em colunas dos atalhos.

**(d) Tarefas**
- Em `chrome::menu`: listras opacas (#F6F7F9/#E6E8EA), alfa 245 no corpo, cantos retos e sombra `blur 8, offset_y 4, alpha .45`. Separador opcional por tema: vão sem linha (Listras) ou linha #D4D4D4 (Metal/Grafite). **Média / P**
- `MENU_ITEM_H = 19` e `SEP_H = 11`. **Baixa / P**
- Atalhos em duas colunas: dividir `shortcut_label` em (modificadores, tecla) e medir a largura máxima de cada coluna em `menu_content_size`. Desenhar Ctrl/Alt/Super com glifos próprios (⌃ ⌥ e um glifo original para Super) ou manter os nomes curtos, mas sempre na ordem ⌃⌥⇧Super. **Média / P**
- Adicionar `MenuItem.icon: Option<Icon>`, desenhado com 16 pt via `icons::draw`, para o menu Ir do Arquivos e para o Forçar Encerrar. **Baixa / P**

## 4. Dock

**(a) Referências.** `jaguar/*-Desktop`, `*-Command-Tab`, `panther/desktop-empty_`, `system-managers-applicationmanager_-1-*`, `jaguar/*-System-Preferences-Dock`
- A prateleira é um **retângulo encostado na borda de baixo**, sem margem, com cantos superiores de no máximo ~2 pt. O material é branco translúcido (~40–45%) com **listras horizontais finas** visíveis e uma linha clara de 1 pt no topo e nas laterais. O separador entre apps e documentos/Lixeira é uma linha dupla (escura + clara) que ocupa a altura toda.
- Os ícones ficam **sem sombra elíptica e sem reflexo** por baixo, porque a sombra é parte da arte de cada ícone. A margem interna é de ~3 pt e o espaço entre ícones fica entre ~4 e 8 pt.
- O indicador de execução é um **triângulo preto apontando para cima**, ~7 pt de largura, colado na borda inferior da tela.
- O rótulo do item sob o mouse é **texto branco em negrito de ~14 pt com contorno/sombra escura, sem pílula de fundo**, centralizado ~10 pt acima do ícone (`Command-Tab`: "Terminal"; `applicationmanager_-1-3`: "iPhoto").
- Ampliação: até ~128 pt, com curva suave por ~3 ícones de cada lado. A prateleira **não cresce na altura**, só na largura (`applicationmanager_-1-3`).
- Comportamentos de época: o ícone **pula** ao abrir um app, controlado por "Animar abertura de aplicativos". A minimização usa **Gênio/Escala** e a janela minimizada vira uma **miniatura viva da janela**, com o ícone do app no canto. Clicar e segurar abre um **menu de contexto** com as janelas do app (✓ na ativa), "Manter no Dock", "Mostrar no Arquivos", "Ocultar Outros" e "Forçar Encerrar"; esse menu tem um bico que aponta para o ícone (`panther/desktop-full_-1-2`). Também existem ocultar automaticamente e posição esquerda/baixo/direita.

**(b) Glove hoje.** `chrome::dock` desenha uma prateleira em degradê sem listras, de raio 3. A sombra elíptica sob cada ícone vem da arte em `icons.rs` (veja `dock.png`). O rótulo fica numa pílula escura (alfa 120, 13 pt). Já existem triângulo de execução, ampliação animada (`DOCK_BASE 56`, `DOCK_MAX 80`) e itens minimizados com o ícone genérico `Icon::Window`.

**(c) Lacuna.** O rótulo em pílula pertence a uma época posterior. A prateleira não tem listras. As sombras elípticas sob os ícones não existem na referência. Faltam pulo ao abrir, efeito de minimizar, miniatura da janela, menu de contexto, ocultar automaticamente e posição lateral. A ampliação máxima é tímida (80 pt).

**(d) Tarefas**
- **Rótulo:** em `chrome::dock`, trocar a pílula por texto negrito de 14 pt branco, com 8 cópias deslocadas em ±1 pt na cor rgba(0,0,0,.6) (contorno) mais uma sombra de 1 pt. **Alta / P**
- **Prateleira:** chamar `c.pinstripes` com alfa ~0,12 sobre o degradê, raio de 2 pt, encostar na borda (`DOCK_PAD_Y` só por dentro) e pôr uma linha clara de 1 pt em volta. **Média / P**
- **Sombras dos ícones:** tirar as elipses de sombra em `icons::draw` quando o contexto for Dock (parâmetro `shadow: bool`). **Média / P**
- **Pulo ao abrir:** em `shell.rs`, guardar `launching: HashMap<app_id, Instant>`; em `prepare_dock`, deslocar o ícone em `y -= 18·|sin(π·t/0,6s)|` até a janela aparecer (no máximo ~10 s). **Alta / M**
- **Minimizar com efeito "Escala"** (barato): animar em 0,3 s o retângulo da janela até a posição do item no Dock, usando o `TextureBuffer` da janela com escala em `render.rs` (`build_elements`). O Gênio, com malha deformada, fica opcional para depois. **Alta / G**
- **Miniatura viva:** ao minimizar, capturar a textura da janela num canvas de 48 pt (como já faz `render.rs::screenshot`) e desenhar o ícone do app de 16 pt no canto inferior direito, em vez de `Icon::Window`. **Média / M**
- **Menu de contexto do Dock:** botão direito ou pressionar 0,6 s abre um `chrome::menu` com bico de 10 pt apontando para o ícone, com as entradas da referência. **Alta / M**
- **Ocultar automaticamente e posição esquerda/direita:** novos campos em `glove_config` mais `dock_pos`/`work_area` em `shell.rs`. **Baixa / M**
- Subir `DOCK_MAX` para ~112 e deixar ajustável nas Preferências (slider "Tamanho" e "Ampliação"). **Baixa / P**

## 5. Moldura de janela (barra de título, botões, sombras)

**(a) Referências.** `jaguar/*-Command-Tab`, `*-About-This-Mac`, `panther/desktop-full_-1-*`, `jaguar/*-System-Preferences-Dock`
- A barra de título tem 22 pt. No Jaguar ela é **listrada, sem degradê forte** (#F5F5F5 / #E9E9E9) e tem uma linha inferior cinza. No Panther vira degradê cinza com listras sutis. O título usa 13 pt regular centralizado, preto #000; nas janelas inativas fica cinza #8C8C8C.
- O título pode trazer um **ícone de proxy** de 16 pt à esquerda (o ícone da pasta "Utilities").
- Os botões são **gel de ~14 pt de diâmetro**, com o primeiro centro em ~14 pt e espaçamento de ~21 pt. Em janela inativa no Jaguar, os botões viram **círculos vazios só com contorno cinza** e um leve brilho; no Panther, viram gel cinza-claro. Um botão indisponível (zoom em diálogos, ou minimizar no "Sobre") aparece **desbotado**, como em `About-This-Mac`, onde só o fechar tem cor. Os símbolos ×, − e + aparecem ao passar o mouse sobre o grupo.
- O **botão de barra de ferramentas** é uma pílula branca de ~22×12 pt no canto superior direito; ele mostra ou oculta a barra.
- Inativa no Jaguar: a barra de título fica **translúcida** (~70–80%), deixando ver o que está atrás (`Command-Tab`: "Utilities", "System Preferences").
- Metal (Panther): a janela inteira é de metal escovado, a barra de título continua no corpo sem linha divisória e os cantos inferiores são arredondados (`panther/desktop-full_-1-1`, janela Applications).
- A sombra da ativa é grande e escura (desfoque de ~20 pt e deslocamento de ~8 pt); a da inativa é menor.

**(b) Glove hoje.** `chrome::titlebar`: degradê #F8F8F8→#CECED0 com listras brancas e alfa 0,35, título embossed de 13 pt, botões `BUTTON_R 6.5` nos centros 14/34/54. A janela inativa usa gel cinza `inactive_button` e as sombras vêm de `shadow_spec`. O tema Metal só pinta a barra de título.

**(c) Lacuna.** O degradê está mais escuro que no Jaguar. Faltam ícone de proxy, pílula de barra de ferramentas, estado desabilitado por botão, inativa translúcida e janela inteira em metal. O espaçamento de 20 pt está correto. Na captura `glove-monitor.png` as listras da barra quase não aparecem.

**(d) Tarefas**
- Separar `TitleBarState` em `buttons_enabled: [bool;3]`, com o cliente pedindo via xdg (janelas de tamanho fixo → zoom desabilitado), e `proxy_icon: Option<Icon>`. Botão desabilitado = `gel_circle` com alfa 0,35. **Alta / P**
- Inativa no tema Listras: desenhar os botões como círculo de contorno #A8A8A8 com brilho branco (novo `gel_ring` em `draw.rs`) e a barra com alfa ~0,8. **Média / P**
- Barra no tema Listras: degradê mais claro (#F6F6F6→#E2E2E2) e listras com alfa 0,6. **Média / P**
- **Pílula de barra de ferramentas:** desenhar em `titlebar` quando o cliente anunciar uma barra (extensão no protocolo de menus em `menus.rs`) e enviar o evento "toggle-toolbar" ao app. **Média / M**
- **Janela Metal completa:** o tema Metal (ou um pedido do app) faz o compositor desenhar também a moldura lateral e o rodapé metálico, ou o kit pinta o fundo com `brushed_metal`, sem a linha inferior da barra. **Baixa / G**
- Sombra da ativa com `offset_y` de 8 e alfa de 0,55. **Baixa / P**

## 6. Folhas (sheets) e alertas

**(a) Referências.** `jaguar/*-Finder-Empty-Trash`, `puma/*-Finder-Copy-Replace`, `cheetah/*-USB-Dismount`
- O alerta é uma janela listrada **sem título**, ou uma **folha que desce de dentro da barra de título** da janela-mãe, presa a ela e animada em ~0,25 s.
- Ícone de 64 pt à esquerda (stop vermelho ou triângulo de aviso amarelo), com o **ícone do app em miniatura de 32 pt sobreposto** no canto. Título em **negrito de 13 pt** e texto explicativo em 11 pt regular. Botões à direita: Cancelar em branco e o padrão em **azul pulsante** (o brilho do botão padrão oscila em ~1,5 s).
- Progresso indeterminado em **listras diagonais azuis animadas** ("barber pole"), em `Finder-Copy-Replace`.

**(b) Glove hoje.** Não há componente de alerta nem de folha. `default_button` é azul estático.

**(d) Tarefas**
- Criar `ui.alert(icon, title, text, buttons)` no kit (`ui.rs`), seguindo o layout acima. **Alta / M**
- Implementar folhas no compositor: tratar `xdg_toplevel` com pai e modal posicionado sob a barra de título do pai, com animação de "descida" por clip em `render.rs`. A janela-pai fica bloqueada. **Média / G**
- Fazer o botão padrão pulsar: alfa do brilho = 0,75 + 0,25·sin(t), pedindo quadro a ~20 fps só enquanto o botão está visível (lembrar do custo de CPU). **Média / P**
- `ui.progress(value: Option<f32>)`: gel azul determinado e listras diagonais animadas para o modo indeterminado. **Alta / P**

## 7. Widgets padrão (glove-kit)

**(a) Referências.** `*-System-Preferences-Dock`, `panther/system-managers-tasks_-1-1`, `Force-Quit`
- **Botões:** pílulas de 20–21 pt de altura e texto de 13 pt, com cantos totalmente redondos. Já correspondem.
- **Slider:** trilho fino de 5 pt. O polegar é uma **gota/escudo apontando para baixo** quando há marcas de graduação; sem marcas, é redondo.
- **Barras de rolagem:** 15 pt de largura, trilho listrado em relevo e **polegar de gel azul em forma de cápsula**. As **duas setas ficam juntas na extremidade** (padrão de época). Aparecem nas listas do Force Quit e do Activity Monitor.
- **Abas:** segmento de gel com a aba ativa azul, centralizado sobre uma caixa em relevo (`tasks_-1-1`: CPU | Memória do sistema…).
- **Listas:** linhas alternadas brancas e azul-claro (#EDF3FE no Panther), seleção azul #3875D7, cabeçalho de coluna em gel claro com a coluna ordenada em azul.
- **Campo de busca:** cápsula com lupa (Panther).

**(b) Glove hoje.** `ui.rs` tem button, checkbox, radio, slider (polegar circular), popup (boa correspondência), text_field, tabs, list e uma barra de rolagem **só em `text_area`**. `list` rola só com a roda, sem barra visível (veja `glove-monitor.png`).

**(d) Tarefas**
- Criar um `scrollbar()` genérico em gel, com as setas juntas, reutilizado por `list` e `text_area`. **Alta / M**
- Polegar do slider em gota e marcas de graduação: `slider_ticks(n)`. **Baixa / P**
- `search_field` em cápsula com lupa vetorial. **Baixa / P**
- `table` com cabeçalho em gel, coluna ordenada em azul e linhas alternadas, para o Monitor e para o Forçar Encerrar. **Média / M**

## 8. Alternador de apps (Command-Tab)

**(a) Referências.** `jaguar/*-Command-Tab`, `cheetah/*`, `puma/*`
- Do 10.0 ao 10.2 **não há painel**: ⌘Tab percorre os apps **no próprio Dock**, que mostra o rótulo em negrito sobre o ícone selecionado (o Dock amplia se a ampliação estiver ligada). O app vem para a frente ao soltar.
- No 10.3 aparece um painel central translúcido escuro (raio ~20 pt), com ícones de 128 pt, o selecionado num quadro claro e o nome embaixo.

**(b) Glove hoje.** `KeyAction` "cycle" (Super+Tab) troca de janela sem nenhum retorno visual.

**(d) Tarefas**
- Fase 1, fiel ao Jaguar: enquanto Super estiver pressionado, guardar `switcher: Option<usize>` em `shell.rs`; `prepare_dock` usa o mesmo realce e rótulo do hover e ativa a ampliação centrada no item. Ao soltar Super, foca o app. Ordem: MRU de apps, não de janelas. **Alta / M**
- Fase 2, opcional, estilo Panther: painel desenhado num canvas em `chrome.rs` (`app_switcher`). **Baixa / M**

## 9. Forçar Encerrar

**(a) Referência.** `jaguar/*-Force-Quit`: janela "Forçar Encerrar Aplicativos" com texto explicativo de 13 pt, lista branca de apps (ícone de 16 pt + nome de 13 pt, seleção azul), dica "Você pode abrir esta janela pressionando ⌥⌘Esc" em 11 pt e botão padrão "Forçar Encerrar". Os botões de minimizar e zoom ficam desabilitados.

**(b) Glove hoje.** O Monitor encerra processos, mas não existe esse diálogo.

**(d) Tarefas**
- Desenhar o diálogo no próprio compositor, para que funcione mesmo com apps travados (os clientes podem estar congelados): canvas em `chrome.rs` com dados de `menu_clients`/PIDs e `kill -9`. Atalho Ctrl+Alt+Esc e entrada no menu principal. Marcar "(não responde)" em vermelho quando o cliente não responder ao `xdg_wm_base.ping`. **Alta / M**

## 10. Sobre este computador

**(a) Referência.** `jaguar/*-About-This-Mac`: janela de ~265×310 pt sem minimizar e sem zoom, logotipo grande, nome do sistema em serifada de ~36 pt, "Versão 10.2" em 11 pt, **Memória / Processador** em 13 pt, botão "Mais informações…" e copyright em 10 pt.

**(b) Glove hoje.** `glove-about` mostra orbe, "Glove", versão, descrição e OK. Faltam memória, processador e "Mais informações…".

**(d) Tarefas**
- Em `crates/about`: ler memória (/proc/meminfo) e CPU (/proc/cpuinfo), trocar OK por "Mais informações…" (abre o Monitor ou um painel do sistema) e desabilitar minimizar/zoom (depende da tarefa 5). **Média / P**

## 11. Mesa, papel de parede e cursor

**(a) Referências.** `jaguar/*-Desktop`, `panther/desktop-empty_`
- O ícone do disco fica no canto superior direito (~64 pt de ícone), com rótulo branco negrito de 12 pt e sombra escura. Seleção: um quadro cinza translúcido em volta do ícone e o rótulo numa pílula escura com texto branco.
- Papel de parede: azul com arcos de luz suaves. O Glove já tem "Ondas" original, o que está correto.
- Cursor: seta preta com contorno branco e sombra suave.

**(b/c) Glove hoje.** Está bom. Em `desktop.png` aparece uma **faixa horizontal clara em y≈1000–1040 px** no papel de parede, que parece emenda de degradê em `wallpaper.rs`. Só existe um ícone de mesa. O realce do rótulo usa `accent` (no Jaguar é cinza-escuro translúcido).

**(d) Tarefas**
- Corrigir a faixa em `wallpaper::render` (dithering/emenda). **Média / P**
- Ícones de mesa para pendrives montados, vindos de `crates/disks`/udisks, mais arrastar e soltar na Lixeira para ejetar. **Média / M**
- Cursores de espera (cata-vento colorido original, girando, ativado se o cliente não responder ao ping por mais de 2 s) e de redimensionamento. **Média / M**

---

## Lista priorizada de tarefas

| # | Tarefa | Arquivos | Prioridade | Esforço |
|---|---|---|---|---|
| 1 | Rótulo do Dock: texto branco com contorno, sem pílula | `chrome::dock` | Alta | P |
| 2 | Submenus nos menus (▶, pilha de menus) | `chrome.rs`, `shell.rs`, `input.rs` | Alta | M |
| 3 | Menu principal reestruturado (Forçar Encerrar, Repouso/Reiniciar/Desligar, Dock ▸, Recentes ▸) | `shell::open_main_menu` | Alta | M |
| 4 | Menu do app completo e menu Janela gerado pelo compositor | `shell::open_bar_menu`, `bar_titles` | Alta | M |
| 5 | Extras de menu: volume, monitor, bateria | `chrome::menubar`, `shell.rs` | Alta | M |
| 6 | Alternador de apps no Dock (estilo 10.0–10.2) | `input.rs`, `shell.rs` | Alta | M |
| 7 | Forçar Encerrar desenhado pelo compositor e detecção de "não responde" | `chrome.rs`, `shell.rs` | Alta | M |
| 8 | Pulo do ícone ao abrir app | `shell::prepare_dock` | Alta | M |
| 9 | Animação de minimizar (Escala) até o Dock | `render.rs`, `shell.rs` | Alta | G |
| 10 | Menu de contexto do Dock com bico | `chrome.rs`, `shell.rs` | Alta | M |
| 11 | Barra de rolagem genérica em gel e `progress()` com barber pole | `kit/ui.rs` | Alta | M |
| 12 | `ui.alert` e botão padrão pulsante | `kit/ui.rs` | Alta | M |
| 13 | Botões de título desabilitados e ícone de proxy | `chrome::titlebar` | Alta | P |
| 14 | Menus: listras opacas, cantos retos, sombra forte, atalhos em 2 colunas, item de 19 pt | `chrome::menu` | Média | P |
| 15 | Prateleira listrada encostada na borda; ícones sem sombra elíptica | `chrome::dock`, `icons.rs` | Média | P |
| 16 | Miniatura viva da janela minimizada | `render.rs`, `shell::dock_model` | Média | M |
| 17 | Janela inativa: botões só com contorno e barra translúcida | `chrome.rs`, `draw.rs` | Média | P |
| 18 | Pílula de barra de ferramentas | `chrome.rs`, `menus.rs` | Média | M |
| 19 | Folhas (sheets) presas à janela-mãe | compositor | Média | G |
| 20 | Tabela com cabeçalho em gel e linhas alternadas | `kit/ui.rs` | Média | M |
| 21 | Faixa no papel de parede; ícones de pendrives na mesa | `wallpaper.rs`, `shell.rs` | Média | P/M |
| 22 | Cursor de espera (cata-vento original) | `chrome.rs`, `render.rs` | Média | M |
| 23 | "Sobre" com memória e CPU | `crates/about` | Média | P |
| 24 | Realce chapado, sombra sob a barra de menu | `chrome.rs` | Baixa | P |
| 25 | Ocultar Dock automaticamente, posição lateral, ampliação até 112 | `config`, `shell.rs` | Baixa | M |
| 26 | Polegar do slider em gota, campo de busca em cápsula | `kit/ui.rs` | Baixa | P |
| 27 | Painel de alternância estilo Panther; janela Metal completa | `chrome.rs` | Baixa | G |
