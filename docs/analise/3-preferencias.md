# Análise 3 — Preferências (glove-prefs)

Escopo: janela principal + todos os painéis vistos nas referências 10.0–10.3, com decisão **Implementar / Adaptar / Pular** e backend Linux. Prioridade: **Alta/Média/Baixa**; esforço: **P** (≤1 dia), **M** (2–4 dias), **G** (≥1 semana).
Arquivos Glove: `crates/prefs/src/main.rs` (571 linhas, 8 painéis), `crates/config/src/lib.rs` (`Config`, `Dock`, `Keyboard`, `Pointer`, `Touchpad`, `Monitor`, `Shortcut`), `crates/kit/src/ui.rs` (tem `tabs`, `slider`, `popup`, `list`, `checkbox`, `radio_group`, `text_field`, `icon_tile`, `group`).

---

## 0. Linguagem visual comum (todas as versões)

O que as referências mostram em **todos** os painéis (`jaguar/*-Dock`, `*-General`, `panther/settings-*`):

- **Largura fixa** (~620 pt em Jaguar, ~595 pt em Panther) e **altura variável por painel**: a janela encolhe/cresce com animação ao trocar de painel (Dock ≈ 380 pt de altura, Keyboard ≈ 570, Startup Disk ≈ 320). O título da janela vira o nome do painel (em Displays vira o nome do monitor: "DELL E190S", "eMac").
- **Barra de ferramentas**: "Mostrar Todos" à esquerda, **separador pontilhado vertical**, depois os favoritos (padrão: Monitores, Som, Rede, Disco de Inicialização). Ícones 32 px + rótulo 11 pt. Botão "pílula" no canto direito do título esconde a toolbar. Em Panther o item ativo ganha fundo cinza afundado (`settings-menu_-1-1.png`, `settings-display_-1-1.png`).
- **Formulário de duas colunas**: rótulos alinhados à **direita** terminando em ":", controles alinhados à esquerda numa coluna comum (`jaguar/...-General`, `...-CDs-and-DVDs`, `...-Dock`). Texto explicativo pequeno embaixo do controle, alinhado com ele.
- **Abas Aqua** centralizadas sobre uma caixa em relevo (group box) que ocupa a largura; em Jaguar abas grandes com linha azul; em Panther abas "segmentadas" pequenas (`panther/settings-power_-1-1.png`).
- **Sliders com marcas** (tick marks) e rótulos nas extremidades/pontos ("1 min / 15 min / 1 hr / 3 hrs / Never"), polegar em **ponta de seta** azul.
- **Cadeado** no rodapé ("Clique no cadeado para impedir mais alterações") em painéis de sistema: Accounts, Energy Saver, Network, Sharing, Date & Time, Startup Disk.
- Panther: botão de ajuda "?" redondo lilás no canto inferior direito.

**Glove hoje** (`glove-prefs.png`, `p1.png`, `p23.png`): janela fixa 660×480, toolbar com 8 painéis centrados (padrão de app "Preferências" de 10.3+ de aplicativo, não de System Preferences), conteúdo em lista vertical de cabeçalhos em negrito + controles indentados; sliders lisos sem marcas, rótulos "Lenta/Rápida" à esquerda/direita; muito espaço vazio.

**Tarefas de kit (pré-requisito de quase tudo):**
| # | Tarefa | Prio | Esf |
|---|---|---|---|
| K1 | `slider_ticks(value, min, max, n_ticks, labels: &[(f32,&str)])` com polegar em seta e marcas | Alta | M |
| K2 | `form_row(label, label_w, |ui| ...)` — rótulo à direita com ":" + coluna de controles; `form_note()` | Alta | P |
| K3 | API para o app pedir **novo tamanho de janela** (`ui.request_size(w,h)`) + compositor animar o redimensionamento de janela `fixed` (xdg_toplevel min=max) | Alta | M |
| K4 | `tab_box(&mut idx, labels, height, |ui| ...)` — abas centradas sobre group box em relevo (estilo Jaguar) | Alta | P |
| K5 | `table(cols, rows, sel)` com cabeçalho e coluna de checkbox ("On", "Ocultar") | Média | M |
| K6 | `lock_footer(&mut unlocked)` — cadeado + texto; desbloqueio via **polkit** (`pkcheck`/`pkexec` com ação `org.glove.prefs.admin`) | Média | M |
| K7 | `image_strip(thumbs, sel)` — faixa horizontal rolável de miniaturas (Desktop, Minha Conta) | Média | M |
| K8 | `stepper` (setinhas ▲▼ ao lado de campo, p/ hora) e `mini_calendar` | Baixa | M |

---

## 1. Janela principal ("Mostrar Todos")

**(a) Referências.**
- 10.0 Cheetah (`cheetah/10-0-Cheetah-System-Preferences-0-scaled.png`): grade **alfabética** sem categorias, 7 colunas, texto "Arraste suas preferências favoritas para a barra" no topo.
- 10.1 Puma (`puma/10-1-Puma-System-Preferences-0-scaled.png`) e 10.2 Jaguar (`jaguar/10-2-Jaguar-System-Preferences-0-scaled.png`): 4 seções **Pessoal / Hardware / Internet e Rede / Sistema**, título da seção em negrito 13 pt, linha divisória fina entre seções, ícones 32 px, rótulos com quebra em 2 linhas ("Energy / Saver"), 7 colunas (~88 pt cada).
- 10.3 Panther (`panther/settings-menu_-1-1.png`): mesmas 4 seções, mas **faixas alternadas** cinza claro/mais claro em vez de linhas; menu Visualizar alterna para **ordem alfabética** (`settings-menu_-1-2.png`). Não há campo de busca em Panther nas referências (busca só chega no 10.4) — tratar como extra opcional.
- Clique simples abre o painel; a janela troca de tamanho; Cmd+L / "Mostrar Todos" volta.

**(b) Glove.** Não existe grade. Toolbar fixa com 8 painéis; abre via `--pane` (argumento).

**(c) Gap.** Falta toda a navegação de dois níveis, a toolbar de favoritos com separador, categorias, título dinâmico, redimensionamento.

**(d) Tarefas.**
- P1 **Modelo de painéis**: `struct PaneDef { id, icon, label, category, backend_ok: fn()->bool }`; categorias fixas `Pessoal, Hardware, Internet e Rede, Sistema`. Esconder painéis cujo backend não existe (ex.: Bluetooth sem `bluetoothd`). — Alta, P
- P2 **Grade "Mostrar Todos"** estilo Jaguar (linhas divisórias) com opção Panther (faixas) conforme tema; 7 colunas, rótulos com quebra. — Alta, M
- P3 **Toolbar de favoritos**: "Mostrar Todos" + separador pontilhado + favoritos (padrão `monitor, som, rede, disco-inicial`); arrastar ícone da grade para a toolbar; arrastar para fora remove. Config: `prefs.toolbar: Vec<String>`. — Média, M
- P4 Menu Visualizar: "Organizar por Categorias / Alfabeticamente" + lista de painéis; atalho Super+L = Mostrar Todos. Config: `prefs.alphabetical: bool`. — Baixa, P
- P5 Campo de busca (extra pós-Panther, opcional): filtra por palavras-chave de cada `PaneDef` e destaca ícones. — Baixa, P
- P6 Título da janela = nome do painel; lembrar último painel (`prefs.last_pane`). — Média, P
- P7 Mover "Atalhos" para dentro de Teclado (aba, como Panther `settings-keyboard_-1-3.png`) e "Terminal" para as preferências do próprio app Terminal (menu Terminal ▸ Preferências), pois não é preferência de sistema. — Média, P

**Distribuição proposta** (Jaguar como base, com Panther onde ajuda):
- **Pessoal**: Aparência, Mesa e Protetor de Tela, Dock, Internacional, Itens de Início*, Minha Conta*, Acesso Universal (Puma o põe aqui)
- **Hardware**: Monitores, Economizador de Energia, Teclado e Mouse (ou separados), Trackpad, Som, Bluetooth, CDs e DVDs/Mídia Removível, Impressão (opcional)
- **Internet e Rede**: Rede, Compartilhamento, Internet (apps padrão)
- **Sistema**: Contas, Data e Hora, Atualização de Software, Disco de Inicialização, Fala (opcional)
(*em Panther ambos migram para Contas; Glove pode seguir Panther e deixar só "Contas" com abas.)

---

## 2. Painéis

### 2.1 Aparência / Geral — **Implementar**
(a) `cheetah/...-General`: Aparência + Cor de destaque + clique na barra. `jaguar/...-General`: + setas de rolagem "Em cima e embaixo / Juntas", itens recentes (Apps/Documentos), suavização de fonte + tamanho mínimo. `panther/settings-appearance_-1-1.png`: + "Rolagem suave", "Minimizar com duplo clique na barra de título". Pop-ups com amostra de cor à esquerda.
(b) Glove: rádio Listras/Metal/Grafite + pop-up de papel de parede, tudo num painel "Aparência".
(c) Falta cor de destaque, opções de barra de rolagem, duplo clique, recentes, suavização; papel de parede deveria sair daqui (vai para Mesa).
(d) Tarefas:
- Pop-up "Aparência:" (Azul=Listras / Grafite; "Metal" vira opção extra) com amostra de cor. Config existente `theme`. Alta, P
- "Cor de destaque:" (Azul, Grafite, Prata, Verde, Roxo, Laranja, Vermelho, Amarelo, Outra…) → `appearance.highlight: String` lido pelo kit para seleção de texto/listas. Média, P
- Setas de rolagem juntas/separadas; clique na barra "Ir para a próxima página / Rolar até aqui" → `appearance.scroll_arrows`, `appearance.scroll_jump` (kit). Média, P
- "Minimizar com duplo clique na barra de título" → `appearance.dblclick_minimize` (compositor). Média, P
- Itens recentes (Apps/Documentos: 0,5,10,15,20,30,50) → `appearance.recent_apps/recent_docs`, usados pelo menu da maçã-equivalente; gravar `~/.local/share/recently-used.xbel` (padrão freedesktop). Baixa, P
- Suavização: "Padrão (CRT) / Leve / Média (LCD plano) / Forte" mapeando para hinting/antialias do rasterizador do kit (e `~/.config/fontconfig/fonts.conf` para apps externos) + "Desligar suavização para fontes ≤ N". Baixa, M

### 2.2 Mesa (Desktop) / Mesa e Protetor de Tela — **Implementar**
(a) `jaguar/...-Desktop`: miniatura grande "Imagem atual" + nome; "Coleção:" pop-up; faixa horizontal de miniaturas; "Trocar imagem: a cada 30 minutos" + "Ordem aleatória". Panther (`settings-screensaver_-1-1/2`) funde com protetor: abas "Mesa | Protetor de Tela".
(b) Glove: pop-up "Imagem: Ondas/Aurora/Liso" + PNGs de ~/Imagens.
(c) Falta preview, miniaturas, coleções, rotação, modo de ajuste.
(d) Tarefas: painel "Mesa e Protetor de Tela" com aba Mesa: preview 160×120, coleções (`Glove` = fundos procedurais; `Imagens` = ~/Imagens; "Escolher pasta…"), faixa K7, pop-up de ajuste (Preencher/Ajustar/Centralizar/Lado a lado, Panther), cor sólida. Config: `desktop.collection`, `desktop.fit`, `desktop.change_every_min: u32` (0 = não), `desktop.random`. Miniaturas geradas em thread e cacheadas em `~/.cache/glove/thumbs` (limitar RAM: 7 visíveis × 96×72). Alta, M

### 2.3 Protetor de Tela / Efeitos de Tela — **Implementar (leve)**
(a) `puma/...-Screen-Saver` e `jaguar/...-Screen-Effects`: abas "Efeitos | Ativação | Cantos Ativos", lista à esquerda, preview preto à direita, "Configurar" / "Testar", "Abrir Economizador". Panther: slider "Iniciar após: 3,5,15,30,1h,2h,Nunca", "Aleatório", botão "Cantos Ativos…" (folha com 4 pop-ups em volta de miniatura — `settings-screensaver_-1-2.png`).
(b) Glove: nada.
(d) Tarefas: compositor já sabe o tempo ocioso → implementar via **ext-idle-notify-v1**/timer interno; protetores desenhados no compositor (baratos: "Nome do Computador", "Fluxo" (linhas tipo Flurry original), "Pasta de Imagens" slideshow, "Preto"). Pedir senha ao sair (Ativação) → bloqueio com PAM. Cantos ativos: ação por canto (`Iniciar protetor`, `Desativar protetor`, `Mostrar mesa`, `Exposé`, `—`). Config: `screensaver.module`, `screensaver.idle_min`, `screensaver.random`, `screensaver.ask_password`, `hot_corners: [String;4]`. Média, G (protetores+bloqueio); Cantos ativos Média, P.

### 2.4 Dock — **Implementar (quase pronto)**
(a) `jaguar/...-Dock`: slider Tamanho (Pequeno–Grande), checkbox+slider Ampliação (Mín–Máx) **na mesma linha**, Posição Esquerda/Embaixo/Direita (rádios com rótulo embaixo), "Minimizar usando: Efeito Gênio/Escala", "Animar abertura de aplicativos", "Ocultar e mostrar automaticamente".
(b) Glove: tamanho, checkbox ampliação, máximo (`Dock.size/magnification/max_size`).
(c) Falta posição, efeito de minimizar, animação de abertura, auto-ocultar; layout não é o de formulário.
(d) Novos campos `dock.position: "left|bottom|right"`, `dock.minimize_effect: "genie|scale"`, `dock.animate_launch: bool`, `dock.autohide: bool`; refazer layout com K1/K2. Compositor implementa (posição e autohide são do compositor). Alta, M (UI P + compositor M).

### 2.5 Internacional — **Implementar**
(a) `jaguar/...-International`: abas Idioma | Data | Hora | Números | Menu de Entrada; lista arrastável de idiomas, "Editar…". Panther (`settings-international_-1-1..6`): Idiomas | Formatos (Região, Datas/Horas/Números com "Personalizar…" e folha) | Menu de Entrada (lista de layouts com checkbox + "Mostrar menu de entrada na barra").
(b) Glove: layout de teclado está em Teclado (pop-up com 10 layouts xkb).
(d) Tarefas:
- Aba **Idioma**: ordem de idiomas → `LANGUAGE` e `LANG` em `~/.config/environment.d/glove-locale.conf` (vale no próximo login); listar só locales instalados (`locale -a`). Média, M
- Aba **Formatos**: região → `LC_TIME/LC_NUMERIC/LC_MONETARY/LC_MEASUREMENT`; prévia de data/hora/número; o relógio da barra de menus deve respeitar. Config `locale.region`. Média, M
- Aba **Menu de Entrada**: lista de layouts xkb com checkbox (múltiplos) + atalho de troca + "Mostrar menu de entrada na barra" → `keyboard.layouts: Vec<(layout,variant)>`, `keyboard.show_input_menu`. Compositor troca via xkbcommon. Alta, M
- Sistema todo (tela de login): `localectl set-locale / set-x11-keymap` com polkit, atrás do cadeado. Baixa, P

### 2.6 Login / Itens de Início / Contas / Usuários / Minha Conta — **Implementar**
(a) `cheetah/...-Login`, `puma/...-Login`: abas Itens de Início | Janela de Login. `jaguar/...-Login-Items`: tabela "Ocultar | Item | Tipo", "Remover"/"Adicionar…". `jaguar/...-My-Account`: Usuário, Senha "Alterar…", Minha Imagem (poço + faixa de imagens), cartão. `puma/...-Users` e `jaguar/...-Accounts`: tabela Imagem/Nome/Tipo, "Novo Usuário…", "Editar Usuário…", "Capacidades…", "Login automático…", "Apagar Usuário…", "Entrar automaticamente como X", cadeado.
(b) Glove: nada.
(d) Tarefas:
- **Itens de Início**: tabela K5; grava `.desktop` em `~/.config/autostart/` (XDG Autostart) com `X-Glove-Hidden=true` para "Ocultar"; o compositor/sessão executa no login. Alta, M
- **Minha Conta** (ou aba em Contas): nome completo e imagem via **AccountsService** D-Bus (`org.freedesktop.Accounts.User.SetRealName/SetIconFile`); senha via `passwd` em PTY ou `chpasswd` com polkit. Imagens de usuário **originais** desenhadas em código (6–8 figuras simples). Média, M
- **Contas** (admin, atrás do cadeado): listar `Accounts.ListCachedUsers`, criar `CreateUser(name, fullname, type)`, apagar `DeleteUser`, tipo Admin = grupo `sudo`/`wheel`; login automático via config do gerenciador de login (greetd `initial_session` ou GDM/LightDM `autologin-user`) — detectar qual existe. Média, G

### 2.7 Monitores (Displays) — **Implementar (melhorar)**
(a) `jaguar/...-Displays` e `panther/settings-display_-1-1..3`: título = nome do monitor; abas Monitor | Geometria | Cor; **lista** de resoluções à esquerda, à direita "Cores:" e "Taxa de atualização:", "Mostrar modos recomendados", "Mostrar monitores na barra de menus"; Panther: sliders Brilho/Contraste.
(b) Glove (`p1.png`): pop-up de monitor, pop-up de resolução, pop-up "Tamanho da interface", confirmação com contagem regressiva (ótimo, manter).
(c) Layout diferente; falta taxa separada da resolução, brilho, cor (gama/luz noturna), arranjo multi-monitor.
(d) Tarefas: lista de resoluções (sem Hz) + pop-up "Taxa de atualização" + pop-up "Tamanho da interface" no lugar de "Cores"; título da janela = `OutputInfo.description`. Alta, P. Slider **Brilho** via `/sys/class/backlight/*` com **logind** `SetBrightness` (sem root). Alta, P. Aba **Cor**: gama/temperatura via `wlr-gamma-control` interno do compositor (luz noturna). `monitor.gamma`, `monitor.night_temp`. Baixa, M. Aba **Organizar** (multi-monitor, arrastar retângulos; espelhar) → `monitor.layout: Vec<{output,x,y}>`. Média, G. "Mostrar monitores na barra de menus" → extra de menu (§ barra). Baixa, P.

### 2.8 Economizador de Energia — **Implementar**
(a) `jaguar/...-Energy-Saver`: abas Repouso | Opções; slider com marcas "1 min…15 min…1 h…3 h…Nunca" (escala não linear) para computador e (checkbox) para monitor; aviso amarelo se monitor dorme antes do protetor; "Pôr o disco em repouso"; cadeado. Panther (`settings-power_-1-1..3`): + aba Agenda; Opções: despertar por rede, botão de energia põe para dormir, reiniciar após falta de energia.
(b) Glove: nada.
(d) Tarefas:
- Sliders K1 com mapeamento não linear (1–60 min de 1 em 1 até 15, depois 5 em 5, 1–3 h, Nunca). Repouso do monitor: compositor faz DPMS (DRM `ACTIVE=0`) após `power.display_sleep_min`. Repouso do computador: compositor chama **logind** `org.freedesktop.login1.Manager.Suspend` após `power.system_sleep_min` (respeitar inibidores, `Inhibit("idle")` durante vídeo). Alta, M
- Se houver bateria (**UPower** `org.freedesktop.UPower`), pop-up "Configurações para: Adaptador / Bateria" (Panther PowerBook) e "Mostrar estado da bateria na barra" → `power.ac.*`, `power.battery.*`, `power.show_battery_menu`. Alta, M
- Opções: "Botão de energia: Repouso/Desligar/Perguntar" (compositor intercepta `KEY_POWER` com `HandlePowerKey` inibido), "Tampa fechada: Repouso". Média, P
- Disco em repouso: `hdparm -S` exige root → Pular (SSD/irrelevante). Agenda (ligar/desligar): `rtcwake` exige root → Baixa, M.

### 2.9 Teclado — **Implementar (refazer layout)**
(a) `jaguar/...-Keyboard`: abas Ajustes | Acesso Total pelo Teclado; dois sliders com marcas lado a lado "Taxa de repetição (Lenta–Rápida)" | "Atraso (Desligado, Longo–Curto)", separador vertical, campo "Digite aqui para testar". Panther (`settings-keyboard_-1-1..3`): "Teclado e Mouse" com abas Teclado | Mouse | Bluetooth | Atalhos de Teclado (tabela com grupos recolhíveis, checkbox On, +/−, "Restaurar Padrões", "Acesso total pelo teclado").
(b) Glove: Layout (pop-up), atraso e velocidade (sliders horizontais empilhados), campo de teste; Atalhos é painel separado.
(d) Tarefas: layout em duas colunas igual Jaguar com K1 (inclui posição "Desligado" → `keyboard.repeat_delay=0` desativa repetição); layout de teclado sai para Internacional ▸ Menu de Entrada (deixar atalho "Layout…" aqui). Alta, P. Aba **Atalhos** (Panther) com tabela K5 agrupada (Janelas, Captura de Tela, Acesso Universal, Apps) e checkbox para desativar → `Shortcut.enabled: bool`. Alta, M. "Acesso total pelo teclado" (Tab foca todos controles no kit) → `keyboard.full_access`. Baixa, M. Opções extras Linux: Caps Lock como Ctrl, NumLock ao iniciar → `keyboard.xkb_options: Vec<String>`. Média, P.

### 2.10 Mouse / Trackpad — **Implementar (refazer layout)**
(a) `jaguar/...-Mouse`: "Velocidade do cursor" (Muito lenta–Rápida, 10 marcas), "Velocidade de rolagem", à direita "Velocidade do duplo clique" + "Clique duas vezes aqui para testar". Panther `settings-mouse_-1-1`: mesmo em aba. PowerBook (não há screenshot de trackpad nas refs): "Trackpad" com "Clicar/Arrastar/Trava de arrasto" e "Ignorar toque acidental".
(b) Glove (`p23.png`): velocidade, aceleração, rolagem natural, canhoto; trackpad: tocar, clique secundário, velocidade, rolagem natural, ignorar ao digitar. Funcionalmente mais rico que 10.2, visual diferente.
(d) Tarefas: refazer em duas colunas com sliders com marcas; adicionar "Velocidade de rolagem" (`pointer.scroll_factor`, aplicado no compositor multiplicando eixos libinput), "Velocidade do duplo clique" (`pointer.double_click_ms`, publicado pelo compositor/config para o kit) com área de teste; trackpad: "Arrastar" e "Trava de arrasto" (libinput `tap_drag`, `tap_drag_lock`), "Rolagem com dois dedos / borda" (`scroll_method`). Manter extras Linux (natural, canhoto) em seção "Opções". Alta, P–M. Opcional: juntar como "Teclado e Mouse" com abas (Panther) e mostrar Trackpad só se libinput detectar touchpad. Média, P.

### 2.11 Som — **Implementar**
(a) `jaguar/...-Sound`: abas Efeitos Sonoros | Saída | Entrada; tabela de sons de alerta (Nome/Tipo), "Tocar alertas por:", slider volume de alerta com ícones alto-falante, "Efeitos sonoros da interface", "Retorno ao pressionar teclas de volume"; fora das abas: **Volume de saída** + "Mudo" + "Mostrar volume na barra de menus".
(b) Glove: nada (verificar se há teclas de volume no compositor).
(d) Tarefas:
- Backend: **PipeWire via `wpctl`** (simples, sem libs) ou `pactl` (compatível PulseAudio/pipewire-pulse); idealmente lib `libpulse` binding leve. Saída/Entrada: lista de dispositivos (`wpctl status`/`pactl list sinks short`), escolher padrão, balanço; Entrada: nível de entrada (medidor) + volume. Alta, M
- Volume de saída + Mudo global; teclas de volume (XF86AudioRaiseVolume…) no compositor com OSD e "bip" de retorno. Alta, M
- Sons de alerta **originais** (sintetizados, 6–8 sons pequenos .ogg/.wav gerados em código) → `sound.alert`, `sound.alert_volume`, `sound.ui_effects`, `sound.volume_feedback`, `sound.show_menu`. Média, M
- Menu de volume na barra (extra) — Média, M.

### 2.12 Rede — **Implementar**
(a) `jaguar/...-Network` (e `cheetah`/`puma` similares): "Local:" pop-up, "Mostrar:" interface, abas TCP/IP | PPPoE | AppleTalk | Proxies; Configurar: DHCP/Manual; IP, máscara, roteador, DNS, domínios de busca, MAC; "Reverter"/"Aplicar agora"; cadeado. (Panther adiciona "Status da Rede" com bolinhas verdes — não há screenshot, mas é o mesmo painel.)
(b) Glove: nada.
(d) Tarefas: backend **NetworkManager por D-Bus** (`org.freedesktop.NetworkManager`; usar `zbus` se já no projeto, senão `nmcli -t` como primeira versão). Alta, G no total:
- "Mostrar:" = dispositivos NM (Ethernet, Wi-Fi) + "Status da Rede". Aba **TCP/IP**: Configurar Usando DHCP / Manualmente / DHCP com endereço manual (`ipv4.method auto|manual`), exibir IP/máscara/roteador/DNS (`IP4Config`), editar; "Aplicar agora" → `Update2` + `ActivateConnection`. Alta, M
- **Wi-Fi** (aba "AirPort" na época; aqui "Wi-Fi"): lista de redes, conectar com senha, "Mostrar Wi-Fi na barra" → menu de rede. Alta, M
- **PPPoE** (NM `pppoe` setting) — Baixa, P; **Proxies** (HTTP/HTTPS/SOCKS → `~/.config/environment.d/proxy.conf`) — Baixa, P; AppleTalk → **Pular** (inexistente; no lugar: aba "VPN" opcional via NM). "Local" = perfis NM (`connection.id`) — Baixa, P.

### 2.13 Compartilhamento — **Adaptar**
(a) `jaguar/...-Sharing`: Nome do computador, nome Rendezvous `.local`, endereço de rede; abas Serviços | Firewall | Internet; tabela On/Serviço; "Parar/Iniciar" + explicação; URL de acesso.
(b) nada.
(d) Nome do computador → `hostnamectl set-hostname --pretty` (systemd-hostnamed D-Bus, polkit); nome `.local` = hostname estático (Avahi). Serviços mapeados a **units systemd** via `org.freedesktop.systemd1` (StartUnit/EnableUnitFiles, polkit): Compartilhamento de arquivos (Samba `smb.service`), Acesso remoto (`sshd`), Compartilhamento web (opcional), Impressoras (`cups` browsing). Só listar serviços cujos units existem. Firewall: se `ufw` ou `firewalld` instalado, liga/desliga + portas por serviço; senão esconder aba. Aba Internet (compartilhar conexão) → NM `ipv4.method shared`. Média, M (serviços) / Baixa, M (firewall/Internet).

### 2.14 Data e Hora — **Implementar**
(a) `jaguar/...-Date-and-Time`: abas Data e Hora | Fuso Horário | Hora da Rede | Relógio da Barra; calendário mensal com setas, campo HH:MM:SS com stepper, **relógio analógico** grande, Reverter/Salvar, "Abrir Internacional", cadeado. Panther (`settings-timedate_-1-1..3`): fuso com **mapa-múndi** clicável + "Cidade mais próxima"; aba Relógio: mostrar data e hora, na barra/janela, digital/analógico, segundos, dia da semana, piscar separadores, 24 h.
(d) Backend **systemd-timedated** D-Bus (`org.freedesktop.timedate1`: `SetTime`, `SetTimezone`, `SetNTP`), polkit (cadeado).
- Aba Data e Hora (calendário K8 + relógio analógico — reaproveitar desenho de `glove-clock`) + "Ajustar automaticamente" (NTP) no topo. Alta, M
- Fuso: pop-up de regiões/cidades a partir de `/usr/share/zoneinfo/tzdata.zi` ou `timedatectl list-timezones`; mapa-múndi **desenhado em código** (contornos simplificados próprios, faixa do fuso destacada) — Média, M (mapa G, pode ficar para depois).
- Aba Relógio (relógio da barra): `clock.show`, `clock.seconds`, `clock.weekday`, `clock.flash_sep`, `clock.24h`, `clock.analog` → `shell.rs::clock_text()` lê config. Alta, P.

### 2.15 Atualização de Software — **Adaptar**
(a) `jaguar/...-Software-Update`: texto explicativo, abas Atualizar | Atualizações Instaladas; "Verificar automaticamente" + frequência (Diariamente/Semanalmente/Mensalmente), próxima/última verificação, "Verificar Agora".
(d) Backend **PackageKit** D-Bus (`org.freedesktop.PackageKit`: `GetUpdates`, `UpdatePackages`) — distribuição-agnóstico; fallback "abrir Terminal com `sudo apt upgrade`". Histórico: `GetOldTransactions`. Cuidado com RAM: PackageKit daemon pesa (~30–60 MB); só ativar sob demanda. Config `update.auto`, `update.interval`. Baixa, M.

### 2.16 Disco de Inicialização — **Adaptar / Baixa**
(a) `jaguar/...-Startup-Disk`: ícones de sistemas (Mac OS 9 / X / Rede), seleção, "Reiniciar…", cadeado.
(d) Linux: listar entradas do **systemd-boot** (`bootctl list`) ou **GRUB** (`grub-editenv`/`grub-reboot`) e oferecer "Reiniciar nesta entrada uma vez" (logind `SetRebootToBootLoaderEntry` funciona sem root com systemd-boot). Ícones originais de disco. Se nenhum suportado, mostrar só "Reiniciar…". Baixa, M. Está nos favoritos padrão da toolbar — no Glove trocar por **Teclado** ou **Mouse** nos favoritos padrão.

### 2.17 Acesso Universal — **Implementar (parcial)**
(a) `puma/...-Universal-Access`, `jaguar/...-Universal-Access`, `panther/settings-accessibility_-1-1..4`: "Tenho dificuldades com:" abas Visão | Audição | Teclado | Mouse; botões grandes "Ativar Zoom", "Mudar para Branco sobre Preto", "Tons de cinza", slider "Aumentar contraste"; Audição: piscar a tela no alerta; Teclado: Teclas de Aderência / Lentas; Mouse: Teclas do Mouse, tamanho do cursor.
(d) Tudo no compositor (barato com shaders/renderer Smithay): zoom de tela seguindo cursor (`a11y.zoom`, atalhos), inversão de cores e tons de cinza (pós-processo), **tamanho do cursor** (`a11y.cursor_size`), piscar tela em alerta (`a11y.flash`), teclas de aderência/lentas via **xkbcommon** (sticky keys implementado no compositor), Teclas do Mouse (numpad move ponteiro). Alta para cursor/zoom/inversão (M), Média para aderência/lentas (M), Baixa teclas do mouse (M). "Habilitar acesso a dispositivos assistivos" → Pular (AT-SPI ausente no kit).

### 2.18 Fala — **Pular (opcional Baixa)**
(a) `jaguar/...-Speech`: Reconhecimento | Voz Padrão | Interface Falada.
(d) Reconhecimento: pular (sem engine leve). Voz Padrão: `espeak-ng` se instalado (lista de vozes `espeak-ng --voices=pt`, velocidade, "Reproduzir"); usado por "Anunciar a hora". Baixa, M.

### 2.19 ColorSync — **Pular**
(a) `jaguar/...-ColorSync`, `cheetah`/`puma`: perfis padrão RGB/CMYK/Cinza, CMMs. Sem relevância para 500 MB/compositor sem gestão de cor; perfil de monitor no máximo vira item da aba Cor de Monitores (colord opcional). Pular.

### 2.20 CDs e DVDs → "Mídia Removível" — **Adaptar, Média**
(a) `jaguar/...-CDs-and-DVDs`: 5 pop-ups "Ao inserir CD vazio / DVD vazio / CD de música / CD de imagens / DVD de vídeo" com ícone do app.
(d) Generalizar para **pendrive, cartão, CD/DVD, celular**: eventos de **UDisks2** D-Bus (`InterfacesAdded`) monitorados pelo daemon de arquivos/Finder; ações: Perguntar / Abrir Arquivos / Abrir Visualizador / Abrir app… / Ignorar. Config `media.on_usb`, `media.on_audio_cd`, `media.on_photos` (DCIM detectado), `media.on_video_dvd`, `media.automount: bool`. Média, M.

### 2.21 QuickTime — **Pular**
(a) `jaguar/...-QuickTime`: Plug-in, Conexão, Música, Teclas de mídia, Atualização. Tudo produto-específico. O único conceito útil (teclas de mídia) já se cobre em Som/Teclado (MPRIS com `playerctl`). Pular.

### 2.22 Classic — **Pular**
(a) `cheetah/puma/jaguar/...-Classic`: iniciar ambiente Mac OS 9. Sem equivalente. Pular (não reinterpretar como Wine/Flatpak para não confundir).

### 2.23 Internet — **Adaptar como "Aplicativos Padrão"**
(a) `cheetah/...-Internet`, `puma/...-Internet-1`, `jaguar/...-Internet`: abas .Mac | iDisk | E-mail (leitor padrão) | Web (navegador padrão, página inicial, pasta de downloads).
(d) Pular .Mac/iDisk. Abas **Web** e **E-mail**: navegador e cliente de e-mail padrão via `xdg-mime default <app>.desktop x-scheme-handler/http(s)|mailto` (grava `~/.config/mimeapps.list`), pasta de downloads via `xdg-user-dirs-update --set DOWNLOAD`. Acrescentar Terminal padrão (`Config.terminal`, já existe) e Editor de texto padrão. Média, P.

### 2.24 Painéis Panther adicionais (sem screenshot de pane, mas na grade)
- **Exposé** (`settings-menu_-1-1`): atalhos e cantos ativos para Todas as janelas / Janelas do app / Mesa — depende do recurso existir no compositor; fundir com Cantos Ativos. Média, P (UI).
- **Bluetooth**: BlueZ D-Bus (`org.bluez`: Adapter1 `Powered`, `Discoverable`, `StartDiscovery`; Device1 `Pair/Connect`); "Configurar Novo Dispositivo…"; bateria do mouse/teclado (`Battery1`, ver `settings-keyboard_-1-2.png`); só mostrar se há adaptador. Média, M.
- **Segurança**: "Pedir senha ao sair do repouso/protetor", "Sair após N minutos de inatividade" → mesma infra do protetor/bloqueio. Média, P.
- **Impressão e Fax**: CUPS (`lpstat`, `lpadmin`, polkit) — Baixa, M (ou botão que abre `system-config-printer` se instalado).

---

## 3. Novos campos de config (resumo)
```
[prefs]      toolbar, alphabetical, last_pane
[appearance] highlight, scroll_arrows, scroll_jump, dblclick_minimize, recent_apps, recent_docs, font_smoothing, smoothing_min_px
[desktop]    collection, fit, change_every_min, random      (wallpaper continua no topo por compat)
[screensaver] module, idle_min, random, ask_password ; hot_corners=[4]
[dock]       + position, minimize_effect, animate_launch, autohide
[keyboard]   + layouts: Vec<(layout,variant)>, show_input_menu, full_access, xkb_options ; Shortcut.enabled
[pointer]    + scroll_factor, double_click_ms ; [touchpad] + tap_drag, tap_drag_lock, scroll_method
[monitor]    + brightness, layout(multi), gamma/night_temp
[power]      ac{system_sleep_min, display_sleep_min}, battery{...}, power_button, lid, show_battery_menu
[sound]      alert, alert_volume, ui_effects, volume_feedback, show_menu
[clock]      show, seconds, weekday, flash_sep, h24, analog
[a11y]       zoom, invert, grayscale, contrast, cursor_size, flash, sticky_keys, slow_keys
[media]      on_usb, on_audio_cd, on_photos, on_video_dvd, automount
[locale]     region (idioma vai para environment.d)
```
Estado do sistema (rede, contas, hora, hostname, serviços) **não** vai para `config.toml`: é lido/escrito direto no daemon via D-Bus. Recomendo um módulo `crates/prefs/src/sys/` com um arquivo por backend (`nm.rs`, `accounts.rs`, `timedate.rs`, `logind.rs`, `upower.rs`, `pipewire.rs`, `bluez.rs`, `udisks.rs`, `systemd.rs`) usando **zbus em modo bloqueante numa thread** (sem tokio) para manter RAM baixa; fallback por CLI (`nmcli`, `wpctl`, `timedatectl`) quando o crate pesar demais.

---

## 4. Lista de tarefas priorizada

**Alta**
1. K2 form_row + K4 tab_box + K1 slider com marcas (kit) — P+P+M
2. K3 janela com altura por painel + título dinâmico — M
3. P1/P2 modelo de painéis + grade "Mostrar Todos" com 4 categorias — P+M
4. Toolbar "Mostrar Todos" + separador + favoritos fixos (P3 sem arrastar) — P
5. Dock: posição, auto-ocultar, efeito, layout Jaguar — M
6. Teclado: layout Jaguar 2 colunas + aba Atalhos (mover painel Atalhos) — P+M
7. Mouse/Trackpad: layout Jaguar, rolagem e duplo clique — M
8. Monitores: lista de resoluções + taxa + brilho (logind) — P
9. Som: volume/mudo/dispositivos (wpctl/pactl) + teclas de volume — M
10. Economizador de Energia: sliders não lineares, DPMS, suspensão logind, UPower — M
11. Rede: NM — TCP/IP + Wi-Fi + status — G
12. Data e Hora: timedated + NTP + aba Relógio da barra — M
13. Mesa: preview + miniaturas + coleções (K7) — M
14. Aparência: pop-up aparência/destaque + duplo clique minimiza — P
15. Itens de Início (XDG autostart) — M
16. Menu de Entrada (multi-layout xkb) — M
17. Acesso Universal: cursor grande, zoom, inversão — M

**Média**
18. K5 tabela, K6 cadeado/polkit, K7 faixa de miniaturas — M
19. Protetor de tela + bloqueio + Cantos Ativos/Exposé — G
20. Contas/Minha Conta (AccountsService) — G
21. Internacional: Idioma e Formatos (locale) — M
22. Compartilhamento (hostnamed + systemd units) — M
23. Mídia Removível (UDisks2) — M
24. Aplicativos Padrão (xdg-mime) — P
25. Bluetooth (BlueZ), Segurança — M
26. Fuso horário com mapa original; Monitores multi-tela — G
27. Arrastar favoritos para a toolbar; mover Terminal para prefs do app — M

**Baixa**
28. Atualização de Software (PackageKit), Disco de Inicialização (bootctl), Fala (espeak-ng), Impressão (CUPS), Proxies/PPPoE, suavização de fonte, itens recentes, ordem alfabética/busca, aba Cor (gama/luz noturna), teclas do mouse.

**Pular**: ColorSync, QuickTime, Classic, .Mac/iDisk, AppleTalk, Reconhecimento de fala, repouso de disco.
