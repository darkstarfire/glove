# Plano do Glove

Mapa do que falta para o Glove ficar fiel ao Mac OS X da era Aqua "gel", do 10.0 ao **10.4 Tiger**
(Cheetah, Puma, Jaguar, Panther e Tiger), **no comportamento, nas proporções e na linguagem visual**,
sempre com arte própria desenhada por código (sem logotipos, ícones ou código da Apple) e com nomes
próprios para os recursos (ex.: "Busca Rápida", não o nome da Apple).

**Base: Tiger (10.4)**, o último da era antes da virada do 10.5 Leopard (Dock 3D, janelas cinza lisas,
barra de menu translúcida). As fases 0 e 1 foram feitas olhando o 10.0–10.3; a etapa 2.0 compara
o Glove com o Tiger e decide o que muda.

Depois de cumprir tudo da era (fases 1 a 9.11), vem a **Fase 10**: o Glove "como deveria ter sido",
com ferramentas de luxo e recursos modernos, mantendo o visual gel.

O levantamento comparou 334 capturas de referência com o Glove atual. As análises
completas, com medidas, cores e arquivos de referência, estão em [`docs/analise/`](analise/):

| Análise | Área |
|---|---|
| [1-shell.md](analise/1-shell.md) | Barra de menu, menus, Dock, janelas, controles, alternador, Forçar Encerrar |
| [2-arquivos.md](analise/2-arquivos.md) | Arquivos (Finder), Abrir/Salvar, lixeira, cópia, Obter Informações |
| [3-preferencias.md](analise/3-preferencias.md) | Preferências: grade "Mostrar Todos" e cada painel, com o serviço Linux de cada um |
| [4-sessao-login.md](analise/4-sessao-login.md) | Tela de login, splash, desligar/reiniciar, bloqueio, primeiro uso, utilitários |
| [5-apps.md](analise/5-apps.md) | Apps existentes e apps que faltam |
| [6-tiger.md](analise/6-tiger.md) | Tiger (10.4) comparado ao Glove: o que muda com o Tiger como base |

As capturas de referência ficam em [`referencias/`](../referencias/) (com as fontes de cada uma). Origem:
use as páginas da [512pixels](https://512pixels.net/projects/aqua-screenshot-library/) (10.0–10.2) e da
[GUIdebook](https://guidebookgallery.org/screenshots/macosx103) (10.3).

## Como cada etapa é feita

1. Implementar a etapa (uma por vez, na ordem abaixo).
2. Compilar com `./build.sh` (apaga antes os restos de compilações antigas). A compilação usa 6
   núcleos (`.cargo/config.toml`), para a máquina continuar usável.
3. Testar: `cargo test --release` e capturas de tela comparadas com as referências.
4. Medir a memória quando a etapa mexer em processo residente (meta: máquina de 500 MB).
5. Marcar `[x]` aqui e fazer **um commit por etapa**, enviado ao GitHub.
6. Subir a versão (`version` em `[workspace.package]` do `Cargo.toml`) e criar a tag `vX.Y.Z`, também
   enviada ao GitHub. Esquema: **0.(fase).(etapa)**: `v0.9.0` = fases 1 a 9.9 completas; cada etapa da
   9.10 sobe o último número (9.10.1 → `v0.9.1`, …, 9.10.11 → `v0.9.11`); a Fase 10 começa em
   `v0.10.0` (10.1 → `v0.10.1`…). Correções soltas entre etapas não mudam a versão.

Legenda de esforço: **P** pequeno (horas), **M** médio (1–2 dias), **G** grande (vários dias).

---

## Fase 0 — Feito

- [x] Sessão própria (DRM/KMS + libinput), Monitor/Mouse/Trackpad nas Preferências
- [x] 0.1 Dock: ampliação entra e sai suave (sem salto no primeiro movimento)
- [x] 0.2 `build.sh` limpa compilações antigas; referências baixadas

## Fase 1 — Acabamento visual rápido (o que mais salta aos olhos)

- [x] 1.1 Rótulo do Dock: texto branco em negrito com contorno escuro, sem pílula (P)
- [x] 1.2 Prateleira do Dock listrada e encostada na borda; ícones sem a sombra elíptica (P)
- [x] 1.3 Papel de parede: tirar a faixa clara (emenda do degradê) (P)
- [x] 1.4 Menus: listras opacas, cantos retos, sombra forte, itens de 19 pt, atalhos em 2 colunas (P)
- [x] 1.5 Janela inativa: botões só com contorno e barra translúcida; botões de título desabilitáveis (P)
- [x] 1.6 Calculadora com visor verde-claro e botões neutros, como a referência (P)
- [x] 1.7 "Sobre o Glove" com memória, processador e versão (P)
- [x] 1.8 **Tema "Claro"** (padrão, visual do Tiger): barra de menu em degradê liso, menus com separador em linha e
  destaque azul liso, Dock liso, janelas em degradê cinza; Listras fica como o visual 10.0–10.2 (M)

## Fase 2 — glove-kit: os controles que faltam (destrava as outras fases)

- [x] 2.0 Referências do Tiger em `referencias/tiger/` (512pixels) e análise das diferenças ([6-tiger.md](analise/6-tiger.md)) (M)
- [x] 2.1 Barra de rolagem em gel genérica (listas, tabelas, colunas) (M)
- [x] 2.2 Barra de progresso com listras animadas (indeterminada) e giratório (M)
- [x] 2.3 Alerta padrão com ícones originais de Pare/Cuidado/Nota e botão padrão pulsante (M)
- [x] 2.4 Controle segmentado com glifos; barra de ferramentas com ícone + rótulo; campo de busca em cápsula (M)
- [x] 2.5 Formulário com rótulos à direita (`form_row`), caixa com abas (`tab_box`), slider com marcas e polegar em gota (M)
- [x] 2.6 Tabela com cabeçalho em gel, colunas ordenáveis e triângulos de expansão (`table`); Monitor ordena e mostra em hierarquia (M)
- [x] 2.7 Triângulo de abrir/fechar seção (`disclosure`) e janela que muda de altura pelo app (`set_window_size`, animada); Atalhos em grupos (P)
- [x] 2.8 Redesenho parcial no kit: só a área que mudou vai para a tela (hoje cada quadro refaz a janela inteira; o pulso do alerta custa ~7% de um núcleo, e digitar/rolar também ficaria mais leve) (M)

## Fase 3 — Menus e sessão (comportamento do sistema)

- [x] 3.1 Submenus nos menus (▸), com navegação por teclado (M)
- [x] 3.2 Menu principal da era: Sobre, Preferências, Dock ▸, Itens Recentes ▸, Forçar Encerrar, Repouso, Reiniciar, Desligar, Encerrar Sessão (M)
- [x] 3.3 Desligar/Reiniciar/Encerrar com alerta e contagem de 120 s, apps avisados antes de sair, tela de desligando (M)
- [x] 3.4 Menu do app completo (Sobre, Preferências, Ocultar Outros, Mostrar Todos, Encerrar) e menu Janela do compositor (M)
- [x] 3.5 Forçar Encerrar desenhado pelo compositor, com detecção de "não responde" (ping) e cursor de espera original (M)
- [x] 3.6 Extras na barra de menu: volume, bateria e monitor, com menus próprios; teclas de volume e brilho (M)
- [x] 3.7 Ociosidade: apagar a tela (DPMS), bloqueio de tela no compositor (senha via PAM) e tampa do notebook (G)
- [x] 3.8 Painel "Iniciando sessão…" com barra de progresso ao entrar (P)
- [x] 3.9 **Busca Rápida** (como o Spotlight do 10.4): lupa no canto direito da barra de menu e
  Super+Espaço; acha apps instalados (`.desktop`), Preferências e arquivos (índice leve em segundo
  plano), Enter abre o primeiro resultado. Resolve o "abrir apps" apontado no uso real (M)
- [x] 3.10 **Exposé** (10.3): F9/F10/F11 ou canto da tela espalha todas as janelas, só as do app,
  ou mostra a mesa; clique traz a janela à frente. Feito no compositor (M)

## Fase 4 — Dock completo

- [x] 4.1 Super+Tab no estilo do Tiger: faixa de ícones grandes no meio da tela, com o nome do app (M)
- [x] 4.2 Ícone pula ao abrir um app (M)
- [x] 4.3 Animação de minimizar até o Dock (Escala) e miniatura viva da janela minimizada (G)
- [x] 4.4 Menu de contexto do Dock com bico apontando para o ícone (Manter, Mostrar no Arquivos, Encerrar) (M)
- [x] 4.5 Ocultar automaticamente, posição esquerda/direita, ampliação até 112 (M)

## Fase 5 — Tela de login e inicialização

Arquitetura escolhida (ver [4-sessao-login.md](analise/4-sessao-login.md)): **greetd** cuida de PAM e logind; o
Glove roda em modo quiosque (`glove --greeter`) e o app `glove-login` (glove-kit) conversa com o greetd.
Meta: tela de login inteira abaixo de 25 MB.

- [x] 5.1 Modo quiosque do compositor (`--greeter`: sem Dock nem menus) (M)
- [x] 5.2 Cliente do protocolo do greetd e `glove-login`: lista de usuários com fotos, senha, painel que balança no erro, Reiniciar/Desligar (G)
- [x] 5.3 Avatares originais desenhados por código; `data/greetd/` + `install-login.sh` (com volta ao gerenciador anterior) (M)
- [x] 5.4 Splash de boot e desligamento: tema Plymouth com arte gerada por código (`glove-arte`) (M)
- [x] 5.5 Assistente de primeiro uso (teclado, data e hora, tema, foto) (M)

## Fase 6 — Arquivos

- [x] 6.1 Janela do Tiger: barra de ferramentas unida ao título em metal, Voltar/Avançar, segmentado de visualização, engrenagem, busca em cápsula e **barra lateral** (discos, Mesa, Início, Aplicativos, Documentos…) (G)
- [x] 6.2 Lixeira: Esvaziar com alerta, Devolver, ícone cheia/vazia no Dock (M)
- [x] 6.3 Ícones de pastas especiais (Mesa, Documentos, Música, Imagens, Vídeos…), de tipos e emblema de atalho (M)
- [x] 6.4 Copiar/colar/mover/arrastar com janela de progresso e alerta de substituição (G)
- [x] 6.5 Renomear no lugar, menu de contexto, Duplicar, Criar atalho (M)
- [x] 6.6 Obter Informações (Ctrl+I) com seções Geral, Nome, Abrir com, Permissões, Comentários (G)
- [x] 6.7 Menu Ir completo, "Ir para a pasta…" com Tab, pasta Aplicativos/Utilitários (M)
  - Apontamento (sessão no hardware): falta um jeito de achar e abrir os apps instalados no Linux.
    No 10.0–10.3 não havia Launchpad (10.7) nem Spotlight (10.4): os apps ficavam na pasta
    **Aplicativos** (Ir ▸ Aplicativos, Ctrl+Shift+A), no Dock e em Itens Recentes. A pasta
    Aplicativos do Glove deve listar os `.desktop` instalados (XDG), com os ícones do sistema,
    e permitir arrastar para o Dock (ver 4.4 "Manter no Dock"). A Busca Rápida (3.9) usa a mesma lista
- [x] 6.8 Lista ordenável com expansão e datas "Hoje/Ontem"; status com seleção e "somente leitura" (M)
- [x] 6.9 Diálogo Abrir/Salvar em colunas (`kit::filebrowser`), usado no Editor, Visualizador e Lembretes (G)
- [x] 6.11 Atualização automática (inotify), seleção múltipla e retângulo elástico; pendrives na mesa (M)
- [x] 6.12 Pastas inteligentes (10.4): buscas salvas da Busca Rápida que aparecem como pastas (M)
- [x] 6.13 Compactar: "Compactar item" no menu do Arquivos e clique duplo em .zip/.tar.gz abre
  (utilitário de arquivo com janela de progresso, como o do 10.3) (M)

## Fase 7 — Preferências

- [x] 7.1 Janela "Mostrar Todos": grade em 4 categorias, barra com favoritos, altura muda por painel (M)
- [x] 7.2 Refazer no layout da era: Aparência (destaque, duplo clique minimiza), Mesa, Dock, Teclado (+ aba Atalhos), Mouse, Trackpad, Monitores (M)
- [x] 7.3 Som: volume, mudo, entrada/saída (PipeWire/PulseAudio) (M)
- [x] 7.4 Economizador de Energia: repouso, tela, ações (logind, UPower) (M)
- [x] 7.5 Rede e Wi-Fi (NetworkManager) (G)
- [x] 7.6 Data e Hora: fuso, NTP (timedated), aba Relógio da barra (M)
- [x] 7.7 Itens de Início (autostart XDG) e Menu de Entrada (vários layouts) (M)
- [x] 7.8 Acesso Universal: cursor grande, zoom, cores invertidas (no compositor) (M)
- [x] 7.9 Protetor de Tela com protetores originais (G)
- [x] 7.10 Contas/Minha Conta (AccountsService), Internacional (idioma e formatos) (G)
- [x] 7.11 Compartilhamento, Mídia Removível, Aplicativos Padrão, Bluetooth (M cada)
- [x] 7.12 Baixa: Atualização de Software, Disco de Inicialização, Fala, Impressão
- Pular: ColorSync, QuickTime, Classic, .Mac/iDisk, AppleTalk, reconhecimento de fala

## Fase 8 — Apps

- [x] 8.1 Navegador que caiba em 500 MB (NetSurf abaixo de 1 GB de RAM, Firefox acima) (P)
- [x] 8.2 Lembretes com moldura própria (faixa colorida) e um processo só para todas as notas (M)
- [x] 8.3 Editor: régua, alinhamento, Localizar/Substituir (M)
- [x] 8.4 Visualizador: barra de ferramentas, miniaturas, PDF (M)
- [x] 8.5 Monitor: ordenar, filtrar, abas CPU/Memória/Disco/Rede (M)
- [x] 8.6 Busca: colunas Data/Tamanho/Tipo, painel de caminho; Discos em dois painéis (M)
- [x] 8.7 Música (Rust puro, meta ~10 MB) (G)
- [x] 8.8 Contatos (vCard) e Agenda (.ics) (G)
- [x] 8.9 Painel de Fontes e Painel de Cores no kit (M)
- [x] 8.10 Calculadora científica, Caracteres em modo Teclado, Captura de seleção (M)
- [x] 8.11 Vídeo (mpv com controles próprios), Registros, Informações do Sistema, Ajuda (M cada)
- [x] 8.12 Correio (por último) (G)
- [x] 8.13 Dicionário (10.4) com dicionários livres (formato DICT/StarDict) (M)
- [x] 8.14 Livro de Fontes (10.3): ver, instalar e desativar fontes (M)

## Fase 9 — Sessão avançada

- [x] 9.1 Vários monitores ao mesmo tempo (estender/espelhar) (G)
- [x] 9.2 Apps X11 (XWayland) (M)
- [x] 9.3 Troca rápida de usuário (Baixa)

## Fase 9.9 — Correções do uso real (05/11/2026, antes da Fase 10)

Defeitos vistos usando a sessão no dia a dia (17 capturas de tela), todos corrigidos:

- [x] 9.9.1 Janelas encolhiam até os controles se sobreporem (Monitor, Discos, Música, Correio, Agenda):
  `WindowOptions::min_size`; sem ele, o mínimo é 3/4 do tamanho inicial (era 1/2)
- [x] 9.9.2 Visualizador: miniaturas roladas passavam por cima da barra de ferramentas (recorte na lateral)
- [x] 9.9.3 Monitores: "Ajustar o monitor" escolhe qualquer monitor ligado; resolução, frequência e
  escala de cada um (`[monitor.others]` na configuração); o compositor usa o ajuste de cada tela
- [x] 9.9.4 Cliques atravessavam menus: o menu da engrenagem do Arquivos media 120 px (tem ~177) e
  "Mostrar arquivos ocultos" caía na coluna de trás; `ui.reserve`/`ui.unreserved` no kit reservam a
  área do menu, e clique fora só fecha. `set_active(true)` apagava o bloqueio do pop-up aberto
  (Captura: "Uma área" caía no pop-up "Quando"); soltar na margem do pop-up agora fecha
- [x] 9.9.5 Som: alertas curtos se perdiam com a saída em repouso (folga de silêncio no WAV); clicar no
  alerta escolhido toca de novo; sliders gravam a configuração no máximo a cada 150 ms (antes, a cada
  quadro: todos os apps recarregavam), `wpctl` numa fila só com o último volume, `wpctl status` em thread
- [x] 9.9.6 "Abrir com": os apps do Glove que abrem arquivos se registram em `~/.local/share/applications`
  (`glove-*.desktop`, fora dos menus) e viram padrão dos tipos sem padrão; clique duplo segue o padrão
  do tipo do arquivo
- [x] 9.9.7 Coluna de detalhes do Arquivos mostra a foto (prévia numa thread); Visualizador e prévia
  leem PNG, JPEG e **WebP**, pelo conteúdo (fotos da web costumam ser WebP com nome .jpg)
- [x] 9.9.8 Caracteres: visão **Fonte** (lista das fontes instaladas, os caracteres que a fonte tem, prévia
  grande com linha de base, altura do x, maiúsculas, subida e descida) e **Instalar Fonte…** (Ctrl+Shift+I)
- [x] 9.9.9 Menu de monitores da barra: a resolução não mudava (o compositor mudava a memória antes de
  gravar e a recarga não via diferença); lista também os outros monitores
- [x] 9.9.10 Apps demorando ~25 s para abrir: o portal GTK saía com "cannot open display"; a sessão passa
  `WAYLAND_DISPLAY`/`DISPLAY` ao systemd e ao D-Bus e grava `glove-portals.conf`; GTK/Qt/SDL caem no
  Xwayland quando não têm Wayland
- [x] 9.9.11 Lembretes: o botão de abrir a nota recolhida ficava sob o puxador e sumia com a nota inativa;
  × (fechar) e –/+ (recolher/abrir) sempre à vista; reabrir travava o app (canto arredondado com
  tamanho negativo na animação: `rrect4` ignora áreas vazias)
- [x] 9.9.12 Tela de login grande demais: monitores que gravam a proporção no lugar do tamanho
  (160 × 90 mm) davam escala 2,25 (`auto_scale` desconfia de telas minúsculas e densas); o instalador
  copia a configuração do usuário (escala, monitor, teclado, tema) para `/etc/glove/login`;
  escolha do **ambiente gráfico** (wayland-sessions e xsessions; X11 pelo `startx`), lembrando a última
- [x] 9.9.13 Arquivos mais perto do Finder do Tiger: lateral branca com linhas de 30 e ícones de 24 por
  lugar, lateral e conteúdo como painéis encaixados no metal, visão de ícones em 64; pasta redesenhada
  (fundo com aba e frente clara listrada); Dock em vidro cinza-lilás com os ícones passando da borda
- [x] 9.9.14 Vídeo: o app existe (8.11) e usa o mpv para a imagem; o instalador oferece o mpv e os
  outros programas que faltarem
- [x] 9.9.15 `./install.sh`: instalador que pergunta cada passo (compilar, programas que faltam, sessão,
  tela de login com greetd e troca do gerenciador, tema do Plymouth); `--remove` desfaz
- [x] 9.9.16 Dock refeito para o do Tiger: vidro cinza-lilás quase opaco (o tom não depende do papel de
  parede), ícones crescidos até encher a vaga e quase encostados (2 px), dentro da prateleira
- [x] 9.9.17 Som: em português o `pw-play` lia "0.50" como 0 (vírgula decimal) e só o volume máximo
  tocava; roda com `LC_NUMERIC=C`
- [x] 9.9.18 Prévia de imagens enche a coluna de detalhes sem distorcer, com −, +, Ajustar, rodinha e
  arrastar a foto ampliada; mostra as dimensões
- [x] 9.9.19 Menus de contexto: nomes compridos encurtados com "…"; com vários escolhidos, o botão
  direito mantém a seleção e o menu vale para todos ("Mover 3 itens para a Lixeira", "Abrir 3 itens")
- [x] 9.9.20 Arquivos mais rápido: metal escovado guardado depois da primeira vez (era calculado pixel a
  pixel a cada quadro, inclusive no fundo todo), pasta Aplicativos guardada por 5 s (relia todos os
  .desktop várias vezes por quadro), abrir arquivo fora da linha principal
- [x] 9.9.21 Arrastar de verdade (protocolo do Wayland): o ícone segue o mouse (o compositor desenha o
  ícone do arrasto), a pasta de destino acende, solta em outra janela do Arquivos ou em outro app;
  mesmo disco move, outro disco copia; apertar num dos vários escolhidos arrasta todos
- [x] 9.9.22 Copiar copia o arquivo (não o caminho): `text/uri-list`, `x-special/gnome-copied-files`
  e, para uma imagem, a imagem em PNG; colar aceita esses formatos de outros apps
  (`ui.copy_files`, `ui.request_paste_files`, `ui.start_drag_files`, `ui.take_drop` no kit)
- [x] 9.9.24 Dock: ícones dentro da prateleira com folga igual em cima e embaixo, sombra com a forma de
  cada ícone (guardada por tamanho), borda clara fina e suave
- [x] 9.9.25 Visualizador: barra de rolagem na lateral de miniaturas
- [x] 9.9.26 "Salvar como" do navegador com a janela do Glove: portal FileChooser próprio
  (`glove-portal`, D-Bus, sob demanda) que abre `glove-escolher` (Abrir, Salvar e Escolher pasta, a
  folha do kit); a sessão registra o portal sem instalar nada no sistema (XDG_DESKTOP_PORTAL_DIR) e
  exporta GTK_USE_PORTAL=1 para os apps GTK3 (Firefox)
- [x] 9.9.27 Terminal pronto para o Neovim e CLIs modernas: fundo das células alinhado aos pixels
  (sem as listras entre colunas), fundos antes das letras, caracteres de bloco e de linhas
  (█ ▀ ░ ─ │ ┌ ╭ ═ ╬…) desenhados como retângulos exatos
- [x] 9.9.28 Janela "Salvar" do navegador ocupa a janela toda (sem as bordas cinza da folha); aviso do
  Vídeo como janela de alerta (sem folha numa janela pequena)
- [x] 9.9.29 Visualizador: botão direito com Copiar Imagem (o arquivo/PNG), Mover para a Lixeira (e
  mostra a próxima), Girar, Mostrar no Arquivos; Ctrl+C e Delete
- [x] 9.9.30 Wi-Fi: o Glove não inicia mais o ícone de rede do GNOME (nm-applet, que pedia a senha e o
  chaveiro nas janelas dele); rede salva que pede senha abre a folha de senha das Preferências
- [x] 9.9.31 Botões em vidro polido (Aqua): reflexo forte com borda definida, cor funda sob o reflexo,
  brilho que atravessa o vidro embaixo, borda fina e sombrinha; azul-céu como o OK do Tiger
- [x] 9.9.32 Captura de uma janela (a janela sob o mouse fica azulada; o clique a traz para a frente e
  fotografa com a barra de título; Ctrl+Shift+J); captura de área em qualquer monitor (a película e o
  retângulo aparecem em todos; a foto de outro monitor sai na escala dele)
- [x] 9.9.33 Arrastar janela em zoom: sai do zoom e volta ao tamanho de antes, com o ponto agarrado sob o
  mouse, sem soltar o arrasto; entrando num monitor em que não cabe, encolhe para 80% dele e continua
- [x] 9.9.34 Minimizar com o efeito Lâmpada Mágica (a janela em 64 faixas: os lados entortam num funil até
  o ícone e ela escorre para dentro do Dock); escolha em Preferências ▸ Dock
- [x] 9.9.35 Temas do Terminal (Básico, Pro, Homebrew, Oceano, Grama, Novela, Areia Vermelha, Manual,
  Noite), escolhidos por amostras nas Preferências e trocados na hora; fonte numa lista das de largura fixa
- [x] 9.9.36 Temas de ícones do sistema (freedesktop, PNG e SVG com herança): os apps de fora mostram o
  ícone de verdade (no Dock, Super+Tab, Forçar Encerrar, Aplicativos, Busca); em Preferências ▸ Aparência
  dá para trocar os ícones do Glove por um tema instalado (com prévia)
- [x] 9.9.37 Tudo do config.toml ajustável nas Preferências: Terminal do atalho, navegador do app
  Navegador, onde a Lixeira abre e os apps X11 (em Aplicativos Padrão)
- [x] 9.9.38 Metal escovado do Tiger: claro e suave (de ~230 a ~200), riscos longos e fracos, um brilho
  sutil no meio da janela; título e barra de ferramentas numa peça só (sem a linha entre eles); cantos
  de baixo arredondados no Arquivos (com a sombra acompanhando)
- [x] 9.9.39 Puxador de redimensionar como o do Tiger: quatro ranhuras diagonais nítidas, sem caixa
- [x] 9.9.40 Lâmpada Mágica sem serrilhado: a foto da janela é deformada pela placa de vídeo (shader que
  acha, para cada pixel, de onde ele vem na janela), com curvas contínuas e bordas suaves
- [x] 9.9.23 Sombra das janelas cobre também os cantos de cima arredondados (antes o pedaço recortado
  ficava sem sombra e o canto parecia vivo sobre outra janela)
- [x] 9.9.41 Janela maximizada encosta exatamente no Dock, em qualquer tamanho: a área útil usa a
  espessura real da prateleira na tela (inclusive quando o Dock encolhe para caber) e as janelas em
  zoom se reajustam quando o Dock muda
- [x] 9.9.42 Arrastar arquivos ou pastas para o Terminal escreve o caminho completo (entre aspas quando
  tem espaço), como no Terminal do Mac; o arrastar vira "copiar", nunca "mover"
- [x] 9.9.43 Abrir mais de uma instância do mesmo app pelo Dock: Ctrl+clique (ou Super+clique) no ícone,
  ou "Abrir Outra Instância" no menu do ícone (o clique simples continua trazendo o app para a frente)

## Fase 9.10 — O que faltou do Tiger (levantamento de 05/11/2026)

Segunda passada nas 91 capturas de `referencias/tiger/` contra o Glove atual, depois do uso real
mostrar que a mesa não era o Finder. Tudo aqui é do 10.4 e vem antes da Fase 10.

- [x] 9.10.1 **Mesa como o Arquivos** (`Desktop`): os itens da pasta Mesa (`XDG_DESKTOP_DIR`, com
  nome e ícone dos atalhos `.desktop`) além dos discos, numa grade a partir do canto superior direito,
  nomes em até duas linhas; menu do fundo (Nova Pasta, Obter Informações, Colar, Alterar Fundo da
  Mesa…, Limpar, Organizar por ▸, Mostrar Opções de Visualização); menu do ícone (Abrir, Mover para a
  Lixeira, Obter Informações, Renomear, Compactar, Duplicar, Criar Atalho, Copiar); clique duplo abre;
  retângulo e Shift/Ctrl escolhem vários; arrastar pela mesa (posição guardada em
  `~/.config/glove/mesa-posicoes`), para pastas e discos da mesa, para o Lixo e os apps do Dock e para
  janelas (arrastar do sistema), e das janelas do Arquivos para a mesa (cai onde foi solto); Alt copia;
  renomear no lugar (Enter; a pasta nova já nasce renomeando); atalhos do Arquivos (Ctrl+N,
  Ctrl+Shift+N, Ctrl+O, Ctrl+I, Ctrl+D, Ctrl+L, Ctrl+J, Ctrl+A, Ctrl+C/V, Ctrl+Backspace, setas,
  digitar o nome); com a mesa em foco a barra mostra Arquivos, Arquivo, Editar, Visualizar e Ir.
  Opções de Visualização (Ctrl+J): tamanho dos ícones e do texto, nome embaixo ou à direita, manter
  organizado por, discos e pendrives na mesa (`[desk]` no config.toml). O glove-files ganhou
  `--abrir`, `--transferir`, `--duplicar`, `--lixo` e `--opcoes-mesa`. "Abrir com ▸" e Rótulo ficam
  com a 9.10.3 (G)
- [x] 9.10.2 **Painel de Widgets** (`Dashboard`, F12 ou canto ativo "Painel de Widgets"): a tela escurece
  e os widgets aparecem (com transição); desenhados pelo compositor, com arte própria
  (`paint/widgets.rs`): Relógio (cidade do fuso), Calendário (mês com setas), Calculadora (mouse e
  teclado, memória), Clima (Open-Meteo pelo `curl` numa thread: agora, máx/mín e 6 dias; clique na
  cidade para trocar), Lembrete (texto guardado), Monitor (processador e memória) e Dicionário (Enter
  abre o Dicionário). O "+" no canto abre a barra de widgets (clique põe o widget no espaço livre mais
  perto do meio) e mostra o × de fechar em cada um; arrastar move; clique fora, Esc ou F12 fecham.
  Widgets e posições em `~/.config/glove/painel.toml`; atalho editável em Preferências ▸ Teclado e
  opção nos cantos ativos. Conversor de unidades fica para depois (G)
- [x] 9.10.3 Arquivos: **Preferências do Arquivos** (Arquivos ▸ Preferências…, Ctrl+,): Geral (discos e
  pendrives na mesa, pasta das janelas novas), Rótulos (os sete nomes), Barra Lateral (o que aparece)
  e Avançado (extensões, aviso ao esvaziar o Lixo, que vale também para o Dock). **Rótulos de cor**
  gravados no próprio arquivo (atributo `user.glove.rotulo`): pílula do nome na vista de ícones e na
  mesa, linha colorida na lista e nas colunas, submenu "Rótulo ▸" com as bolinhas das cores no menu de
  contexto do Arquivos, no da mesa e em Arquivo ▸ Rótulo. **Abrir com ▸** (o padrão primeiro) nos menus
  de contexto do Arquivos e da mesa, pelos `.desktop` do sistema (`glove_config::arquivos`, que também
  acha o tipo pela extensão sem abrir o arquivo). **Opções de Visualização** (Ctrl+J): tamanho dos
  ícones e do texto, nome embaixo ou à direita, organizar por e fundo da vista de ícones. Tudo em
  `[files]` no config.toml, valendo na hora nas janelas abertas e na mesa (M)
- [x] 9.10.4 Arquivos: **Ejetar** (`glove_config::discos`: desmonta as partições do disco e desliga o
  aparelho pelo udisksctl, sem root; "disco em uso" vira um alerta claro): botão ⏏ ao lado de cada
  pendrive na barra lateral, Arquivo ▸ Ejetar (Ctrl+E) no Arquivos e na mesa, "Ejetar “disco”" no menu
  do ícone; arrastar o pendrive para o Lixo ejeta, e durante o arraste o Lixo do Dock vira o ⏏. Aviso
  "disco não foi ejetado corretamente" (`Finder-Eject-Warning`) só quando o aparelho some sem ter sido
  ejetado pelo Glove ou pelo Discos. `GLOVE_MOUNTS_FILE` simula montagens nos testes (M)
- [x] 9.10.5 Arquivos: **Conectar ao Servidor** (Ir ▸ Conectar ao Servidor…, Ctrl+K; `Finder-Connect-to-Server`):
  endereço (sem esquema vale smb://), "+" para os favoritos, os últimos usados, Remover, Navegar e
  Conectar; monta pelo gvfs (`gio mount`, sem root) para smb, sftp, ftp, webdav…, com a folha de login
  do Glove (usuário registrado ou convidado, senha, domínio no smb; senha recusada avisa e pede de
  novo). O servidor conectado abre numa janela e aparece na barra lateral e na mesa com o ⏏ para
  desconectar (opção "Servidores conectados" nas Preferências do Arquivos). Item **Rede** na barra
  lateral (Ctrl+Shift+K) com as máquinas que o gvfs vê na rede (`network:///`, pelo Avahi/Samba);
  clicar numa conecta. Testado com um servidor FTP local que pede senha (M)
- [x] 9.10.6 Arquivos: **pastas com mola**: segurar o arrastado (de qualquer app ou da mesa) sobre uma
  pasta por 0,9 s abre a pasta na janela, e dá para continuar descendo até soltar; na mesa, segurar
  sobre uma pasta abre uma janela dela (P)
- [x] 9.10.7 Janelas: **botão-pílula** no canto do título nos apps com barra de ferramentas (o app põe o
  item `toggle-toolbar` nos menus; no Arquivos esconde a barra e a lateral, também em Visualizar ▸
  Ocultar Barra de Ferramentas, Ctrl+Alt+T); **ponto no botão fechar** com alterações não salvas
  (`ui.set_edited`, no Editor); **ícone do documento** no título (`ui.set_document`, no Editor, no
  Visualizador e na pasta do Arquivos): arrastar leva o arquivo, Ctrl+clique (ou botão direito) mostra
  as pastas do caminho e abre a escolhida; **Ctrl+`** alterna as janelas do mesmo app (atalho editável
  nas Preferências) (M)
- [x] 9.10.8 **Imprimir** (Arquivo ▸ Imprimir…, Ctrl+P) como folha do kit (`kit::print`, `Print`): impressora
  (as do CUPS, a padrão primeiro; sem nenhuma, "Adicionar…" abre Preferências ▸ Impressão), cópias,
  páginas (todas ou de/até), Salvar PDF… (folha de salvar do kit) e Pré-visualizar (abre o PDF no
  Visualizador); imprime pelo `lp`. Texto vira PDF A4 no próprio kit (Helvetica com acentos, quebra nas
  palavras, rodapé com nome e página); imagens vão direto ao CUPS e viram PDF para salvar. No Editor,
  no Visualizador (imagem ou PDF) e no Correio (a mensagem aberta, com De/Para/Data/Assunto). O
  Navegador (Firefox/NetSurf) já tem o imprimir dele (M)
- [x] 9.10.9 **Acesso às Chaves** (`glove-chaves`, `Keychain-Access`, em Utilitários): chaveiros à esquerda
  (o "login" primeiro), as senhas guardadas numa tabela com busca (nome, tipo — senha de rede, de
  Wi-Fi, de app — e data), detalhes com conta e onde, Mostrar senha, Copiar senha, Apagar (com
  alerta), Trancar/Destrancar e Mudar a Senha do Chaveiro (folha com senha atual, nova e confirmação).
  Fala com o Secret Service pelo D-Bus (zbus; sessão local "plain"); `GLOVE_CHAVES_TESTE=1` mostra
  itens de mentira e `--contar` só conta chaveiros e itens (diagnóstico sem dados pessoais) (M)
- [x] 9.10.10 **Utilitário de Rede** (`glove-utilrede`, `Network-Utility`, em Utilitários): abas Informações
  (cada interface, as de verdade primeiro: tipo, estado, endereço físico, IPs, velocidade, pacotes e
  erros), Ping, Lookup (host/dig, com o tipo de registro), Traceroute (traceroute ou tracepath),
  Whois (protocolo falado direto: IANA e o servidor indicado), Finger (porta 79) e Portas (varredura TCP
  de/até, 64 de cada vez, com o nome do serviço); saída em fonte fixa que acompanha o fim, botão Parar.
  O kit agora liga o relógio do `App::tick` também quando o app só passa a pedir depois de aberto (M)
- [x] 9.10.11 Preferências que faltavam: **Segurança** (senha ao sair do repouso e do protetor; encerrar a
  sessão depois de N minutos parado, com o aviso de contagem do glove-sair; firewall pelo ufw, com o
  estado lido de /etc/ufw/ufw.conf e Ligar/Desligar pelo pkexec), **Busca Rápida** (categorias
  Aplicativos, Preferências e Arquivos; Privacidade: pastas que a busca não olha), **CDs e DVDs** no painel
  Mídia Removível (CD/DVD de dados: abrir no Arquivos ou nada; DVD de vídeo: abrir no Vídeo, no Arquivos
  ou nada) e **Exposé e Painel** num painel próprio (os quatro cantos ativos e as teclas F9–F12; a Mesa
  ganhou o botão "Cantos Ativos…" que leva até lá). Configuração em `[security]`, `[search]` e `[media]` (M)
- Pular: Classic, .Mac/iDisk, AirPort, iSync, Internet Connect (coberto por Rede), Automator e
  Editor de Scripts (não há AppleScript no Linux); iChat fica na 10.9 (Mensagens)

## Fase 9.11 — Ícones no estilo do Tiger (comparação de 05/11/2026)

Comparando lado a lado os ícones do Glove (`cargo run -p glove-paint --example icones -- folha.png`) com
os do 10.4, todos ficaram fora da linguagem do Tiger: estão num estilo vetorial plano, de frente, com
degradês lisos, luz fraca, a elipse cinza de sombra embaixo e pouco detalhe. Os do Tiger são objetos
"de verdade": em perspectiva (vistos um pouco de cima e de lado), com materiais (metal escovado com
reflexo forte, vidro, couro, madeira, papel), luz de cima com brilhos marcados, sombra no formato do
objeto e muitos detalhes pequenos, ocupando o quadro todo.

Ordem: primeiro desenhar todos os ícones no código; só depois de aprovados vira o tema (9.11.6).

Regra: **arte própria**, com os ícones da Apple só como referência de estilo (luz, perspectiva,
materiais, paleta). Nenhum arquivo deles entra no projeto, e nenhum desenho decalca ou copia a
composição de um ícone da Apple nem usa marcas (rosto do Finder, bússola do Safari, a maçã).

- [x] 9.11.1 Base no glove-paint (`paint/src/tiger.rs`): medidas relativas ao quadrado do ícone, objetos
  inclinados (transformação dos caminhos), sombra macia no formato do objeto no lugar da elipse,
  materiais reutilizáveis (riscos de metal escovado, grão de couro, brilho de vidro, metal claro) feitos
  com um caminho só por cor (rápido: ~1,5 ms por ícone em 128 px) (G)
- [x] 9.11.2 Primeiro lote: Arquivos/pastas (azul translúcido com folhas dentro e espessura; as pastas
  especiais herdam), Terminal (monitor de alumínio escovado), Calculadora (alumínio com visor verde e teclas
  de gel), Relógio (de mesa, cromado), Correio (envelope inclinado com selo picotado e carimbo), Agenda
  (bloco de mesa com argolas e a ponta virada), Contatos (caderno de couro verde com relevo dourado,
  elástico e fita) e Lixo vazio/cheio (aço escovado furadinho, com papéis amassados). Aguardando a
  aprovação da direção para seguir (G)
- [x] 9.11.3 Os outros apps do Dock e de Aplicativos/Utilitários (Navegador, Editor, Lembretes, Busca,
  Caracteres, Captura, Monitor, Xadrez, Música, Vídeo, Registros, Informações, Ajuda, Dicionário, Livro de
  Fontes, Acesso às Chaves, Utilitário de Rede, Discos, Preferências) (G) — v0.9.13: redesenhados; Música e
  Chaves ficaram como estavam (aprovados); o Lixo cheio ganhou papéis amassados em facetas
- [x] 9.11.4 Documentos e tipos de arquivo, pastas especiais, discos, pendrives e servidores (M) — v0.9.14:
  folha com a dobra, foto, áudio, vídeo, zíper, PDF, código; HD de 3,5", pendrive, CD/DVD, servidor e Ejetar
  (`icons::mount_icon` escolhe pelo aparelho). Pedidos do teste: Terminal e Monitor viraram monitores de tubo
  com vidro brilhante, lupa com vidro de verdade, engrenagens de aço polido, pasta com margem; o app
  Arquivos ganhou ícone próprio (pasta de gel azul com um rosto sorrindo de frente)
- [x] 9.11.5 Painéis das Preferências e os ícones pequenos (barra lateral, menus, mesa) (M) — v0.9.15:
  todos os painéis redesenhados (`tiger_prefs.rs`), mouse, trackpad, chaves, clipe, janela e genérico;
  ícones próprios para Utilitário de Rede, Visualizador, Utilitário de Disco e Utilitário de Arquivo;
  Lixo de tela de arame que enche aos poucos (um papel por item, cheio com 20); relevo das pastas
  especiais reforçado nos tamanhos pequenos; Dock com mais espaço entre os ícones e os sliders de
  tamanho mostrando os pixels
- [x] 9.11.6 Depois de todos aprovados: **tema de ícones "Glove"** gerado a partir do código pelo
  `glove-arte` (PNGs de 16 a 256 px, 1× e 2×, nomes no padrão freedesktop), instalado em
  `~/.local/share/icons/Glove` e usado pelo Glove (desenhando na hora só o que muda, como o relógio do
  Dock) e pelos apps GTK/Qt; publicado num **repositório próprio** no GitHub (M) — v0.9.15:
  `glove-arte icones <pasta>` gera 84 ícones em 16 tamanhos (1.344 PNGs) com os nomes comuns do
  freedesktop/GNOME como atalhos; o tema é publicado em github.com/fabioaacarneiro/glove-icons

## Fase 9.12 — Ícones mais realistas (pedido de 07/11/2026)

Menos "desenho animado": nada de contorno preto grosso; luz de cima, chanfros, rebaixos e textura de
material. Os ícones-símbolo do Mac servem de homenagem (lembrar, sem copiar forma, marca ou composição).

- [x] 9.12.1 Disco, Terminal, Monitor de Atividade, Navegador e Arquivos (`paint/src/tiger_real.rs`) (M)
  — v0.9.16: HD em perspectiva de verdade (tampa de alumínio com os rebaixos do prato e da etiqueta,
  parafusos Torx, frente com as fendas e a luz acesa); monitor de tubo bege de frente (moldura funda,
  queixo com plaqueta, tecla de ligar e alto-falante, sobre o pé) para o Terminal e o Monitor;
  bússola de bolso cromada com coroa, mostrador azul com meridianos e agulha vermelha e prata;
  rosto em dois tons do Arquivos (perfil com nariz olhando para a direita, olhos redondos, sem traço
  preto). Correção: a janela que o app pede para abrir maximizada (porque fechou assim) agora ocupa a
  área útil certinha, e "restaurar" a devolve a 2/3 da tela, centralizada
- [x] 9.12.2 Segundo lote, pelas referências do Tiger e do Mac (M) — v0.9.17: Preferências (placa de
  alumínio escovado com o poço e a engrenagem de cinco raios); Utilitário de Disco (o HD com um
  estetoscópio; os discos nas outras telas continuam só o HD); Lixo de tela preta em losangos com aro e
  cinta cromados (o enchimento de papéis continua igual); Música (CD com a nota de vidro verde);
  Editor (carta escrita com caneta-tinteiro); Xadrez (cavalo de madeira dourada com volume, colarinho e base bojuda, na casa da ponta de um tabuleiro 2×2 cinza e branco); Fontes ("F"
  serifado de metal com tipos móveis); Visualizador (fotos com uma lupa de vidro que aumenta o pedaço de baixo); Captura (câmera
  sobre fotos impressas); Dicionário em couro vinho com "Aa"
- [x] 9.12.3 Os que ainda não seguiam as referências (M) — v0.9.18: Xadrez com o cavalo como peça
  torneada (pé, bojo e anel com as elipses na inclinação do tabuleiro; pescoço entrando no anel) e a
  cabeça redesenhada do zero (duas orelhas, focinho com a boca, bochecha, garganta recortada e crina);
  Contatos em couro marrom com costura e "@" dourado em relevo; Correio como selo picotado com
  andorinha, valor e carimbo; na barra lateral do Arquivos, Início vira uma casinha e Aplicativos um
  compasso de desenho em forma de "A" e Rede o globo de rede (os outros lugares já tinham ícone próprio)
- [x] 9.12.4 Fecho dos ícones e tema mais leve (M) — v0.9.19: o Xadrez passa a ser uma torre torneada
  (pé, bojo, coluna, anel e coroa com ameias) no tabuleiro 2×2; claquete do Vídeo com volume (lousa com
  reflexo, ripas chanfradas, giz e a sombra da ripa aberta). Tema: PNG sem perda com compressão máxima e
  as versões 2× que têm os mesmos pixels de um tamanho 1× viram atalhos para ele (840 imagens em vez de
  1.344; 7,5 MB → 4,0 MB)

## Fase 9.13 — Mais leve (pedido de 07/11/2026)

Estudo completo em `docs/MEMORIA.md`.

- [x] 9.13.1 Medição e as mudanças que valem (M) — v0.9.20: comandos `mem`/`trim` no modo de teste e a
  opção `memstats` (conta as alocações do Rust); um executável só para os 32 apps (`glove-apps`, os
  outros viram atalhos: o código fica uma vez na memória e no disco, 166 MB → 44 MB); fontes mapeadas
  do arquivo (divididas entre todos os processos). Três apps abertos: 35,0 MB → 26,0 MB. O resto da
  memória do compositor é da Mesa (custo fixo do OpenGL); parado, o Glove usa 0,1% de um núcleo

## Fase 9.14 — Instalação e pacotes (o `instalador.sh` virou `glove-setup-tool.sh` na 9.15.7) (pedido de 07/11/2026)

- [x] 9.14.1 `instalador.sh` (M) — v0.9.21: menu ou comandos para instalar em qualquer Linux (instala o
  que falta para compilar pelo apt, dnf, pacman, zypper, xbps ou apk; compila; põe os programas, a
  sessão, o PAM do desbloqueio, o tema de ícones e a documentação no prefixo, com a lista do que foi
  instalado), desinstalar (pela lista, ou pelo gerenciador quando veio de pacote) e gerar os pacotes
  .deb (Debian/Ubuntu), .tgz (Slackware, com slack-desc), .rpm (receita; o pacote sai onde houver
  rpmbuild) e Arch (.pkg.tar.zst com .PKGINFO e .MTREE, e o PKGBUILD para o makepkg). `DESTDIR=` testa
  sem mexer no sistema. A licença GPL-3 passa a acompanhar o projeto (LICENSE); os pacotes e o
  `instalar` trazem tudo, inclusive a tela de login como o serviço `glove-login` (greetd com
  configuração própria; o greetd vira dependência): ativar com `sudo systemctl enable --force
  glove-login` ou `./instalador.sh login` (guarda a anterior; `login remover` volta); ao remover o
  pacote com ela ativa, volta sozinha a tela de login anterior. Documentado no README
- [x] 9.14.2 Pacotes completos (P) — v0.9.22: a tela de login entra em todos os formatos (serviço
  `glove-login` + `lib/glove/glove-login-greetd`), com os passos de pós-instalação e pré-remoção de
  cada formato (deb, rpm, Arch `.INSTALL`, Slackware `doinst.sh`)

## Fase 9.15 — Internacionalização (pedido de 07/11/2026)

O texto em português fica no código e é a chave (`tr!("Abrir")`); as traduções ficam em
`locales/<idioma>.po` (formato do gettext). Idioma pelo `LANG` (o painel Internacional grava),
`GLOVE_LANG` força um; idioma sem tradução cai no inglês. `cargo test -p glove-i18n` acusa
qualquer texto marcado sem tradução em inglês; `tools/i18n.py` marca textos e registra as
traduções. Cuidado: texto que também é chave na lógica (ex.: comparar com "Cancelar") compara
com a tradução ou com outra coisa (o caminho, um identificador).

- [x] 9.15.1 Base e o glove-kit (M) — v0.9.23: crate `glove-i18n` (sem dependências; catálogos
  embutidos e em /usr/share/glove/locales ou ~/.local/share/glove/locales), `tr!` exposto pelo
  glove-config; Abrir/Salvar, impressão, painéis de fontes e cores e o alerta traduzidos; o menu
  Janela dos apps vai ao compositor como `@window` (não depende do idioma)
- [x] 9.15.2 Compositor: barra de menu, menu principal, Dock, Exposé, Painel, avisos e diálogos (M)
  — v0.9.24: menus da barra, do Dock, da mesa e dos extras (bateria, relógio, Wi-Fi, monitores),
  Busca Rápida (também acha as Preferências pelo nome traduzido; a coluna de categorias se ajusta
  ao idioma), Forçar Encerrar, bloqueio de tela, Painel, protetor de tela, encerramento, início da
  sessão e o relatório do `--monitores`; datas e números no formato de cada idioma (ordem da data,
  vírgula decimal); nomes de apps lidos dos .desktop no idioma em uso; chaves com contexto
  (`"Monitor|tela"`) quando a mesma palavra tem dois sentidos
- [x] 9.15.3 Preferências (G) — v0.9.25: todos os painéis; a busca de painéis entende os dois idiomas;
  listas fixas (painéis, idiomas, serviços, regiões) traduzidas onde são mostradas; nomes de apps dos
  .desktop no idioma em uso; `tools/i18n.py candidatos|aplicar` marca uma pasta inteira e separa os
  textos que também são chave da lógica
- [x] 9.15.4 Arquivos (M) — v0.9.26: janelas, barra lateral, Obter Informações, transferências, Lixeira,
  servidores e preferências; separador decimal do idioma (`virgula()`) em todos os apps; todos os apps
  passam a depender do glove-config (para o `tr!`); `tools/i18n.py consertar` desfaz o `tr!` onde o
  compilador não aceita (listas fixas e padrões de match)
- [x] 9.15.5 Os outros apps (G) — v0.9.27: os 30 apps restantes (Correio, Editor, Visualizador, Monitor,
  Agenda, Contatos, Fontes, Caracteres, Dicionário, Utilitário de Rede, Discos, Terminal, Busca,
  Lembretes, Música, Xadrez, Captura, Vídeo, Ajuda com os textos de ajuda, Registros, Calculadora…);
  datas por extenso e meses abreviados (também no ícone da Agenda) na ordem de cada idioma; a tecla
  decimal das calculadoras mostra o separador do idioma; protocolos (IMAP, SMTP, vCard, iCalendar,
  datas de e-mail) continuam no formato padrão
- [x] 9.15.6 Nomes do Dock e dos apps (configuração), tela de login e boas-vindas (M) — v0.9.27: os nomes
  gravados na configuração continuam em português e são traduzidos ao mostrar (Dock, barra de menu,
  Busca Rápida); atalhos, layouts de teclado, cores, rótulos e sons de alerta também
- [x] 9.15.7 Instalador vira `glove-setup-tool.sh` em português e inglês; README em inglês (M) — v0.9.28:
  mensagens, menu, perguntas ([S/n] ou [Y/n]) e ajuda no idioma do sistema; comandos também em
  inglês (install, uninstall, deps, all, login remove, help); descrições dos pacotes e scripts dentro
  deles em inglês; `README.en.md`

## Fase 9.16 — glove-ui: a biblioteca dos apps (pedido de 07/11/2026)

O instalador do sistema (Fase 11) vai ser feito com ela.

- [x] 9.16.1 Repositório próprio (M) — v0.9.29: o glove-kit vira **glove-ui**, e vai junto com o
  glove-paint, o glove-config e o glove-i18n para github.com/fabioaacarneiro/glove-ui; o Glove usa
  como submódulo em `ui/` (compila sem internet; cada versão do Glove aponta para uma versão exata);
  os catálogos de tradução são somados na compilação (`locales/` da glove-ui + `GLOVE_LOCALES` do app)
- [x] 9.16.2 Documentação, exemplos e modelo de app (M) — v0.9.30 (glove-ui v0.2.1): guia em português e
  inglês (`docs/GUIA.md`, `docs/GUIDE.md`), exemplos `ola`, `formulario` e `lista`, e a pasta `modelo/`
  (app pronto para copiar, com menus e traduções; compila baixando a glove-ui do GitHub); a glove-ui
  re-exporta `tr!` e as configurações; um app escrito em outra língua pode trazer `pt.po`

## Fase 10 — O "Tiger" "como deveria ter sido" (depois de cumprir toda a era)

Ideias para depois das fases 1 a 9.11: o que veio depois do Tiger (ou nunca veio), redesenhado no
visual gel. Cada uma será detalhada antes de começar.

- [x] 10.1 Grade de apps em tela cheia, com pasta e busca (M) — v0.10.1: "Todos os Apps" abre pelo
  Dock (ícone novo, logo depois do Arquivos; configurações antigas ganham o ícone), pela tecla F4 ou
  por um canto ativo. Desenhada pelo compositor (`grade.rs`) sobre a tela escurecida: campo de busca
  no alto (digitar já filtra), ícones grandes em páginas com pontinhos, e a pasta **Utilitários**
  (mesma regra do Arquivos: categorias System/Utility/Settings/Monitor e os utilitários do Glove).
  Setas escolhem, Enter abre, Esc limpa a busca, sai da pasta ou fecha; roda do mouse, dois dedos
  no touchpad e Page Up/Down trocam de página; clicar fora fecha. O campo e a grade são texturas
  separadas (o cursor pisca sem redesenhar os ícones) e são soltas ao fechar.
- [x] 10.2 Mesas virtuais integradas ao Exposé (M) — v0.10.2: 4 mesas por padrão (1 a 9 em
  Preferências ▸ Exposé e Painel; 1 desliga). Ctrl+Super+←/→ trocam de mesa (as janelas deslizam de
  lado e um aviso mostra as mesas); com Shift, a janela da frente vai junto. No Exposé (F9), uma faixa
  no alto mostra cada mesa com as janelas dela em miniatura: clicar vai até a mesa, arrastar uma
  janela até outra mesa leva a janela para lá. O Dock, a troca de apps e o menu Janela (que lista as
  janelas das outras mesas com o número) vão até a mesa da janela. As janelas das outras mesas saem
  do espaço como as ocultas (`mesas.rs`), sem custo de desenho.
- [x] 10.3 Olhada Rápida no Arquivos (M) — v0.10.3: Espaço (ou Arquivo ▸ Olhada Rápida, Ctrl+Y) abre
  uma janela escura com a prévia do item escolhido: imagens (com as dimensões), a primeira página do PDF
  (pelo `pdftoppm`), texto e código (o começo do arquivo, com rolagem) e, nos outros, o ícone grande. Ela
  segue a escolha: as setas, nela ou no Arquivos, passam de item. Espaço ou Esc fecham; Abrir abre no app
  do tipo. Na glove-ui, `ui.redraw_all_windows()` redesenha as outras janelas do app.
- [x] 10.4 Central de avisos (M) — v0.10.4: o Glove atende o serviço padrão de notificações do Linux
  (`org.freedesktop.Notifications`, o do `notify-send`, Firefox, Thunderbird…) numa thread (zbus) e mostra
  cada aviso num balão fumê no canto de cima à direita (6 s; críticos ficam até clicar). O balãozinho
  na barra de menu mostra quantos não foram lidos e abre a central, com os últimos 60 avisos, Não
  perturbe (sem balões, menos os críticos) e Limpar. Clicar num aviso responde ao app ("default") e traz o
  app à frente; o × dispensa. Se outro serviço já atende (dentro de outro desktop), o Glove não interfere.
- [x] 10.5 Gestos de trackpad com vários dedos (P) — v0.10.5: na sessão própria (libinput), 3 dedos para
  cima abrem o Exposé e para baixo as janelas do app (com o Exposé aberto, voltam); 3 dedos para os lados
  trocam de mesa virtual (o conteúdo acompanha os dedos; no Exposé, troca e continua nele); pinçar com 4
  dedos abre Todos os Apps e abrir os 4 dedos mostra a mesa. Liga e desliga em Preferências ▸ Trackpad
  (`gestos.rs`). Voltar/avançar com dois dedos fica com cada app (é rolagem horizontal).
- [x] 10.6 Abas no Arquivos, no Editor e no Terminal; janelas em tela cheia (G) — v0.10.6:
  - **Tela cheia**: Ctrl+Super+F ou Janela ▸ Entrar em Tela Cheia (e o pedido dos apps, como o F11 do
    Firefox ou o vídeo): a janela ocupa o monitor sem barra de título; a barra de menu e o Dock saem
    de cena e aparecem quando o mouse encosta no alto. O mesmo atalho volta ao tamanho de antes.
  - **Abas**: faixa de abas da glove-ui (`ui.tab_strip`, com × e +), que só aparece com mais de uma.
    Terminal (Ctrl+Shift+T, cada aba com o seu shell; a glove-ui observa vários descritores com
    `ui.watch_fd`), Editor (Ctrl+Shift+T; Abrir… abre numa aba nova; fechar uma aba com alterações
    pergunta antes) e Arquivos (Ctrl+T na mesma pasta, "Abrir em Nova Aba" no menu de uma pasta; cada aba
    com a sua navegação). Ctrl+Page Up/Down trocam e o Fechar fecha a aba.
- [x] 10.7 Cópias de Segurança (G) — v0.10.7: app novo (`glove-copias`, em Utilitários). Escolhe-se um
  disco externo (ou pasta); a pasta pessoal é copiada para `Glove Backups/<computador>/<data>`. Arquivos
  iguais aos da cópia anterior viram links fixos: cada cópia parece completa e só o que mudou ocupa espaço.
  Ficam todas as das últimas 24 h, uma por dia no último mês e uma por semana antes disso; com o disco
  quase cheio, as mais velhas saem. O Glove chama `glove-copias --auto` de hora em hora (10 min depois de
  entrar): copia se estiver ligado, com o disco conectado e sem cópia na última hora, e avisa na central.
  A janela tem a linha do tempo à esquerda, o conteúdo da cópia escolhida (sem os ocultos), Restaurar
  (volta ao lugar de origem; se já existir, com "(restaurado)" no nome) e Mostrar no Arquivos. Ficam de
  fora caches e lixeiras (`~/.config/glove/copias.toml`).
- [x] 10.8 Central de Apps (G) — v0.10.8: app novo (`glove-central`). Abas Buscar (Flatpak e apt juntos;
  no apt, o nome que começa com a busca primeiro e bibliotecas por último), Instalados (os apps com janela
  do apt, pelos donos dos `.desktop`, e os Flatpaks) e Atualizações (apt e Flatpak, com Procurar
  Atualizações e Atualizar Tudo). Instalar, Remover e Abrir no app escolhido; sem nenhum remoto do
  Flatpak, Ativar o Flathub (na conta do usuário). O apt pede a senha numa folha da janela e roda com
  `sudo -S` (o Glove não tem agente do polkit): a senha vai só para o sudo, nunca para a tela ou o disco.
- [x] 10.9 Fotos, Gravador e Mensagens (G) — v0.10.9: três apps novos, com ícones próprios.
  - **Fotos** (`glove-fotos`): a pasta Imagens em miniaturas por mês (feitas em duas threads; só as que
    aparecem ficam na memória), álbuns pelas subpastas, tamanho das miniaturas, a foto grande com setas,
    Apresentação (Espaço) e Abrir no Visualizador.
  - **Gravador** (`glove-gravador`): grava em WAV pelo `pw-record` (ou `parecord`/`arecord`) em Música ▸
    Gravações, com o tempo, os LEDs de nível e a onda; ouvir, mover para a Lixeira, mostrar no Arquivos.
  - **Mensagens** (`glove-mensagens`): IRC (Libera.Chat por padrão, com TLS), conversas na lateral com as
    não lidas, balões (os seus em azul), /join, /part, /msg, /me, /nick; mensagem particular ou com o seu
    apelido, com a janela no fundo, vira aviso na central.

## Fase 11 — Instalador do sistema e ISO Debian (depois da Fase 10; testada antes de fechar)

- [x] 11.1 Instalador do sistema (G) — v0.11.1, **a testar numa máquina virtual**: `glove-instalador`,
  assistente da glove-ui (Introdução, Idioma e Teclado, Fuso Horário, Disco, Conta, Resumo, Instalação).
  Disco inteiro (GPT: EFI de 512 MiB + ext4; no modo BIOS, a partição de 1 MiB do GRUB + ext4) ou ao lado
  de outro sistema no maior espaço livre (UEFI, GPT, reaproveitando a partição EFI dele). Copia o sistema
  com `unsquashfs` da imagem do pendrive, grava fstab (UUIDs), nome, idioma, fuso e teclado, cria a conta
  (grupo sudo; a senha vai pelo `chpasswd`, nunca nos argumentos), tira o que é do sistema ao vivo
  (login automático, instalador no início, pacotes live-*), refaz o initramfs e instala o GRUB (UEFI,
  com a cópia em `EFI/BOOT` para máquinas que esquecem a entrada, ou BIOS). Só instala no Glove ao vivo;
  em outro sistema abre só com `--simular`, que mostra os comandos sem executar nenhum. A tela de login
  ganhou login automático opcional (`/etc/glove/autologin`, usado pela ISO).
- [x] 11.2 `glove-setup-tool.sh iso` (G) — v0.11.2, **a testar**: gera o .deb, baixa o live-build e o
  debootstrap oficiais do Debian (a versão mais nova do repositório) para `iso/ferramentas` e monta
  com eles uma ISO Debian 13 (trixie) híbrida (pendrive ou DVD, UEFI e BIOS) em `iso/`, fora do Git:
  o Glove, o Firefox ESR, PipeWire, NetworkManager, firmwares comuns, Flatpak (com o Flathub) e o
  instalador. O sistema ao vivo entra sozinho no Glove (usuário `glove`, em português com teclado ABNT2)
  e abre o instalador. `glove-setup-tool.sh iso testar` liga a ISO no QEMU (UEFI se houver o OVMF),
  com um disco virtual de 32 GB para testar o instalador. A lista de pacotes e o ajuste do sistema ao
  vivo ficam em `data/iso/`.
- [x] 11.3 Bateria que o firmware não reconhece (M) — v0.11.3: a leitura da bateria vira a biblioteca
  `glove-energia` (vinda do Mide), usada pela barra e pelas Preferências. Três caminhos: o kernel; o
  arquivo do serviço `cros-ec-battery` e o EC do Chromebook (`/dev/cros_ec`), quando o firmware diz
  "bateria ausente". Remendos para baterias paralelas: carga pela energia quando falta `capacity`,
  estado "Unknown" decidido pelo adaptador (inclusive carregador USB-C), corrente com sinal, saúde
  escondida quando o chip informa uma capacidade de projeto absurda. Tempo até acabar e até carregar
  pela potência suavizada ou, sem potência, pela queda da carga nos últimos minutos. O menu da bateria
  mostra também "na tomada, sem carregar", a potência, a saúde e os ciclos. Corrigidas as chaves de
  tradução com contexto da Fase 10 (em português apareciam "Avisos", "Gravador" e "Instalador" no
  lugar de "Limpar", "Gravação" e "Voltar"; a pasta das gravações passa a ser Música ▸ Gravações).
- [x] 11.4 Todos os Apps como deveria ter sido (G) — v0.11.4 (glove-ui v0.2.5): ícone novo (disco de
  alumínio com aro de vidro azul e um foguete retrô, arte própria); a grade fica sobre o papel de
  parede desfocado, com o Dock na frente e sem a barra de menu, janelas e ícones da mesa. Pastas
  abrem numa faixa de tecido escuro que separa a grade, com bico e brilho azul, e nome editável.
  Modo de edição: segurar faz os ícones balançarem; arrastar muda a ordem e vira a página na
  borda; app sobre app cria pasta; sobre pasta, entra nela; para fora da faixa, sai (pasta vazia
  some). A arrumação fica em `grade.json`. Os ícones viram imagens prontas e a grade só as compõe,
  então o balanço custa pouco. Na glove-ui, a folha Abrir/Salvar faz a janela crescer quando ela é
  pequena demais (e voltar depois): o painel do Vídeo abria a folha espremida; ele também passa a
  ser redimensionável.
- [x] 11.5 Glove OS (G) — v0.11.5 (glove-ui v0.2.6): a ISO vira a distro **Glove OS**, Debian 13 por
  dentro. Pacote `glove-os` (só na ISO): desvia o `os-release` e o `/etc/issue` do Debian (ficam com
  `.debian`) e escreve os do Glove OS, refeitos depois de cada apt (`NAME="Glove OS"`,
  `PRETTY_NAME="Glove OS 0.11.5 (Debian 13 trixie)"`, mas `ID=debian`, `VERSION_ID` e o codinome do
  Debian, para os programas); logo da luva de gel mordida (ícones hicolor `glove-os`), fastfetch e
  neofetch com a luva em blocos, `GRUB_DISTRIBUTOR="Glove OS"`, papel de parede do Glove OS como
  padrão onde ele existe. A logo substitui a bolinha na barra de menu, no Sobre (que mostra "Glove
  OS" e a base Debian no Glove OS), no login, nas boas-vindas e no instalador. A ISO se chama
  `glove-os-VERSÃO-amd64.iso`, o menu de boot diz "Glove OS" e tem imagem de fundo própria.
- [x] 11.6 Vídeo e Música completos (G) — v0.11.6 (glove-ui v0.2.7): **Vídeo** com menus Arquivo
  (abrir, abrir endereço, recentes, mostrar no Arquivos), Controles (pulos de 10 s e 1 min, anterior e
  próximo da pasta, velocidade, repetir, volume, sem som), Vídeo (tela cheia, tamanho, proporção,
  capturar quadro), Áudio (faixa, atraso) e Legendas (mostrar, faixa, carregar legenda, atraso,
  tamanho), lidos do mpv pelo soquete JSON, e Preferências (continuar de onde parou, próximo da pasta
  ao terminar, placa de vídeo, idiomas). **Música** com barra lateral (Biblioteca, Tocadas
  Recentemente, listas M3U), navegador de Artistas e Álbuns, aleatório e repetir (todas ou uma),
  pastas extras de biblioteca, etiquetas pelas pastas quando faltam, aviso de música nova e
  preferências guardadas. A luva entra no lugar da bolinha também na tela de encerramento, no painel
  "Iniciando sessão", no tema do Plymouth e na Ajuda.
- [x] 11.7 Correções (P) — v0.11.7 (glove-ui v0.2.8): apps Chromium/Electron (VS Code) voltavam
  congelados depois de minimizar: a janela minimizada continuava "ativa" e, ao voltar, não recebia
  configuração nova (o Chromium só volta a desenhar com ela). Agora minimizar desativa e restaurar
  sempre manda uma configuração. A folha Abrir/Salvar numa janela que cresceu para ela ocupa a
  janela inteira (sem margem cinza). Preferências do Vídeo e da Música maiores e redimensionáveis.
- [x] 11.8 Papel de parede do Glove OS (P) — v0.11.8 (glove-ui v0.2.9): a imagem "Glove OS" (a luva
  sobre o azul) vem no pacote glove, em `share/glove/papeis/glove-os.png`; é o padrão de quem ainda
  não escolheu um e o primeiro de Preferências ▸ Mesa ▸ Imagens do Glove.
- [x] 11.9 Placa de vídeo para os apps e correções (M) — v0.11.9 (glove-ui v0.2.10): o Glove anuncia o
  **linux-dmabuf** (v4, com a placa principal; nos dois modos), então mpv, Firefox, VS Code, jogos e o
  XWayland desenham pela placa de vídeo (antes o Mesa não achava a placa: caía no llvmpipe, o OpenGL
  no processador, ou o mpv nem abria). O ícone do Navegador abre o Firefox com o perfil de sempre do
  usuário (antes, um perfil separado do Glove). As Preferências ganham rolagem (`scroll_view` na
  glove-ui) quando o painel não cabe, a janela que cresce sozinha sobe para não passar do Dock, e os
  nomes dos atalhos cabem na coluna (rótulo maior que a coluna termina em "…"). O
  `glove-setup-tool.sh` sincroniza a glove-ui (pasta `ui/`) antes de compilar: depois de um
  `git pull`, a glove-ui velha fazia a compilação falhar.
- [x] 11.10 Correções (P) — v0.11.10 (glove-ui v0.2.11): a ampliação do Dock travava porque a cada
  movimento do mouse o Dock inteiro era redesenhado na CPU (~6 ms por quadro em 1×, ~23 ms em 2×; as
  sombras dos ícones saíam de um cache de 96 itens que a ampliação esgotava). Agora a GPU monta o
  Dock em peças: cada ícone com a sombra é desenhado uma vez por faixa de tamanho e só é posicionado
  e escalado; a prateleira só é refeita quando muda de tamanho (preparar o quadro: ~5 ms → ~0,2 ms).
  O painel de senha da tela bloqueada aparece no monitor onde está o mouse. A grade "Mostrar Todos"
  das Preferências ocupa a largura toda dentro da área com rolagem (nada cortado à esquerda nem
  embaixo) e a janela fica da altura da grade. `GLOVE_PROF=1` faz o compositor registrar a cada 2 s
  quadros por segundo e o tempo de preparar e desenhar.
- [x] 11.11 Correções (P) — v0.11.11 (glove-ui v0.2.12): Alt+Tab alterna as janelas do mesmo app
  (antes Ctrl+`; configurações com o padrão antigo são atualizadas), juntando pelo app_id (ou pela
  classe nos apps X11), não pelo processo: apps que abrem um processo por janela também entram.
  "Todos os Apps": trocar de página desliza as páginas de lado (a nova entra pela direita indo para a
  frente, pela esquerda voltando; só as imagens prontas andam, nada é redesenhado) e as bolinhas
  ficam paradas. Ícone do Lixo: o fundo do cesto curva para baixo, como deve ser na perspectiva.
- [x] 11.12 Correções (P) — v0.11.12 (glove-ui v0.2.13): a folha de Salvar travava porque relia
  do disco, a cada quadro, o conteúdo de todas as pastas das colunas; agora lê uma vez. Nova Pasta
  não funcionava: o teclado ficava no "Salvar como:" e o Enter salvava o arquivo. Agora o campo pega
  o teclado, o nome padrão vem marcado e a primeira tecla o troca, Esc desiste e, criada a pasta, o
  teclado volta ao nome do arquivo. Instalador: troca os programas pelo nome (dá para instalar com o
  Glove aberto), arquivos com dono root, e oferece remover o pacote .deb/.rpm/Arch se houver.

## Fase 12 — Editor de documentos (pedido de 09/11/2026)

Um Editor de Texto bom o bastante para livro, trabalhos da faculdade, formulários e relatórios, sem
precisar de um Word. RTF é o formato padrão (abre no Word, LibreOffice e TextEdit); `.txt` continua
texto simples, com Formatar > Converter (Ctrl+Shift+T). Atalhos com Ctrl por enquanto (a troca para o
Command do Mac, quando decidida, vale para todos os apps). O usuário testa a fase inteira no fim.

- [x] 12.1 Base do texto rico (G) — v0.12.1 (glove-ui v0.2.14): estilo de cada trecho (fonte, tamanho, negrito, itálico, sublinhado,
  tachado, cor, marca-texto) e de cada parágrafo (alinhamento, recuos, espaçamento, tabulações) junto
  com o texto; desfazer/refazer e copiar/colar mantêm o estilo dentro do Editor
- [x] 12.2 Caracteres (M) — v0.12.2: aplicar ao trecho selecionado (ou ao que se vai digitar); menu Formatar >
  Fonte (Mostrar Fontes, Negrito, Itálico, Sublinhado, Tachado, Maior, Menor, Mostrar Cores); painel de
  Fontes e de Cores agindo na seleção; linhas com fontes misturadas
- [x] 12.3 Parágrafos (M) — v0.12.3: alinhamento (esquerda, centro, direita, justificado) por parágrafo; régua
  com recuo da primeira linha, esquerdo, direito e tabulações arrastáveis; espaçamento entre linhas e
  antes/depois (Formatar > Texto > Espaçamento)
- [x] 12.4 RTF (G) — v0.12.4: gravar e ler (padrão dos documentos novos); abre RTF do Word, LibreOffice e TextEdit
  (fontes, cores, parágrafos, ignorando o que não conhece); Converter em Texto Simples / Rico
- [x] 12.5 Páginas (G) — v0.12.5: modo página (folhas A4 ou Carta sobre fundo cinza), margens, Configurar
  Página, quebra de página (Inserir > Quebra de Página); o modo "ajustar à janela" continua existindo
- [x] 12.6 Cabeçalho, rodapé e notas (M) — v0.12.6: cabeçalho e rodapé com número de página, total, título e
  data; primeira página diferente (capa sem número); notas de rodapé numeradas no pé da página
- [x] 12.7 Estilos e listas (M) — v0.12.7: Título, Capítulo, Seção, Subseção, Corpo, Citação; listas com
  marcadores e numeradas (Tab aprofunda); sumário gerado dos capítulos; modelo ABNT (margens 3/2 cm,
  Times 12, 1,5, recuo de parágrafo, número de página no canto, capa sem número)
- [x] 12.8 Tabelas (G) — v0.12.8: inserir tabela, acrescentar/remover linhas e colunas, juntar células, bordas e
  largura das colunas (formulários e relatórios); gravadas e lidas no RTF
- [x] 12.9 Imprimir e exportar (G) — v0.12.9 (revisto na v0.12.10): PDF vetorial gerado pelo próprio
  Editor (as mesmas folhas da tela, com cabeçalho, rodapé, notas, tabelas e listas; fontes embutidas
  e texto que se copia); Imprimir usa esse PDF. O formato do Editor é só o RTF (pedido do usuário):
  .docx e .odt, que dependiam do LibreOffice, saíram. Guia de uso em docs/USO.md e docs/USAGE.md.
- [x] 12.10 Imagens (M) — v0.12.11: Inserir > Imagem… (PNG, JPEG e WebP) ou arrastar o arquivo para o
  documento; a imagem fica na linha, com o alinhamento do parágrafo, e diminui para caber na coluna;
  clique seleciona e o canto redimensiona (sem deformar; Desfazer volta). Gravada dentro do RTF
  (\pict com \pngblip/\jpegblip, como o Word); as imagens de RTFs do Word e do LibreOffice aparecem
  no Editor. No PDF e na impressão: JPEG como está, PNG com transparência.
- [x] 12.11 Zoom à mão (P) — v0.12.12: barra no pé da janela com − / porcentagem / + (e Ajustar à
  Largura), a página do cursor e as palavras; Ctrl + roda amplia em volta do ponteiro; Ctrl+Shift+.
  e Ctrl+Shift+, aumentam e diminuem, Ctrl+0 = 100%; barra de rolagem horizontal quando a folha não cabe.

## Fase 13 — Computadores mais simples (pedido de 09/11/2026)

O usuário notou o Glove mais pesado que o XFCE num dual-core com vídeo integrado e 4 GB.
O que mais pesa: os apps desenham tudo no processador (tiny-skia) e redesenhavam a janela
inteira a cada movimento do mouse, mesmo sem mudar nada; e os efeitos do compositor
(sombras, animações, ampliação do Dock) gastam o vídeo integrado.

- [x] 13.1 Mouse passando não redesenha (G) — v0.12.14: o kit guarda as áreas que reagem ao mouse
  em cada quadro; movimento sem entrar nem sair de nenhuma (e sem botão) não pede quadro. Quem lê
  a posição direto (arrastar, seguir o ponteiro) continua recebendo todo movimento. Áreas que, ao
  serem cruzadas, não mudaram nenhum pixel são aprendidas como mudas. Medido na sessão oculta, 80
  movimentos: Editor 80 → 0 quadros, Arquivos 80 → 10, Preferências 80 → 6; Terminal, em 40 movimentos, 40 → 0.
- [x] 13.2 Reduzir efeitos (M) — v0.12.14: Preferências > Aparência. Animações do compositor
  (minimizar, Exposé, mesas, Todos os Apps) terminam no quadro seguinte; sem ampliação do Dock, sem
  pulo ao abrir, sem sombras de janela; nos apps, sem o pulso do botão do alerta, progresso e
  giratório mais lentos, sem animação de redimensionar. Vale na hora (o Config::load liga o ajuste).
- [ ] 13.3 Medir no computador do usuário (P): `GLOVE_PROF=1` na sessão e nos apps, para escolher
  o próximo passo com números (o que ainda pesa: compositor, apps, fontes).
- [x] 13.6 O que abre sozinho (M, v0.12.18): Contas ▸ Itens de Início ganhou as abas Aplicativos e
  Serviços. Aplicativos mostra quanto cada item gasta agora (RSS dos processos da conta, pelo nome
  do programa). Serviços lista os do systemd que costumam pesar e quase sempre podem ser desligados
  (impressão, Bluetooth, modems, Avahi, AnyDesk, SSH, Docker, servidores de desenvolvimento, Exim,
  atualizações automáticas, indexador do GNOME…), só os instalados, com estado e memória
  (`systemctl show`); ligar/desligar é `enable/disable --now`, com pkexec nos do sistema, numa
  linha à parte. Monitor de Atividade ▸ Processo ▸ Itens de Início…/Serviços…; `glove-prefs
  servicos`. O Glove deixa de abrir pasystray, xapp-sn-watcher e o applet de impressora (volume já
  é dele; não há bandeja para os ícones).
- [ ] 13.4 Guardar desenhado o que não muda (G): fundo listrado e barras das janelas como imagem
  pronta, redesenhando só o conteúdo.
- [~] 13.5 Menos desenho por tecla e clique (G). Feito na v0.12.15, medido no Editor digitando:
  14,2 → 4,2 ms por letra. (a) Ui::settled: o app avisa que o quadro já saiu certo e o kit pula a
  segunda passada (antes, todo quadro com teclado ou clique era desenhado duas vezes); o Editor usa
  quando nada acima do documento mudou (barra, régua, abas). (b) Imagens do documento copiadas
  direto (Canvas::blit_pixmap), sem transformação: 1,5 ms → quase nada; e certas em HiDPI (antes
  sairiam com metade do tamanho). (c) A folha: sombra só nas beiradas e contorno em fios (2 → ~0,5 ms).
  Na v0.12.16: o Arquivos também (teclado e menus são tratados antes de desenhar; sem renomear,
  menu ou folha abertos, o quadro já sai certo): seta 8,2 → 4,3 ms; e o brilho do metal calculado
  numa linha só e repetido (Canvas::metal_sheen), idêntico na tela: 4,3 → ~3,2 ms, e vale para todos
  os apps de metal. Terminal (0,7 ms) e Notas (0,9 ms) já eram baratos.
  Na v0.12.17: o arrasto de janelas dava um tranco por segundo. A rede de segurança do compositor
  (vblank perdido) liberava a cada 1 s toda tela com quadro esperando, descartando o flip em curso;
  agora a tela guarda desde quando espera (Head.pending: Option<Instant>) e só é liberada após 1 s
  de espera real. Achado por darkstarfire (PR #1).
  Na v0.12.19, o redesenho parcial para todo app (glove-ui 0.2.22), sem o app precisar dizer o que
  mudou: (a) o Canvas anota cada operação de desenho (área em pixels + impressão digital dos
  parâmetros: caminho, tinta, texto, pixels da imagem); (b) a cada quadro o app roda primeiro só
  anotando, sem pintar (Canvas::start_record(true)); (c) a lista é comparada com a do quadro
  anterior (diff_ops, tolera operações que entram ou saem) e as áreas diferentes são juntadas em até
  3 retângulos (merge_rects); (d) o app roda de novo pintando só nelas (Canvas::set_region, que vale
  junto com o set_clip do app, em superfícies encaixadas com 3 px de folga). Rascunhos (Ui::clipped,
  editor de texto) entram na lista operação por operação (begin_sub/end_sub). Janela nova ou de
  tamanho novo pinta tudo de uma vez, já anotando. `GLOVE_FULL_REDRAW=1` volta ao jeito antigo;
  `GLOVE_CHECK_DAMAGE=1` pinta também o quadro inteiro e avisa se o parcial sair diferente.
  Medido (antes → agora, por tecla): Editor 6,7 → 0,9 ms; Notas 1,6 → 0,45 ms; Arquivos em ícones
  4,6 → 2,2 ms; em lista (pasta com 400 itens) 15,4 → 7,8 ms. No Arquivos, a lista de cada pasta
  passou a ser compartilhada (Rc) em vez de copiada a cada consulta.
  Falta: no Arquivos o que sobra é a conta do próprio app a cada quadro (ordenar e montar as linhas),
  não mais o desenho.

## Fase 14 — Captura de tela com desenho (pedido de 10/11/2026)

- [x] 14.1 Captura (v0.12.20): a janela abre em "Uma área" (depois Uma janela e Tela inteira).
  Toda captura vai para a área de transferência: quem guarda é o compositor
  (Glove::copy_file_to_clipboard, com image/png além da lista de arquivos), então vale sem
  nenhum app aberto. Os apps pedem o mesmo com Ui::copy_file_kept ("COPIAR" no canal dos menus).
- [x] 14.2 Desenhar no Visualizador (v0.12.20): a captura abre com `glove-view --desenhar`. Faixa
  com 8 cores e 4 espessuras, Desfazer, Limpar e Salvar; os traços ficam em pixels da imagem e só
  entram nela ao salvar (PNG por cima; JPEG ou imagem reduzida, num arquivo novo ao lado), girar ou
  copiar. Fechar ou trocar de imagem com desenhos não salvos pergunta antes. Glifo novo
  (Glyph::Pencil) e Canvas::stroke_pen na glove-ui 0.2.23.
- [x] 14.4 Seta de redimensionar (v0.12.21): na borda das janelas (e enquanto se redimensiona,
  inclusive pelo puxador do app) a seta vira a de duas pontas: ↔, ↕, ⤡ ou ⤢, desenhadas por código
  (chrome::cursor_resize). A até 18 px de um canto, a borda redimensiona na diagonal.
  `GLOVE_SHOT_CURSOR=1` põe o cursor nas capturas (depuração).
  Falta: os apps do Glove pedirem a própria seta (texto, mão, puxador) ao compositor.
- [ ] 14.3 Outras ferramentas (se pedirem): seta, retângulo, texto, borrar um trecho.
