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
   `v0.10.0` (10.1 → `v0.10.1`…). Da Fase 11 em diante, as etapas seguem numa sequência só: a Fase 11 usou
   `v0.11.x` e as fases 12 a 15 continuam em `v0.12.x` (cada etapa sobe o último número). Cada tag
   publica a ISO e os pacotes na release do GitHub (`.github/workflows/iso.yml`). Correções soltas entre
   etapas não mudam a versão.

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
texto simples, com Formatar > Converter em Texto Rico / Simples. Atalhos com Ctrl por enquanto (a troca para o
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
- [x] 13.6 O que abre sozinho (M, v0.12.18; painel próprio na grade das Preferências na v0.12.22): Contas ▸ Itens de Início ganhou as abas Aplicativos e
  Serviços. Aplicativos mostra quanto cada item gasta agora (RSS dos processos da conta, pelo nome
  do programa). Serviços lista os do systemd que costumam pesar e quase sempre podem ser desligados
  (impressão, Bluetooth, modems, Avahi, AnyDesk, SSH, Docker, servidores de desenvolvimento, Exim,
  atualizações automáticas, indexador do GNOME…), só os instalados, com estado e memória
  (`systemctl show`); ligar/desligar é `enable/disable --now`, com a senha de administrador numa folha da janela e `sudo -S` nos do sistema (v0.12.23: o Glove não tem agente do polkit e o pkexec ficava esperando para sempre), numa
  linha à parte. Monitor de Atividade ▸ Processo ▸ Itens de Início…/Serviços…; `glove-prefs
  servicos`. O Glove deixa de abrir pasystray, xapp-sn-watcher e o applet de impressora (volume já
  é dele; não há bandeja para os ícones).
- [ ] 13.4 Guardar desenhado o que não muda (G): fundo listrado e barras das janelas como imagem
  pronta, redesenhando só o conteúdo.
- [x] 13.5.1 Correção crítica (v0.12.24, glove-ui 0.2.25): juntar as áreas mudadas custava o cubo
  da quantidade (`merge_rects`). Com a tela do Terminal cheia e rolando (um `ls`, ou um comando
  longo voltando do histórico) eram milhares de letras mudadas: um quadro levava de 2,6 a 10 s de
  CPU. Agora, com muitos pedaços, uma passada linear junta os vizinhos (letras viram trechos) e
  depois faixas; a busca do melhor par só roda com até 24. O mesmo quadro: ~8 ms.
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
- [x] 14.5 Monitor desconectado (v0.12.25): `rescue_windows` passou a saber de onde veio cada
  janela (`snapshot_outputs` guarda os monitores antes da mudança). As do monitor que saiu vão
  para o centro do principal; maiores que ele encolhem para 90% da área útil; em zoom continuam
  em zoom na área útil dele (antes iam com o tamanho e o lugar do monitor antigo); em tela cheia,
  na tela inteira. As do monitor que ficou acompanham a posição nova dele. Vale para minimizadas,
  ocultas e de outras mesas. Teste: comando `fakeout add L A [left]` / `to` / `remove` do modo
  de depuração (monitor de mentira no backend winit).
- [x] 14.6 Janelas novas no centro (v0.12.26): `place_new_window` não usa mais cascata nem o
  terço de cima; `centered()` põe a janela no meio da área útil, igual para Wayland e X11.
  `recenter_if_fresh`: por 2,5 s depois de abrir, se o app muda o tamanho e a janela ainda está
  onde o Glove a pôs, ela é centralizada de novo (apps X11 que aparecem pequenos e crescem).
- [x] 14.6.1 Cascata só entre janelas do mesmo app (v0.12.27): `new_window_loc` vê em que
  degraus (26 px) estão as outras janelas do app em relação ao centro delas (`cascade_slot`,
  com folga de 6 px) e usa o primeiro livre (`cascade_step`, 6 degraus). Diálogos (com janela
  dona) ficam no centro.
- [x] 14.7 Abrir/Salvar redimensionável (v0.12.28, glove-ui 0.2.26): o `glove-escolher` (janela
  do portal) era criado com `fixed: true`; agora tem mínimo de 480×300 e cresce à vontade. A
  folha dentro dos apps ganhou um puxador no canto (`sheet_fit`, `FileBrowser::sheet_size`):
  cresce dos dois lados, até a janela do app.
- [x] 14.8 Tela do GRUB com o papel de parede do Glove OS (v0.12.29): `glove-setup-tool.sh grub
  [remover]` (e a pergunta no fim do `instalar`) copia a imagem para `/boot/grub`, define o
  `GRUB_BACKGROUND` e põe `data/grub/06_glove` (texto branco) em `/etc/grub.d`. Na ISO é automático:
  o pacote `glove-os` leva o `GRUB_BACKGROUND` e o `06_glove`, e o menu de boot ao vivo usa o papel
  de parede (`data/iso/includes.binary`) com o menu embaixo da luva (`data/iso/bootloaders`: tema do
  GRUB e `stdmenu.cfg` do syslinux, que o live-build copia por cima dos dele).
- [x] 14.9 Preferências redimensionável (v0.12.30, glove-ui 0.2.27): a janela deixou de ser
  `fixed`. `Ui::set_window_min_size` (no lugar de `set_window_size`) manda o tamanho do painel
  como mínimo a cada quadro: a janela acompanha o mínimo (com a animação de sempre) até o usuário
  aumentar; o que ele aumentou é guardado por eixo (`chosen_size`/`floor_size`) e continua nos
  outros painéis. No kit, o tamanho que o compositor repete nas configurações (foco) só vale
  quando muda: antes desfazia o tamanho que o app tinha escolhido depois.
- [x] 14.9.1 Os painéis acompanham a janela (v0.12.31): `grow()` (glove-prefs) dá a altura que
  sobra além do tamanho natural do painel; a caixa de abas e a lista principal de cada painel
  somam esse valor (Contas, Itens de Início, Mesa e Protetor, Internacional, Som, Rede, Data e
  Hora, Teclado, Energia, Acesso, Busca, Bluetooth, Compartilhamento, Monitores, Atualização,
  Disco de Inicialização, Fala, Impressão). O mínimo da janela é o tamanho sem esse crescimento
  (`pane_natural`). Painéis que são só formulário ficam como estavam (sobra espaço embaixo).
- [x] 14.10 Bloqueio antes de qualquer repouso ou hibernação (v0.12.32; falha achada no uso real
  em 12/11/2026: tampa fechada com HDMI, o HDMI saiu, o logind suspendeu e hibernou, e a sessão
  voltou sem senha). `watch_sleep` (lock.rs) acompanha o `PrepareForSleep` do logind numa thread
  (zbus, barramento do sistema) e segura o repouso com `Inhibit(sleep, delay)` por 300 ms, para o
  quadro do bloqueio ir para a tela antes. `lid_alone` (udev.rs): monitor externo saiu com a tampa
  fechada = apaga e bloqueia, em vez de religar a tela embutida. Teste: `sleepev going|woke` no
  modo de depuração; o caminho do monitor só tem teste de unidade (precisa do hardware).
- [x] 14.11 Tela de início (Plymouth) na ISO (v0.12.33): `plymouth` e `plymouth-label` na lista da
  ISO; o tema (`data/plymouth/glove`) vai no pacote `glove-os`, que o ativa com
  `/usr/share/glove-os/tela-de-inicio` (uma vez só, com o Plymouth já configurado; anota o tema
  anterior e o `postrm` volta para ele). O gancho da ISO chama o mesmo script com
  `--sem-initramfs` (o live-build refaz o initramfs depois dos ganchos, em `chroot_hacks`; o
  instalador já refazia o do sistema instalado). `glove-os.cfg` do GRUB acrescenta `splash` à
  linha do kernel (a menos que já tenha `splash` ou `nosplash`). Falta conferir numa ISO montada.
- [x] 14.11.1 A luva no tema do Plymouth (v0.12.34): a arte guardada em `data/plymouth/glove` era
  de antes da logo virar a luva (mostrava o orbe azul); regenerada com `glove-arte plymouth`. O
  teste `tema_do_repositorio_em_dia` (glove-arte) falha se a arte guardada ficar diferente da que
  o código gera.
- [ ] 14.3 Outras ferramentas (se pedirem): seta, retângulo, texto, borrar um trecho.

## Fase 15 — Papel de parede moderno (pedido de 14/11/2026)

Recurso que o Tiger não tinha, no visual da era: escolher o papel de cada monitor, uma imagem
panorâmica atravessando vários monitores e papel animado.

- [x] 15.1 Monitores e ajuste (G) — v0.12.35 (glove-ui v0.2.28): `wallpaper_mode` "igual" (a mesma
  imagem em cada monitor, cada um no tamanho dele), "por_tela" (`wallpapers`, um papel por conector)
  ou "panorama" (uma imagem só cobrindo o retângulo de todos os monitores, pela posição real deles:
  lado a lado, um em cima do outro, em L; cada um mostra o seu pedaço). `wallpaper_fit`: preencher,
  ajustar, esticar, centralizar, lado a lado. O papel aceita PNG, JPEG e WebP (o leitor de imagens
  foi para o glove-paint). `papel.rs` (compositor) prepara a textura de cada tela e só refaz o que
  muda; `chrome.wallpaper` segue sendo a imagem parada da principal (bloqueio, login, Exposé). O
  arquivo de monitores ganhou a posição e o tamanho lógico de cada um (`rect`).
- [x] 15.2 Papel animado (G) — v0.12.35: vídeo ou GIF (mp4, webm, mkv, mov, gif...) tocado pelo
  ffmpeg em laço, já reduzido ao tamanho da tela (ou da área do panorama) e ao ritmo escolhido (12
  a 60 quadros por segundo; 24 por padrão), com a opção leve (metade da resolução, a placa amplia).
  Para (SIGSTOP) com a tela bloqueada ou apagada, com "Reduzir efeitos", e, se pedido, na bateria
  e com app em tela cheia. Medido: vídeo 720p a 24 q/s no modo leve, ~36% de um núcleo no ffmpeg.
  O primeiro quadro (cache em `~/.cache/glove/papel`) vale para a miniatura e enquanto o vídeo não
  começa. ffmpeg recomendado no pacote e incluído na ISO.
- [x] 15.3 Preferências ▸ Mesa (M) — v0.12.35: com mais de um monitor, o modo e a disposição dos
  monitores em miniatura (cada um com o seu papel ou o seu pedaço do panorama; clicar escolhe o
  monitor no modo por monitor), o ajuste, a coleção "Vídeos e Animações" (pastas Vídeos e Imagens)
  e as opções do animado. A Pasta Imagens mostra também JPEG e WebP.
- [x] 15.4 Papel animado leve (M) — v0.12.36: na primeira vez, o vídeo é convertido numa thread de
  prioridade baixa para uma cópia já no tamanho da tela (ou da área do panorama) e no ritmo
  escolhido (x264, CRF 17: diferença invisível; o original não muda), em `~/.cache/glove/papel`
  (até 1 GB, saem as menos usadas). Enquanto isso, toca o original; quando a cópia fica pronta,
  passa para ela. Medido (vídeo 720p, 640×400 a 24 q/s): o ffmpeg foi de 32% para 6% de um núcleo,
  mais 6% do Glove trocando a textura. A placa de vídeo decodifica (`-hwaccel auto`) o original e a
  conversão; a cópia pequena toca melhor no processador com uma thread (com a placa, 11%: trazer o
  quadro de volta da placa custa mais que decodificar um vídeo pequeno).
- [x] 15.5 ISO pela integração contínua (M) — v0.12.37: a cada tag `v…`, o GitHub Actions
  (`.github/workflows/iso.yml`) monta a ISO num contêiner Debian trixie privilegiado (`ci/iso.sh`:
  dependências, Rust pelo rustup e `./glove-setup-tool.sh iso`), confere a versão do Cargo.toml com
  a tag e publica a ISO, os `.deb` e o `SHA256SUMS` na release da versão (ISO acima de 2 GB vai em
  partes). Rodado à mão, deixa os arquivos como artefato por 7 dias. `glove-arte previa` gera a
  folha de amostra do tema de ícones (o preview do glove-icons, atualizado com os 89 ícones).
  v0.12.38: o live-build exige o debootstrap do sistema (mesmo usando o baixado): instalado no
  contêiner, e o `iso` avisa quando falta.
  v0.12.39 (primeira ISO da CI testada no QEMU): caía no login de texto do tty7. (1) O pacote
  greetd ativa o greetd.service dele (agreety) como display-manager e o `enable` do glove-login,
  sem `--force`, falhava calado; o gancho agora desliga o greetd, ativa com `--force` e para a
  montagem se o display-manager não for o glove-login. (2) Faltavam os atalhos dos apps (glove-files,
  glove-instalador…): o Cargo novo da CI põe o OUT_DIR mais fundo e o `build.rs` do glove-apps os
  criava na pasta errada. O pacote agora tira os nomes de `crates/apps/src/main.rs`, e o `build.rs`
  procura a pasta `build`. O gancho confere `/usr/bin/glove-instalador`. Senha do usuário ao vivo
  (`live`) no USO/USAGE. Rust: `rust-version = "1.89"` no workspace (o mínimo das dependências;
  com um Rust mais velho o Cargo dá um erro claro) e o `glove-setup-tool.sh` confere a versão antes de
  compilar e indica o rustup. Conferido que o Rust 1.100.0 compila o Glove inteiro.
  Testado no QEMU (UEFI, ISO da release): entra sozinho no Glove e abre o Instalar o Glove OS.
  v0.12.40: o menu de boot da ISO esperava o Enter para sempre (`timeout 0` do live-build); agora
  entra sozinho em 5 s (GRUB e isolinux).


## Fase 16 — Discos completo e senha do sistema (pedido de 15/11/2026)

O Glove OS tem que bastar: as ferramentas nativas dão o que se precisa, sem ir atrás de outras.
O Discos chega ao nível do gnome-disks e serve para preparar o disco da instalação. Tudo pelo
udisks2 (o serviço de discos do sistema, pelo D-Bus), com a senha pedida pelo polkit.

- [x] 16.1 Agente do polkit (`glove-senha`) (M) — v0.12.41: a janela de senha do sistema. Sem ele,
  nenhum programa que pede permissão pelo polkit funcionava no Glove (Discos, gnome-disks, Flatpak,
  NetworkManager…). O serviço registra o objeto `/org/glove/PolkitAgent` para a sessão do logind
  (`RegisterAuthenticationAgent`) e sobe com a sessão (startup.rs; não no login nem aninhado). Cada
  pedido abre `glove-senha --pedir` (dados pelo ambiente, nunca na linha de comando), que confere a
  senha pelo `polkit-agent-helper-1` (cookie e senha pela entrada dele) e permite tentar de novo.
  Prefere a própria conta; senão, o primeiro administrador que o polkit aceita. `CancelAuthentication`
  fecha a janela. polkitd e udisks2 na ISO e nas recomendações do pacote. Testado no Glove
  aninhado com `pkexec`: janela, senha errada (aviso e nova tentativa) e Cancelar ("Not authorized").
- [x] 16.2 Discos sobre o udisks2 (G) — v0.12.42: `udisks.rs` lê tudo do `GetManagedObjects`
  (aparelhos, blocos, tabelas, partições, sistemas de arquivos, imagens loop); a lista mostra os
  aparelhos e as imagens em uso (sem os snaps); o mapa desenha as partições proporcionais (com
  largura mínima), os espaços livres (buracos de 1 MiB ou mais) e, no MBR, as lógicas numa segunda
  fileira. Detalhes do volume (tamanho, conteúdo, dispositivo, tipo de partição pelo GUID/código,
  UUID, montagem e uso). Montar, Desmontar, Abrir e Ejetar pelo D-Bus numa linha à parte (a janela
  de senha não trava o app). Atualiza a cada 2 s.
- [x] 16.2.1 Bateria do Chromebook no Glove OS (M) — v0.12.42: o `glove-energia` lia o arquivo do
  serviço `cros-ec-battery` (chromebook-linux-fixes), que o Glove OS não instala, e o `/dev/cros_ec`
  é só do root: no live de um Chromebook com firmware que não reconhece a bateria, ela não aparecia.
  `glove-bateria` (binário pequeno do crate energia, em Rust, port do serviço em Python) roda como
  root pelo `glove-bateria.service` (só onde existe `/sys/class/chromeos/cros_ec`, e não junto com o
  cros-ec-battery): carrega o cros_ec_chardev, grava `/run/cros-ec-battery.json` a cada 5 s no mesmo
  formato, desliga na bateria com 4% (ou tensão baixa) e o gancho `system-sleep` acorda a cada hora
  suspenso para conferir (desliga com 15%, ou volta a suspender com a tampa fechada). Ajustes em
  `/etc/default/glove-bateria` (ou o do chromebook-linux-fixes). Ativado pelo postinst do pacote.
- [x] 16.2.3 Ícones pelo tema em arquivos (P) — v0.12.43 (glove-ui 0.2.30): as Preferências tinham
  "Glove (desenhados)" e "Glove" (o tema PNG gerado dos mesmos desenhos, que gasta 30 a 40 MB a
  menos). O tema "Glove" passa a ser o padrão (`icons::DEFAULT_THEME`; `icon_theme` vazio = ele,
  se instalado) e a opção dos desenhados saiu. Os desenhos ficam para o que o tema não tem, para
  os ícones vivos (Agenda com o dia, Relógio, níveis do Lixo) e para gerar o tema. O `glove-arte`
  gera calendários com data fixa (17/07, `fix_calendar_date`), e o glove-icons foi regenerado.
- [x] 16.2.2 Instalador: o campo "Teste o teclado aqui" não guardava o texto (uma variável nova a
  cada quadro); agora guarda, e no Glove ao vivo o teclado escolhido já vale na sessão.
- [x] 16.3 Formatar (M) — v0.12.45: o disco (`Block.Format` gpt/dos/empty, com `erase=zero`
  opcional) e o volume (ext4, FAT32, exFAT, NTFS, btrfs, com nome; FAT em 11 letras maiúsculas).
- [x] 16.4 Partições (M) — v0.12.45: criar no espaço livre (`CreatePartitionAndFormat`, tipo pelo
  formato e pela tabela), apagar (com confirmação), Editar o tipo (Linux, Windows, FAT32, EFI,
  troca, BIOS boot; GPT ou MBR) e o nome do volume (`SetLabel`; o nome da partição GPT só sem volume).
- [x] 16.5 Redimensionar (M) — v0.12.45: `Manager.CanResize` (marcas 2/4/8/16: diminuir e aumentar,
  desmontado e montado). Aumentar até o fim do espaço livre logo depois (partição, depois o sistema
  de arquivos ocupando tudo); diminuir: desmonta, `Repair`, sistema de arquivos 4 MiB abaixo do
  novo tamanho, partição, e o sistema de arquivos ocupa o resto. Mínimo pelo uso quando montado;
  desmontado, o programa de redimensionar recusa tamanhos menores que os arquivos (montar só para
  medir abria a janela do volume). Volumes do sistema em uso ficam de fora.
  Testado num disco de imagem de 2 GB: GPT, Ext4 de 1 GB e FAT32 de 1,1 GB criadas; Ext4 diminuída
  para 500 MB e aumentada de volta (Check do udisks: íntegro); FAT32 apagada; nome mudado.
  **Defeito achado no teste e corrigido** (glove-ui 0.2.31): as folhas não bloqueavam os cliques —
  o `set_active(true)` dos botões desfazia o `set_enabled(false)` (o mesmo campo). Um clique no
  campo Nome da folha caiu no "Formatar Disco…" de trás e o disco de teste foi formatado. Agora
  o bloqueio de folha (`sheet_blocked`) e o de fora da `scroll_view` (`clip_blocked`) são campos à
  parte, que o `set_active` não desfaz (vale para todos os apps); o Discos bloqueia a janela e os
  atalhos com qualquer folha ou aviso aberto.
- [x] 16.6 Disco inicializável (G) — v0.12.44: `gravar.rs`. ISO híbrida (assinatura 55 AA no MBR):
  `OpenForRestore` do udisks (senha pelo polkit), cópia em blocos de 4 MiB com andamento e
  Cancelar, `sync_all` e conferência lendo de volta pelo `OpenForBackup`. Instalador do Windows (sem
  55 AA, com sources/install.wim|esd ou bootmgr.efi e efi/boot/bootx64.efi): a ISO ligada como loop
  só de leitura (`LoopSetup`) e montada; o pendrive formatado em GPT, uma partição "dados básicos"
  FAT32 com o nome da ISO (11 letras), os arquivos copiados e o install.wim > 4 GB dividido com
  `wimlib-imagex split` (partes de 3800 MB, andamento pelo tamanho das .swm). Formatar e criar a
  partição não pedem senha em disco que não é do sistema (polkit: modify-device = yes). Discos do
  sistema em uso nunca são oferecidos. `glove-disks --gravar <iso> <dispositivo>` para scripts.
  Testado com um "pendrive" loop de 3 GB e uma ISO UDF com a estrutura do Windows (GPT, FAT32 e
  conteúdo idêntico); a gravação inteira tem teste de unidade (precisa da senha para o real).
  wimtools, exfatprogs, ntfs-3g e btrfs-progs na ISO e nas recomendações do pacote.
- [x] 16.6.1 Imagem de disco (M) — v0.12.47: Disco ▸ Criar Imagem do Disco… (`OpenForBackup`, cópia
  com andamento para um `.img`; apaga o arquivo se der erro ou Cancelar) e Restaurar Imagem no
  Disco… (bloco a bloco, sem olhar o tipo, com conferência).
- [x] 16.7 Saúde, reparo e uso (M) — v0.12.47: saúde do `Drive.Ata` (SmartFailing, setores ruins,
  atributos falhando, horas, temperatura, último teste) e do `NVMe.Controller` (avisos críticos,
  horas, temperatura, desgaste pelo `SmartGetAttributes`), no cabeçalho e na folha Detalhes…, com o
  Teste Rápido (`SmartSelftestStart short`). Verificar e Consertar (`Check`, e `Repair` se preciso).
  Uso de cada volume: faixa no mapa e barra nos detalhes, e a porcentagem do disco; os montados pelo
  statvfs, os desmontados por Medir o Uso (montagem só de leitura **silenciosa**: o Discos anota o
  aparelho em `glove-silenciosos` e a mesa não mostra o ícone nem abre a janela; desmonta tentando
  de novo enquanto o udisks está ocupado). O rótulo já era o Editar (16.4).
- [x] 16.9.1 ISO "sleeper" (P) — v0.12.47: `--firmware-chroot false` e a lista de firmwares de uso
  diário (vídeo AMD/NVIDIA/Intel, Wi-Fi, Bluetooth e som); os de servidor (Netronome, Cavium, QLogic,
  Myricom, NetXtreme…) saem, uns 160 MB instalados a menos.
  v0.12.49: a ISO da v0.12.47 não montou: o `firmware-ipw2x00` (Wi-Fi Intel PRO/Wireless de 2004)
  pede para aceitar a licença numa pergunta do debconf, e na montagem não há quem responda (antes
  ele vinha pelo `--firmware-chroot`, que aceita sozinho). Saiu da lista.
- [x] 16.8 Instalador (P) — v0.12.46: botão "Preparar o disco com o Discos…" na página Disco; o
  instalador acompanha o Discos aberto por ele (tique de 1 s) e, quando ele fecha, relê os discos
  mantendo o escolhido. O caminho de instalar ao lado do Windows fica: Discos ▸ diminuir o NTFS ▸
  fechar ▸ "Instalar ao lado, no espaço livre".
- [x] 16.8.1 Instalar numa partição escolhida (M) — v0.12.48: `Modo::Particao(n)` no motor (desmonta,
  `wipefs` e `mkfs.ext4` só nela, tipo Linux pelo `sfdisk --part-type`, a EFI do disco reaproveitada);
  na página Disco, a terceira opção com a lista das partições possíveis (UEFI, GPT com EFI, 20 GB
  ou mais, não a EFI) e o aviso de que só ela é apagada; Resumo diz qual. Teste de unidade do plano.
- [ ] 16.9 ISO e pacote (P): udisks2, polkitd, exfatprogs, ntfs-3g, btrfs-progs e dosfstools.

## Fase 17 — Atualizações do Glove OS (pedido de 15/11/2026)

O Glove OS atualiza o que é dele (o `glove` e o `glove-os`) no próprio ritmo, sem depender do Debian 13.

- [x] 17.1 Repositório apt (M) — v0.12.50: o trabalho `repositorio` da CI (depois da ISO, só em tag)
  monta com `apt-ftparchive` a suíte `glove` (componente `main`, amd64 e all) com os `.deb` da versão,
  assina o `Release` (InRelease e Release.gpg) com a chave do segredo `APT_SIGNING_KEY` (Ed25519,
  "Glove OS (repositorio apt)", gerada em 15/11/2026; cópia de segurança com o usuário) e publica no
  GitHub Pages com a chave pública, o `novidades.json` (versão, data e o texto do commit) e uma
  página com os comandos. Sem o segredo ou sem o Pages, não publica e a ISO sai do mesmo jeito.
  O pacote `glove-os` traz o `glove-os.sources` (deb822, `Signed-By`) e a chave em
  `/usr/share/keyrings/glove-os-repositorio.gpg`. Testado localmente: assinatura conferida pelo gpgv
  e o apt lendo o repositório (candidato `glove 0.12.49`).
- [x] 17.2 Aviso e painel (M) — v0.12.50: `glove-prefs --verificar-atualizacao` (a sessão inicia; 2 min
  depois de entrar e a cada 6 h) lê o `novidades.json`, compara com a versão instalada
  (`dpkg-query`) e, se for nova e ainda não avisada, manda o aviso pelo `notify-send` com a ação
  "Ver", que abre o painel. Preferências ▸ Atualização de Software ganhou a seção Glove OS: versão
  instalada e nova, o que há de novo, Atualizar o Glove (só o índice do repositório do Glove e
  `apt-get install --only-upgrade glove glove-os`, com a senha pela folha do sudo) e Encerrar
  Sessão. Fora do Glove OS, Adicionar o Repositório do Glove. `GLOVE_REPO` troca o endereço.
- [x] 17.1.1 Ordem da CI (P): a ISO da v0.12.50 falhou porque o pacote glove-os já traz o
  repositório e o apt da montagem tentou lê-lo antes de ele existir. Agora são três trabalhos:
  `pacotes` (compila e gera os .deb no trixie, `ci/pacotes.sh`), `repositorio` (publica) e `iso`
  (usa os mesmos .deb com `GLOVE_USAR_PACOTES`, sem compilar de novo). Com o repositório fora do ar
  (fork sem a chave), `GLOVE_SEM_REPOSITORIO` tira o glove-os.sources da imagem em vez de falhar.
- [x] 17.4 Pacotes para Fedora, Arch e Slackware na release (M) — v0.12.73 (glove-ui 0.2.44; pedido
  de 16/11/2026): o fluxo `pacotes-distros.yml` (chamado pelo `iso.yml` a cada tag; também roda à
  mão) compila o Glove num contêiner de cada distribuição (`ci/fedora.sh` no Fedora 44, `ci/arch.sh`
  no Arch, `ci/slackware.sh` no Slackware 15.0, que serve também para o -current), gera o pacote com
  o `glove-setup-tool.sh` e o confere ali mesmo: instala com o gerenciador da distribuição (que
  resolve as dependências declaradas) e vê com o `ldd` se os programas acham as bibliotecas. O
  trabalho `publicar-distros` põe os pacotes na release e soma ao `SHA256SUMS`; uma distribuição que
  falha fica de fora sem derrubar a ISO nem as outras. `ci/release.sh` cria a release (notas com a
  tabela de instalação) para quem publicar primeiro.
  - Slackware: o contêiner é a instalação mínima e não há resolução de dependências; a lista de
    pacotes do `ci/slackware.sh` foi fechada com o `ldd`, e a `libseat` (que o 15.0 não traz) é
    compilada do código do seatd só para ligar.
  - Arch: no CI o pacote é montado direto do código da versão (`GLOVE_SEM_MAKEPKG`); o makepkg não
    roda como root e a receita baixaria o código de novo pela tag.
  - Achado do teste: o Glove não abria no Fedora. As fontes eram procuradas só nas pastas do Debian
    e do Arch/Slackware; agora há as do Fedora e, se nenhuma existir, o fontconfig responde
    (`font::load`, `font::font_file`; o PDF do Editor usa o mesmo para a Liberation). O `.rpm`
    passou a pedir também a `dejavu-sans-mono-fonts` (a fonte do Terminal).
- [x] 17.5 Atualização no Fedora, no Arch e no Slackware (M) — v0.12.75 (pedido de 17/11/2026). O
  aviso de versão nova já aparecia em qualquer distribuição, mas o painel só sabia do apt (oferecia
  "Adicionar o Repositório" do apt no Fedora).
  - Repositórios no GitHub Pages, montados por `ci/repositorios.sh` (o mesmo script do apt, agora
    fora do YAML e testável): `fedora/` (createrepo_c; o índice `repomd.xml` assinado, `repo_gpgcheck`;
    `glove.repo` na raiz) e `arch/` (o índice do `repo-add`, feito no trabalho do Arch, e a
    assinatura `.sig` do pacote). A chave é a do apt. Distribuição sem pacote na versão: fica o
    repositório que está no ar (`manter`, pela lista `arquivos.txt`).
  - `atualizar::pacote()` descobre com que pacote o Glove está instalado (dpkg, rpm, pacman,
    pkgtools ou nenhum) e daí saem a versão instalada, se há repositório e os comandos:
    `script_repo_dnf`/`script_repo_pacman` (a chave vem no pacote, em
    `/usr/share/glove/glove-os-repositorio.asc`; no pacman, `--lsign-key` pela impressão digital
    fixada) e `script_atualizar` (dnf com conferência da versão; `pacman -U` do pacote da versão,
    sem renovar os índices do sistema; no Slackware, o `.tgz` da release conferido pelo
    `SHA256SUMS` e o `upgradepkg`). A versão vem da rede e vai para um comando de administrador:
    só números e pontos (`versao_valida`).
  - Testado em contêineres com um repositório assinado por uma chave de teste: instalar pelo dnf e
    pelo pacman; índice ou pacote adulterado e chave desconhecida recusados; 0.12.73 → 0.12.74
    pelos comandos do painel no Fedora, no Arch e no Slackware (este, contra a release de verdade).
    Falta ver o painel em si numa dessas distribuições (a tela), e a primeira publicação de verdade.
- [ ] 17.3 Ligar no GitHub (usuário): o segredo `APT_SIGNING_KEY` e o Pages com origem "GitHub
  Actions" (feitos em 15/11/2026), e no ambiente `github-pages` a regra de tag `v*` (ele só
  aceitava publicar do ramo master). O token do gh desta máquina não tem permissão para isso.

## Fase 18 — Tela cheia que empurra (pedido de 15/11/2026)

- [x] 18.1 Barra e Dock empurram a janela (M) — v0.12.52: em tela cheia, encostar no alto fazia a
  barra de menu e o Dock aparecerem por cima da janela. Agora `fullscreen_peek` dá à janela (que
  continua em tela cheia, sem moldura) o retângulo entre a barra e a prateleira do Dock (o Dock
  aparece mesmo com "ocultar sozinho"), de uma vez, e devolve a tela inteira quando eles saem;
  `chrome.peek` (0 a 1, 0,2 s, nada com Reduzir efeitos) desliza a barra de cima para baixo e o
  Dock (pelo `dock_pos`, junto com o `dock_hide`) de baixo para cima. Só no monitor principal (os
  outros não têm barra nem Dock). Testado no Glove aninhado com o Editor.

- [x] 11.13 Ao vivo sem senha ao voltar (P) — v0.12.53: no teste num computador real, o sistema ao
  vivo apagou a tela e pediu a senha (a `live`, que ninguém sabe). O modelo de configuração do ao
  vivo (`/etc/skel/.config/glove/config.toml`, que o instalador apaga) leva
  `[energy] require_password = false`; o Super+L continua pedindo.

## Fase 19 — Achados do teste em computador real (15/11/2026)

- [x] 19.1 Disco inicializável sem pedir a senha (P) — v0.12.54: a gravação falhava porque a sessão
  em uso era anterior ao `glove-senha` (instalado por cima, sem entrar de novo): ninguém respondia ao
  polkit e o udisks negava. O Discos inicia o `glove-senha` ao abrir (com outro agente registrado,
  ele sai sozinho).
- [x] 19.2 Submenu na borda (P) — v0.12.54: no Arquivos, o submenu (Rótulo, Abrir com) era empurrado
  para dentro da janela e cobria o próprio menu. Agora abre à esquerda quando não cabe à direita
  (`sub_x_in`, com teste), e o menu sobe só o necessário perto do rodapé. Os menus ainda são
  desenhados dentro da janela do app; menus que saem da janela (xdg_popup) ficam para depois.
- [x] 19.3 Captura: temporizador e tela congelada (M) — v0.12.54: o temporizador vale para área e
  janela (`request_screenshot_area_in`/`window_in`, glove-ui 0.2.33). Ao começar a escolha, o
  compositor fotografa a tela (`grab`, sem a película) e a mostra por baixo da película
  (`Frozen`); o recorte sai dessa imagem (`shot_from_freeze`), ou ao vivo se a área for noutro
  monitor. Testado no Glove aninhado com fundo animado: quadros idênticos durante a escolha e o
  recorte de 400×300 certo.
- [x] 19.5 A central de controle congelava a tela (G) — v0.12.72 (glove-ui 0.2.43; relato de
  16/11/2026, num Slackware): `open_control` rodava `bluetoothctl show` na linha principal do
  compositor, e sem o bluetoothd esse comando nunca termina (reproduzido: 8 s de limite, sem
  resposta). A tela inteira parava; desligando à força, o disco ficou com erro. Agora:
  `glove_config::comando::output_within` roda o comando com limite e o encerra; a central abre com o
  que já se sabe (`quick_control_info`) e lê Wi-Fi, Bluetooth e volume numa linha à parte
  (`refresh_control` → `control_arrived`, uma leitura por vez, canal único); `wpctl`, `loginctl` e as
  consultas do `nmcli` ganharam limite. No painel Bluetooth das Preferências: sem adaptador não
  pergunta; sem resposta, avisa e só tenta de novo em 30 s.
- [x] 19.6 `instalar` parava com a compilação pela metade (P) — 17/11/2026: o script só olhava se
  o `glove` e o `glove-apps` existiam; com eles prontos e um programa novo ainda não compilado (o
  `glove-bateria`), não compilava e parava ao copiar. Pior: depois de um `git pull`, perguntava
  "Compilar de novo?" com "não" de padrão e instalava os programas velhos com os arquivos novos.
  Agora `stage` sempre chama o cargo (que só refaz o que falta ou mudou) e confere que todos os
  programas existem; a pergunta saiu. A receita do Arch, que compila antes, pula isso (`_stage`).
- [x] 19.4 `glove-setup-tool.sh` com sudo (P): sem cargo, como root vindo do sudo, diz para rodar sem
  o sudo; a mensagem do Rust mínimo dizia 1.85 (é 1.89).

## Fase 20 — Pedidos do teste no Glove OS (15/11/2026)

- [x] 20.1 Arquivos em lista por padrão (P) — v0.12.55.
- [x] 20.2 Nome inteiro ao passar o mouse (M) — v0.12.55 (glove-ui 0.2.34): `Ui::tooltip` e
  `tooltip_if_cut` (meio segundo parado; balão por cima de tudo no fim do quadro; acorda sozinho
  na hora). Na tabela (nome cortado da primeira coluna: Arquivos em lista e todo app com tabela),
  nas colunas e nos ícones do Arquivos e nas folhas Abrir/Salvar (`file_sheet`).
- [x] 20.3 Nome do sistema sem o Debian (P) — v0.12.55: `PRETTY_NAME="Glove OS 0.12.55"`, /etc/issue
  e o Sobre o Glove; `ID=debian` e a versão do Debian continuam nos outros campos.
- [x] 20.4 Luz do teclado (M) — v0.12.55: Ctrl + teclas de brilho e as teclas XF86KbdBrightness/
  KbdLightOnOff; `/sys/class/leds/*::kbd_backlight` (o do Chromebook primeiro), pelo logind
  (`SetBrightness("leds")`), passos de 10% (como no Mide); aviso na tela com o desenho de teclado.
- [x] 20.5 Central de controle (G) — v0.12.55: ícone na barra (ExtraKind::Control, ao lado do
  relógio); painel (`chrome::control_panel`, `controle.rs`) com Wi-Fi (nmcli, nome da rede),
  Bluetooth (bluetoothctl, rfkill unblock), Não perturbe e trilhos de brilho, luz do teclado (se
  houver) e som; comandos em linhas à parte, no máximo a cada 40 ms ao arrastar.
- [x] 20.6 "Mostrar na pasta" (M) — v0.12.55: o `glove-portal` atende o `org.freedesktop.FileManager1`
  (ShowItems/ShowFolders/ShowItemProperties; ativado pelo D-Bus como o portal), e o Arquivos ganhou
  `--mostrar ARQUIVO`; o Arquivos é o app padrão de `inode/directory`. Testado: abre a pasta com o
  arquivo escolhido.
- [x] 20.7 Botões das janelas GTK (P) — v0.12.55: na entrada, `gtk-decoration-layout=close,minimize,
  maximize:` no settings.ini do GTK 3 e 4 (e o button-layout do GNOME, se ainda for o padrão), sem
  mexer numa escolha feita. O Firefox passa a mostrar os três botões.
- [x] 20.8 Ctrl/Shift na lista do Arquivos (P) — v0.12.55: a lista usa o `multi_click` das colunas, e
  a tabela destaca as escolhidas a mais (`TableStyle::also`).
- [x] 20.9 Pastas padrão na barra lateral (P) — v0.12.55: `xdg-user-dirs` na ISO e nas recomendações,
  e a sessão roda o `xdg-user-dirs-update` ao entrar.
- [x] 20.10 Tema do Glove para o Firefox (M) — v0.12.57: `glove-arte firefox` gera o tema (manifest e
  o metal escovado do tema Metal, 3840 px); o pacote glove-os o leva em
  `/usr/share/glove-os/firefox/glove-tema.xpi`, instala e liga para todos os perfis pelas políticas
  (`/etc/firefox/policies/policies.json`: ExtensionSettings normal_installed e
  `extensions.activeThemeID`) e libera o tema sem assinatura da Mozilla
  (`/etc/firefox-esr/glove-os.js`). Testado no firefox-esr do trixie: perfil novo já abre com ele.
  O usuário ainda pode trocar de tema no Firefox.
- [x] 20.11 AppImage (M) — v0.12.57: o Arquivos (e a mesa) mandam o AppImage (extensão ou o ELF com
  "AI\x02") para `glove-files --appimage`: uma janela como as do Mac, com o ícone do programa (tirado
  do `--appimage-extract`) e a pasta Aplicativos; arrastar o ícone para a pasta (ou Instalar) copia
  para `~/Aplicativos`, torna executável e cria o atalho em Todos os Apps; "Abrir sem Instalar" só
  roda. Sem a libfuse2, roda com `APPIMAGE_EXTRACT_AND_RUN=1`; a libfuse2t64 vai na ISO e nas
  recomendações.
- [x] 20.5b Ícone da central de controle (P) — v0.12.57: dois botões como os do Mac (em cima a pílula
  clara com contorno e a bolinha escura à direita; embaixo a pílula escura com a bolinha clara à
  esquerda).
- [x] 20.12 Menus vazios em apps X11 (Arduino IDE 1.8 e outros apps Java) — v0.12.60: os menus,
  dicas e listas override-redirect (`x11_overlays`) não recebiam o frame callback, e o Xwayland
  só manda o próximo quadro depois dele: o menu ficava no primeiro, um retângulo vazio (cinza ou
  preto). Agora recebem junto com as janelas. Testado no Arduino 1.8.19: menus, submenus e dicas.
  O erro de segmentação da versão em .zip fica para quando houver o relato com a versão.
  v0.12.63: menus que ficavam parados no meio da tela ao arrastar ou redimensionar a janela (ideia
  do PR #4 de darkstarfire). No começo do arraste (`dismiss_menus_for_move`), os popups xdg da
  janela recebem `popup_done`; os menus X11 do mesmo app (mesmo pid) saem de `x11_overlays` (o
  compositor não pode fechar nem mover um override-redirect, e o Java não fecha o menu ao perder o
  foco: testado), e o app os fecha no próximo clique; se ele os mapear de novo, voltam. Os de outros
  apps ficam.
- [x] 20.13 Atualização de Software sem Terminal (M) — v0.12.56: a consulta ao `novidades.json` pede
  a resposta fresca (`?t=` e `Cache-Control: no-cache`; o GitHub Pages guarda por 10 min, e logo depois
  de uma versão o painel dizia "em dia"). Verificar Agora roda o `apt update` como administrador
  (folha de senha do sudo) e relê o Glove e a lista do sistema. Atualizar o Glove e Instalar Tudo
  (`apt full-upgrade`) rodam com `APT::Status-Fd` e `admin::run_stream`: a barra junta baixar (0 a
  50%) e instalar (50 a 100%) com o texto do apt; aviso claro se outro programa segura o apt. Sem
  apt, continua o Terminal.
  v0.12.62: aviso também das atualizações do sistema. O glove-os liga o
  `APT::Periodic::Update-Package-Lists "1"` (o apt-daily renova a lista como root, sem instalar), e o
  verificador lê `apt list --upgradable` de hora em hora (sem rede e sem senha; o Glove e o glove-os
  ficam com o aviso deles) e avisa quando aparece pacote ainda não avisado. Na central de avisos, um
  aviso com ações continua valendo depois que o balão some (o balão não manda mais
  NotificationClosed; só ao sair da central): clicar nele lá abre o painel.
- [x] 20.16 Pasta do Launchpad fluida (M) — v0.12.64: abrir ou fechar uma pasta redesenhava a grade
  inteira na CPU (todos os ícones, o tecido, os brilhos) e mandava a imagem para a GPU a cada quadro.
  Agora `build_fold_gpu` desenha uma vez a página (sem a pasta), a pasta, a faixa aberta de todo e a
  borda de baixo, e `fold_gpu_parts` só move, recorta e esmaece as camadas a cada quadro (`mem_part`).
  Sem arrastar, editar, renomear ou buscar; nesses casos continua o desenho de antes.
- [x] 20.17 Painel de Fontes (P) — v0.12.64 (glove-ui 0.2.37): a linha cortada no alto da lista
  rolada passava por cima do campo de busca; a lista agora é recortada na área dela.
- [x] 20.18 Formas no desenho (M) — v0.12.65 (glove-ui 0.2.38): no Visualizador, ao desenhar, a barra
  ganha **Forma** (livre, linha, tracejado, seta, retângulo, elipse; glifos novos no glove-paint).
  Cada `PenStroke` guarda a forma; tudo vira linhas no `path` (o tracejado em pedaços, a seta com a
  ponta), então a tela e a imagem salva desenham igual. Shift: quadrado, círculo, linha a 45°.
- [x] 20.19 Cabeçalho, rodapé, número de página e sumário como elementos (G) — v0.12.66: o menu Inserir
  não repete mais o que estava no Formatar (Cabeçalho e Rodapé saiu de lá); Cabeçalho…, Rodapé…,
  Número de Página… e Sumário… ficam marcados quando existem e abrem a janela de cada um (pôr,
  configurar, remover), também com duplo clique na área deles (`DocView::hf_zone`) ou no sumário.
  `HeaderFooter` ganhou letra, tamanho, negrito, itálico, cor, distância da borda, recuo e fio;
  `PageNumber` (no alto/embaixo, alinhamento, 5 formatos, início, primeira folha) e `TocOptions`
  (título, nível, páginas) são novos. No RTF: `\*\glovestyle`, `\*\glovepagenum`, `\*\glovetoc`
  (exatos para o Editor) e, para o Word, o número entra como campo PAGE no cabeçalho ou rodapé.
- [→] 20.14 Variante escura (G): passou para a Fase 21 (Moderno Escuro).
- [~] 20.15 Papéis de parede em gel, 4K — feitos na v0.12.58 (gerados pelo `glove-arte`, pacote
  glove-papeis) e retirados na v0.12.61: não ficaram bons; fica só o papel original do Glove OS. O
  glove-os tem `Conflicts: glove-papeis` (a atualização remove o pacote) e quem estava com um deles
  volta para o original (glove-ui 0.2.36).

## Fase 21 — Aparência Moderna e ícones do Mide (pedido de 15/11/2026)

O visual da época (Tiger) continua o padrão. Para quem quiser algo mais atual, uma aparência
**Moderno**, em **Moderno Claro** e **Moderno Escuro**, inspirada na geração de 2021 do sistema da
maçã (leve e moderna perto dos originais, mas sem chegar ao visual mais novo, que é o do Mide: o
Glove não pode tornar o MideOS desnecessário). Sem nomes nem arte da Apple na interface.

- [x] 21.1 Ícones do Mide nativos (M) — v0.12.67: o pacote `glove-icones-mide` (o mide-icons, fork do
  WhiteSur sem arte da Apple, num commit fixo; temas Mide, Mide Claro e Mide Escuro, em SVG) é gerado
  na CI, vai na ISO e é recomendado pelo glove e pelo glove-os (vem junto nas atualizações). v0.12.71:
  renomeado para `glove-mide-icons` (substitui o antigo) e virou dependência do glove-os, porque só
  como recomendação as atualizações não o instalavam. Em
  Preferências › Aparência, como alternativa moderna aos ícones do Glove; nos apps do Glove, o tema
  usa os desenhos próprios do Mide (`mide-files`, `mide-terminal`…) antes dos nomes freedesktop.
- [x] 21.2 Aparência Moderno Claro e Moderno Escuro (G) — v0.12.69 (glove-ui 0.2.41): duas variantes
  novas (`Variant::Moderno`/`ModernoEscuro`) com a paleta delas (`Theme::modernize`: texto, listas,
  controles, separadores) e um estilo global do processo (`theme::set_style`, ligado onde o tema
  principal é montado, para os desenhos que não recebem o tema). Restilizados: barra de título lisa
  (cantos de 10, semáforo plano, título sem relevo), sombras maiores e macias, barra de menu
  translúcida com realce arredondado, menus arredondados com o realce por dentro, Dock em vidro fosco
  arredondado com o pontinho, o Arquivos sem metal, e no kit: `gel_rect`/`gel_circle` lisos (todos os
  botões, caixas, rádios, teclas e pílulas de uma vez), barra de progresso fina, barras de rolagem
  finas sem setas, segmentados lisos, texto branco no botão principal. Nas janelas dos apps (só
  nelas, nunca nos ícones nem nas imagens), os degradês cinza-claros fixos viram lisos e, no Escuro,
  o texto escuro clareia e os fundos e degradês cinza-claros escurecem (`Canvas::dark_remap`,
  `draw::set_grad_remap`), o que vale para os apps sem mexer em cada um.
  - [ ] 21.2.1 Revisar app por app no Escuro (cores vivas e cinzas médios fixos que o remapeamento
    não pega: a mesa do Discos, a grade das Preferências, os painéis do Painel de Widgets).
  - [x] 21.2.2 Campos de texto (v0.12.70): lisos e arredondados, com o anel de foco só no campo em
    uso; as listas suspensas já ficam lisas pelo `gel_rect`. Falta: abas lisas.
  - [x] 21.2.4 Moderno mais liso (v0.12.74, glove-ui 0.2.45): sem listras em lugar nenhum
    (`pinstripes` vira fundo liso), tabelas com cabeçalho liso (a coluna ordenada em negrito com uma
    divisa, sem o azul) e linhas alternadas em cinza bem leve (a Busca e o Monitor mudam junto), e
    linhas finas e contornos que escurecem no Escuro. Ícones como nos sistemas de hoje: na barra
    lateral (Arquivos, Abrir/Salvar) os de contorno do tema na cor de destaque, numa lateral cinza
    encostada na lista, sem moldura; nas listas, os coloridos do Mide também pequenos.
  - [x] 21.2.5 Visual por app no glove-ui (v0.2.45): `set_visual(Visual::Classic | Modern | System)`
    fixa o visual de um app sem mudar a estrutura dele (documentado no guia).
  - [x] 21.2.3 Painéis do compositor (v0.12.70): a central de controle, a central de avisos, a Busca
    Rápida, Forçar Encerrar e o volume escurecem no Moderno Escuro (`draw::PanelStyle`, guarda que liga
    o remapeamento só enquanto o painel se desenha). A grade de apps já é escura.
- [x] 21.3 Janela do cabeçalho do Editor de Texto (P) — v0.12.68: rótulos curtos numa coluna medida,
  os controles encolhem com a folha e as explicações vão ao lado só se couberem (cortadas com "…");
  testado com a janela no tamanho mínimo.
- [x] 21.4 Zoom no Visualizador e na captura (M) — v0.12.68 (glove-ui 0.2.40): Ctrl + roda aumenta e
  diminui em volta do ponteiro (o ponto embaixo dele fica no lugar), sempre com a mesma escala nos
  dois eixos; Shift + roda vai para os lados; maior que a área, aparecem as barras de rolagem em gel
  (`Ui::vscrollbar_in` e `hscrollbar`, ligadas ao mesmo deslocamento do arrasto). Vale desenhando.
- [x] 21.5 Discos (P) — v0.12.68: o nome e o tamanho dos volumes no mapa (gel azul) em texto escuro.
- [x] 21.6 Ícone do Xadrez (P) — v0.12.68: o peão torneado (pé, base, anel, corpo, colarinho e a
  cabeça redonda) no lugar da torre com defeitos; o glove-icons foi regenerado.
- [x] 21.7 Ajustes do uso real (v0.12.75, glove-ui 0.2.46; pedido de 17/11/2026):
  - Alt+Tab com a faixa das janelas do app (`WinSwitcher`, switcher.rs): o mesmo fundo da troca de
    apps, com as miniaturas das janelas (`push_scaled_window`, nas casas de
    `chrome::window_switcher_layout`) e o título da escolhida; soltar o Alt traz a janela, Q a
    fecha, Esc cancela. Substitui o `cycle_app_windows`, que trocava sem mostrar nada.
  - Ícones dos painéis das Preferências no tema escolhido (`Icon::pref_name`, nomes freedesktop de
    "preferências"); no tema do Glove continua o desenho.
  - Moderno: o seletor (`Ui::popup`) sem a ponta de gel, com as setas num quadradinho liso.
  - Favoritas no papel de parede (`wallpaper_favorites` na configuração): grade, a casa "+
    Adicionar…" (a folha Abrir), botão direito nas outras coleções junta, "×" tira.
  - Senha de administrador numa janela própria: `glove-senha --sudo` confere com o sudo e devolve a
    senha pela saída (`glove_config::senha::Pedido`); a folha dentro da janela fica só como
    reserva. As folhas que continuam (Discos, Agenda, senha nova…) usam `Ui::sheet_panel`, com a
    sombra em degradê no lugar do retângulo cinza.
  - Dock com os ícones do Mide: a sombra (no formato do ícone) era guardada sem o tema na chave e
    ficava a do desenho antigo; o tema entrou na chave (`icons::theme_name`).
  - As 28 frases da gravação de tela (PR #5) ganharam a tradução em inglês.
  - Arquivos no modo lista: apertar uma linha não começava o arrasto (só nos ícones e nas colunas)
    e a pasta sob o mouse não virava destino; agora arrasta para pastas, outras janelas e apps.
  - Compositor: sem o inotify (o limite de vigias do sistema cheio), as Preferências não tinham
    efeito; agora confere a data do config.toml a cada 2 s nesse caso.
  - Os atalhos da gravação de tela (PR #5) entraram no grupo Sistema do painel Atalhos.
  - A janela de senha (glove.Senha) tem nome e o ícone do chaveiro no Dock e na barra.
- [x] 21.8 Repouso que apaga o monitor (v0.12.76; pedido de 17/11/2026): `Glove::sleep` bloqueia, deixa
  a tela de bloqueio 3 s à vista (`SLEEP_SHOW_LOCK`), apaga o monitor (`screen_off`) e só então
  chama o `systemctl suspend` (o erro dele vai para o registro). Antes, se a suspensão não
  acontecia, ficava tudo aceso. Dentro de outro desktop não suspende (só bloqueia e apaga).
- [x] 21.9 Atualização de Software: versão e rolagem (v0.12.77; relato de 17/11/2026):
  - A versão instalada vinha do dpkg mesmo com o pacote já removido (estado "deinstall ok
    config-files": mostrava 0.11.9). A 0.12.75 já olhava o estado; agora pela terceira palavra
    dele (`dpkg_instalado`), que também aceita o pacote segurado ("hold ok installed").
  - Lentidão ao rolar: o painel rodava o dpkg e shells (`command -v`) a cada quadro. A versão e o
    repositório são lidos ao abrir e depois de cada operação; o gerenciador de pacotes e o nome
    do sistema, uma vez.
  - Barra de rolagem solta no meio da janela: a área que rola das Preferências passa a ocupar a
    janela toda abaixo da barra (`PANE_PAD` por dentro), e a barra fica encostada na borda.
- [x] 21.10 Ajustes de 18/11/2026 (v0.12.78, glove-ui 0.2.47):
  - Ocultação automática do Dock ligada ou desligada: as janelas em zoom não acompanhavam a área
    útil nova (o reajuste só via o Dock mudar de tamanho). `chrome.last_work_area` compara a área
    de um quadro com a do anterior e chama `refit_zoomed` sempre que ela muda.
  - Todas as senhas em janela própria: `glove-senha --campos` (`glove_config::senha::Formulario`)
    mostra um formulário (campos escondidos ou não, confirmação de senha nova, aviso de erro) e
    devolve os valores pela saída. Usado na senha do Wi-Fi, na troca de senha da conta e do
    Chaveiro; o Instalador pede a de administrador pelo `--sudo`. A folha fica só de reserva.
    O login do "Conectar ao Servidor" (Arquivos) já é uma janela própria.
  - Lâmpada Mágica em duas etapas, como no Mac: primeiro a parte de baixo da janela estica até o
    ícone e afina no funil, com o topo parado (40% do tempo); depois a janela escorre pelo funil.
  - Ícone da central de controle na barra: as duas chaves só no contorno, com a bolinha cheia
    (nada pintado de branco: no tema escuro virava um bloco branco).
- [x] 21.11 As últimas senhas pelo Terminal (v0.12.79): o Disco de Inicialização e o "Instalar no
  Terminal…" da Atualização de Software (fora do apt) abriam o Terminal para o sudo pedir a senha.
  Agora usam a janela de senha (`admin::Prompt`) e rodam em segundo plano (`upgrade_all`: dnf,
  pacman, zypper sem perguntas; efibootmgr e, no "Reiniciar Nele", o `systemctl reboot` depois).
  Ficam campos de senha só em formulários que não são pedidos: a conta de e-mail no Correio, a
  conta nova no Instalador, a tela de login e o "Conectar ao Servidor" (já uma janela própria).
- [x] 21.12 Popups fora da janela (v0.12.80, glove-ui 0.2.48; trabalho começado por outra sessão em
  17/11/2026 e terminado em 19/11): o 19.2 tinha deixado os menus desenhados dentro da janela.
  - Kit: a lista suspensa (`Ui::popup`) abre num xdg_popup próprio (`ListPopup`, platform.rs): o item
    atual sobre o botão, desliza para dentro da tela e, alta demais, encolhe e rola (setas, rodinha,
    mouse parado na seta); teclado e Esc. `GLOVE_POPUP_IN_WINDOW=1` volta ao desenho dentro da janela.
  - Compositor: grab de popup (`XdgShellHandler::grab`, `popup_grab`, `end_popup_grab`): o teclado
    vai para o popup, clicar fora fecha e o foco volta à janela na hora; popups acima da barra de
    título e fora das bordas no `surface_under`; não cobrem a barra de menu nem o Dock.
  - Testado na sessão oculta: lista do Teclado passando da janela, escolha pelo teclado e pelo mouse,
    Esc, clicar fora, lista de 40 itens rolando (rodinha e seta), e a lista suspensa do GTK (zenity).

## Fase 22 — Lista de 21/11/2026

- [x] 22.1 Dock no Moderno (v0.12.82): bordas finas em cima e dos lados, faixa de baixo mais alta
  com o pontinho de app aberto no meio dela, com folga do ícone e da borda.
- [x] 22.2 Mesa (v0.12.82): as miniaturas dos papéis de parede rolam (roda e barra) quando não
  cabem, em todas as coleções.
- [x] 22.3 (v0.12.89) Confirmações e senhas sempre em janela própria (regra: nunca dentro da janela
  do app). `Ui::alert` abre uma janela filha (sem barra, com sombra e, no Moderno, os quatro cantos
  arredondados), centrada sobre a dona perto do alto; a dona fica parada até a resposta. Vale para
  todo app sem mudar nada nele (GLOVE_ALERT_SHEET=1 volta à folha). As senhas já eram janela
  própria (glove-senha, v0.12.78). Botão azul do Moderno com texto branco.
  - [x] 22.3.1 (v0.12.91) Revisão das cores do Moderno Escuro em 13 apps: sem metal escovado
    no Moderno (`brushed_metal` vira faixa lisa, que escurece; a barra do Música), visor da
    Calculadora neutro (escuro no Escuro), a sigla do teclado na barra legível no Escuro. As
    abas segmentadas já estavam certas.
- [x] 22.4 (v0.12.83) Capturas na pasta Imagens/Capturas de tela (criada se não existir); o Visualizador
  salva ali por padrão.
- [x] 22.5 (v0.12.87) Busca Rápida no Moderno: no centro da tela, no visual moderno (campo grande,
  cantos arredondados, sombra macia, seleção em pílula; o Escuro pelo PanelStyle).
- [x] 22.6 (v0.12.85) Moderno: os quatro cantos arredondados em todas as janelas (no compositor, com um
  programa de textura que recorta os cantos de baixo de cada superfície: vale para qualquer app).
- [x] 22.7 (v0.12.90, glove-web 1.0.0) Navegador próprio do Glove (base Chromium), leve para
  máquinas fracas: `crates/web` (workspace à parte), o CEF 156 sem janela (a página desenhada numa
  janela do glove-ui), abas, barra de endereço com busca, menu de contexto solto do Glove,
  downloads, alert/confirm como alertas do Glove, uma instância só (socket). Sem o processo da GPU,
  até 4 processos de página, sem cache de voltar e sem rede de fundo. Pacote `glove-web` (o motor
  em /usr/lib/glove-web), dependência do glove-os e na ISO; a CI só recompila quando a versão dele
  muda. O ícone do Navegador (glove-navegador) abre ele primeiro.
  - [x] 22.7.1 Cursores (mão nos links): v0.12.94. O resto foi para a 22.13.
- [x] 22.8 (v0.12.88) Menus de contexto soltos (acima de tudo, fora da janela); o submenu abre ao lado do
  item, alinhado com a primeira opção; só sobe se faltar espaço no monitor. O app pede o menu ao
  Glove (`Ui::context_menu`, protocolo `CTX … CTXEND` no socket dos menus) e o compositor o desenha
  como os da barra; a escolha volta como ação de menu. Arquivos (com as bolinhas dos rótulos) e
  Visualizador já usam; fora do Glove, o menu antigo dentro da janela continua.
- [x] 22.9 (v0.12.86; tempos na v0.12.95) Lâmpada Mágica: 500 ms em duas fases (funil de 200 ms sem
  mexer no topo da janela, depois 300 ms sugada para o ícone); restaurar é o inverso.
- [x] 22.10 (v0.12.84) Mesa: opção de mostrar a mesa (como o F11) ao clicar numa área livre dela.
- [x] 22.11 (v0.12.93) O Firefox sai da ISO: o navegador do Glove OS é o glove-web. Quem quiser outro
  instala pela Central de Apps (o Firefox instalado continua ganhando o tema do Glove).
- [x] 22.12 (v0.12.94, Jangada 1.2.1) O navegador virou a **Jangada**, em repositório próprio
  (github.com/fabioaacarneiro/jangada, com o histórico), pacote `jangada` (substitui o glove-web,
  navegador padrão do sistema), publicado na release dela e trazido pelo CI do Glove (versão fixa
  em `jangada_version`). Ícone próprio (a jangada no mar redondo) no Dock, no tema e no pacote; o
  Dock migra o Navegador para ela. Visual e comportamento do Safari: barra com só o site e o
  cadeado, recarregar dentro dela, início, abas com o ícone dos sites, página inicial e de erro
  próprias, Sobre a Jangada; o user agent diz Jangada. No Glove todo: cursor de mãozinha e de
  texto (protocolo cursor-shape no compositor e `Ui::set_cursor`).
- [x] 22.13 (Jangada 1.2.1) Abas como as do Safari (iguais, a escolhida em destaque, ícone e
  título no meio, × ao passar o mouse); Preferências (Geral, Busca, Privacidade, Extensões,
  Avançado); favoritos (estrela, barra, menu, página inicial); histórico no menu; buscar na página;
  salvar como PDF; bloqueador de anúncios e rastreadores, modo escuro nas páginas, Não Rastrear/GPC,
  bloqueio de janelas sozinhas, apagar cookies; buscador escolhido; reabrir as abas; zoom padrão.
- [x] 22.14 (v0.12.95, Jangada 1.3.0) Jangada completa: janela sem barra de título (os botões da
  janela na barra dela, `use_own_frame_main`/`Ui::window_buttons`/`Ui::toolbar_drag`; o compositor
  dá sombra e cantos às janelas sem moldura, no clássico só os de cima), campo de endereço mais
  baixo, abas sempre à vista com um só + depois da última e as de fundo um tom acima da faixa,
  botão do meio (aba por trás; fecha a aba) e rolagem lateral, Reabrir a Última Aba Fechada,
  Reabrir as Abas da Última Sessão, janela Histórico, janela Downloads, Imprimir pela folha do
  Glove. Desempenho: a página vai do motor direto para a imagem da aba só nas áreas que mudaram,
  e para a janela em faixas com chave (sem conferir bytes); a janela só é refeita quando algo
  muda (`App::tick_redraw`). Medido na mesma página: ~13% → ~3,5% de CPU no processo da janela.
  - [ ] 22.14.1 Ainda falta a entrada de idiomas (IME). Feitos depois: senhas e autopreenchimento
    (Jangada 1.4.0), extensões no formato do Chrome com scripts nas páginas e armazenamento
    (1.11.0; as da loja com script de fundo continuam fora: o motor sem janela não as carrega) e as
    ferramentas do desenvolvedor (1.12.0).
- [x] 22.15 (v0.12.95) Menus de contexto no monitor do clique (e o submenu no da janela-mãe),
  desenhados também nos monitores extras; ícone da central com a bolinha de cima à esquerda e a de
  baixo à direita; logo da barra de menus chapado no Moderno (preto no claro, branco no escuro);
  **Usar a aparência em todo o sistema** (Aparência): abertura da sessão, bloqueio e tela de saída
  no Moderno, e, com a senha de administrador, a tela de login (/etc/glove/login) e os temas do
  Plymouth glove-moderno e glove-moderno-escuro (`glove-prefs --tema-sistema`); Monitor com o % de
  CPU dos processos como fatia da máquina inteira (a soma bate com o resumo embaixo).
- [x] 22.16 (v0.12.96, Jangada 1.3.1) A luva chapada redesenhada em vetor (palma, quatro dedos com
  os vãos, polegar, punho e a mordida), no lugar da silhueta tirada da imagem de gel; ícone da
  central como nos sistemas de hoje (de cima só o contorno, bolinha à esquerda; de baixo cheio, com
  a bolinha vazada à direita); o glove-ui não chama mais o app nas passadas de pintura da janela de
  alerta (o "Sobre" da Jangada piscava e o OK sumia); Jangada: iniciar a sessão no Google
  (Preferências ▸ Contas e menu Jangada) com o user agent de um Chrome comum; o atualizador do Glove
  também atualiza a Jangada.
- [x] 22.17 (v0.12.97, Jangada 1.4.0) Preferências › Telas do Sistema: tema próprio para a sessão
  (abertura, bloqueio, saída), a tela de login e a abertura do sistema (`screen_session`,
  `screen_login`, `screen_boot`; "" segue a opção "em todo o sistema"); o comando de administrador
  usava o caminho do glove-apps (o multi-call não sabia que era o glove-prefs) e o login nunca
  mudava: agora usa o atalho e confere o arquivo depois. Jangada 1.4.0: escudos com o motor do Brave
  (`adblock`: rede e cosmético, contagem por aba, por site, cookies de terceiros, WebRTC),
  gerenciador de senhas no chaveiro (guardar ao entrar, preencher), importar do Chrome, Chromium,
  Brave, Edge e Vivaldi (favoritos, histórico, cookies, senhas; AES-128-CBC com a chave do
  chaveiro), menu Extensões, passkeys desligadas (travavam o login do Facebook), Contas sem piscar.
- [x] 22.18 (v0.12.98, Jangada 1.5.0 a 1.10.0) Sliders com um só padrão (botão redondo; o trilho
  acompanha o moderno) e alça de redimensionar moderna; Teclado ▸ Adicionar Layout (janela com
  busca no catálogo do sistema); Telas do Sistema com a prévia do bloqueio, da saída e do início no
  tema escolhido. Jangada: sugestões do histórico e dos favoritos na barra, janela privada (tudo na
  memória), Ver Código-Fonte, página inicial personalizável, permissões dos sites (câmera,
  microfone, localização, notificações; guardadas por site) e pacotes para Fedora, Arch e
  Slackware, cada um conferido instalando na própria distribuição.
- [x] 22.19 (v0.12.99, Jangada 1.11.0 e 1.12.0) Barra de menu e Dock em todos os monitores (mesa
  estendida): a barra pintada na largura de cada monitor, o mouse traduzido para o item
  equivalente do principal e os menus abertos no monitor do clique; só o Dock sob o mouse amplia;
  a barra dos monitores sem a janela ativa esmaecida; zoom nos extras entre a barra e o Dock deles;
  Preferências ▸ Monitores liga ou desliga cada um (`[monitor] menubar_all`, `dock_all`,
  `dim_inactive`). A Jangada entra nos repositórios dnf e pacman do Glove (assinados pela mesma
  chave) e o instalador a instala no Fedora, Arch e Slackware. Jangada 1.11.0: extensões no formato
  do Chrome (pasta, .zip ou GitHub; linha de comando e Preferências ▸ Instaladas). Jangada 1.12.0:
  ferramentas do desenvolvedor (o inspetor do motor numa aba, pela porta local de depuração,
  desligada por padrão) e um teste que exige tradução para todo texto.
  - [x] 22.19.1 (v0.13.1) A central de controle, o volume e a Busca Rápida abrem no monitor da
    barra em que foram clicados.
- [x] 22.20 (v0.13.0) Tela de login nos temas modernos com layout próprio: o papel de parede
  desfocado na tela inteira (pintado em 1/4, desfocado em caixa e ampliado), os usuários lado a lado
  com fotos redondas, a senha embaixo da foto escolhida com a seta de entrar e Cancelar, Repouso,
  Reiniciar e Desligar no pé da tela; o Glove da tela de login estica na tela toda a janela que não
  é de tamanho fixo. Telas do Sistema ▸ **Ver a Tela de Login…** abre o login de verdade numa janela,
  em modo de teste e no tema escolhido (`glove-login --fake --tema TEMA`). Internacional ▸ **Instalar
  Este Idioma…** (pela senha de administrador; `glove-prefs --instalar-idioma`), conferido no Debian,
  Ubuntu, Arch e Fedora.
  - [x] 22.20.1 (v0.13.2, Jangada 1.13.0) O painel "Iniciando sessão" aparece de verdade: o relógio
    dele começa no primeiro quadro desenhado e a mesa é preparada por etapas enquanto ele cobre a
    tela (antes o primeiro quadro preparava tudo e o painel passava com a tela preta). Com o Dock em
    todos os monitores, o que some sozinho sobe só no monitor do mouse, o Dock dos monitores sem
    foco fica esmaecido como a barra, e cada barra acende o próprio menu (as outras não piscam). A
    Lâmpada Mágica também com o Dock à esquerda ou à direita (o shader gira o funil). Login escuro:
    bolinhas da senha e ícones de Repouso, Reiniciar e Desligar brancos (o kit desenha as bolinhas
    sem o remapeamento do escuro, o que vale para todo campo de senha). Jangada 1.13.0: a placa de
    vídeo desenha as páginas por padrão (se não subir, o motor volta ao processador sozinho).
  - [x] 22.20.2 (v0.13.3, Jangada 1.14.0) A miniatura das minimizadas não sobe no Dock do principal
    quando o Dock que some sozinho é revelado em outro monitor. Jangada 1.14.0: na sessão Wayland
    (sem DISPLAY) a placa de vídeo nunca subia (o EGL sem tela não acha configuração); agora o
    motor usa o ANGLE sobre o Vulkan, que sobe sem tela, e é atendido na hora em que pede para
    trabalhar (antes esperava tiques de 16 ms fora do compasso da tela: a rolagem dava trancos).
    Kit: a janela entrega o quadro sem copiar a mesma área várias vezes.
  - [x] 22.20.3 (v0.13.4) Bandeja de ícones na barra de menu: o Glove atende
    `org.kde.StatusNotifierWatcher` (e se registra como host) no D-Bus da sessão; os ícones dos
    apps (StatusNotifierItem: IconPixmap ou IconName pelo tema, relidos a cada sinal) viram extras
    à esquerda dos do sistema (`Extra::Tray` no kit). Clique = `Activate` (ou o menu, se o app só
    tem menu ou não diz: os da appindicator); botão direito = o menu do app
    (`com.canonical.dbusmenu`: GetLayout vira menu do Glove, Event "clicked" no item). Os ícones
    saem quando o app deixa o barramento. Sem a bandeja, o Telegram encerrava ao fechar a janela.
    Conferido com um item de teste e com o `nm-applet` num D-Bus próprio.
  - [x] 22.20.4 (v0.13.5, Jangada 1.15.0) Feeds longos sem travar: os scripts de página da Jangada
    liam a página inteira a cada 0,7 s (escudos genéricos) e regravavam a folha de estilo toda a cada
    resposta; o de senhas procurava o campo de senha a cada mudança. Agora só o que chegou, quando o
    navegador está livre, cada resposta numa folha nova, e senhas no máximo 1×/s. Num feed simulado
    (45 s): de 43 tarefas longas (6,9 s travado) para nenhuma. Com as Jangadas de verdade e os
    escudos ligados (página servida num domínio falso): 1.14 com 7,9 s travado e pior quadro de
    317 ms; 1.15 com 0,17 s e 83 ms.
  - [x] 22.20.5 (v0.13.6, Jangada 1.15.1) Clicar numa sugestão da barra de endereço abre o site.
    O campo da barra perdia o foco no apertar do botão (fora dele), a lista sumia no mesmo quadro e
    o soltar não achava mais a sugestão; o clique ia para a página. Agora a escolha vale no apertar
    e a página não recebe esse clique até o botão subir. Conferido na sessão oculta: 1.15.0 deixa o
    clique na página, 1.15.1 abre a sugestão.

## Fase 23 — Jangada com vídeos H.264 e mais fluida: o motor WebKitGTK (23/11/2026)

- [x] 23.0 Decisão (23/11/2026): trocar o motor em vez de compilar o Chromium. Compilar o
  CEF com os codecs custaria ~30 GB de código e uma compilação quase inteira por mês (cada
  atraso deixava falhas conhecidas sem correção); o WebKitGTK é o motor que as distribuições
  mantêm, com as correções de segurança pelo gerenciador de pacotes, e os vídeos pelo GStreamer.
  A Jangada 1.x (CEF) fica na branch `cef`.
- [x] 23.1 Motor com H.264/AAC (Jangada 2.0.0). O GStreamer do sistema decodifica H.264, AAC,
  VP9, AV1 e Opus (no Fedora, o H.264 pelo OpenH264); com o VA-API, pela placa de vídeo.
  Conferido: YouTube tocando, WhatsApp Web aceitando o navegador (os sites veem o Safari).
- [x] 23.2 Página direto da placa (Jangada 2.0.0). A página é um widget do WebKitGTK dentro da
  janela, embaixo do desenho do glove-ui (o backend GTK do kit: as barras em texturas por cima,
  transparentes onde a página aparece, e o mouse passa por elas): a placa de vídeo desenha a página
  direto na tela, sem cópia pelo processador. Parada, a Jangada não gasta processador.
- [x] 23.3 Tudo da 1.x no motor novo: abas, sugestões, favoritos, histórico, sessões, downloads,
  buscar na página, PDF e impressão, alertas e permissões em janela própria, menu de contexto do
  Glove, janela privada (sessão efêmera do motor), senhas e extensões num mundo isolado, escudos
  (as mesmas listas convertidas para os filtros nativos do WebKit, sem script na página), Não
  Rastrear/GPC, modo escuro nas páginas, inspetor do WebKit, tela cheia, gestos do touchpad.
- [x] 23.4 (v0.14.0) Bordas pelo protocolo da KDE no compositor (o GTK pede por ele): as janelas do GTK que
  querem as do servidor ganham a barra do Glove (as Preferências da Jangada, outros apps GTK).
- [x] 23.5 (v0.14.0, Jangada 2.0.0) Pacotes: deb (3,7 MB em vez de 110 MB), rpm e Arch dependem do WebKitGTK, do GTK 4 e
  do GStreamer da distribuição. Slackware -current: o WebKitGTK compilado no CI vai junto no
  pacote (o Slackware não o tem). Slackware 15.0: fica na Jangada 1.15.1 (GTK 4 e compilador
  velhos demais para o WebKitGTK atual); o instalador escolhe sozinho.
- [x] 23.6 (v0.14.1, Jangada 2.0.1) A janela da Jangada com a sombra e os quatro cantos
  arredondados do Glove também fora do zoom. O GTK guarda em volta das janelas sem barra uma
  margem invisível (para redimensionar), e o Glove não as reconhecia como janelas sem moldura. Agora
  o app pede a moldura do Glove pelo canal dos menus (MOLDURA glove), e o arredondamento aceita a
  superfície que contém a janela. A janela privada se vê de longe (grafite e o selo Privada), e o
  endereço se edita: o primeiro clique escolhe o endereço inteiro, o segundo põe o cursor.
- [x] 23.7 (v0.14.1) A aparência no instalador (página Aparência, depois de Conta): vale para o
  sistema inteiro: janelas, ícones, tela de login, abertura e desligamento. A regra dos ícones é
  uma só (`glove_config::icones_da_aparencia`): Mide no Moderno Claro, Mide Escuro no Moderno Escuro
  e os do Glove nas clássicas. Nas Preferências, com "Usar a aparência em todo o sistema", trocar
  a aparência troca os ícones, e a tela de login ganha os mesmos ícones.
- [x] 23.8 (v0.14.2, Jangada 2.0.2) A Jangada nítida em telas HiDPI (1,25, 1,5, 2). As barras
  borravam em escala fracionária: o backend GTK do glove-ui mandava o desenho pelo nó de
  textura com filtro, que o GTK reamostra. Com o nó de textura simples, cada pixel do desenho
  cai num pixel da tela: conferido pixel a pixel (diferença 0) em 1,25, 1,5 e 2, no GTK 4.14 e
  no 4.18. Para testar escalas numa tela comum: `GLOVE_ESCALA=1.5 glove --winit`.
- [x] 23.9 (v0.14.3, Jangada 2.0.3) Revisão da Jangada 2 (estabilidade, compatibilidade, desempenho,
  HiDPI), cada falha reproduzida na sessão oculta antes e depois:
  - Quedas: `prompt()` e sair de uma página com aviso de saída (`beforeunload`: Gmail, editores,
    formulários) derrubavam a Jangada inteira (falha de segmentação: o diálogo era fechado dentro
    do sinal do motor, que o soltava de novo em seguida).
  - Abas: fechar uma aba aberta por outra (window.open, login em janelinha) matava o processo que
    as duas dividem, e a que abriu caía ("A página parou de responder"); `window.close()` não
    fechava a aba; fechar uma aba antes da escolhida trocava a escolhida. A aba comum continua
    soltando o processo na hora (a memória volta já).
  - Downloads: o que o motor não sabe mostrar e não vem marcado como anexo (`.zip`, `.deb`,
    `application/octet-stream`) era ignorado sem aviso; sem a pasta Downloads todo download
    falhava como "Cancelado" (agora a pasta é criada, e a falha diz "Falhou").
  - Barra: carregar pelo endereço, por uma sugestão ou por um favorito não mostrava a faixa azul
    nem o botão de parar, e um endereço inválido não mostrava a página de erro (os avisos do
    motor chegavam com a página ainda emprestada e se perdiam).
  - Histórico: regravado inteiro a cada troca de título (um site que pisca o título: 167
    gravações de 219 KB em 10 s; agora 8, em lote).
  - Menu do botão direito: abria no canto da página com o mouse parado sobre uma aba nova; e
    "Salvar Página como PDF" abria Imprimir.
  - Janela: abria maior que a tela em 1366 × 768 ou com a tela em 150% (agora cabe na menor tela).
  - Senhas: o chaveiro é consultado fora da linha principal (trancado, congelava a Jangada).
  - Extensões: com dois scripts na página, só o primeiro recebia as respostas do armazenamento;
    um erro de escrita num script derrubava os outros; instalar de novo com o manifesto mudado
    deixava duas cópias; o pedido de armazenamento aceitava caminhos no lugar do nome. A API
    atende por `jangada` (e por `chrome`), com `storage.local.clear`.
  - Documentação: o README dizia "extensões no formato do Chrome" e prometia `run_at`; agora diz
    que é um formato próprio (não roda as da loja do Chrome), com o guia `docs/EXTENSOES.md` e o
    exemplo mínimo `exemplos/ola/` em JavaScript; e lista o que ainda não funciona.
- [x] 23.10 (v0.14.4, Jangada 2.0.4) O que tinha ficado da revisão, conferido na sessão oculta:
  - Abas abertas por trás carregam com a página no tamanho certo (era `innerWidth` 0): as abas
    escondidas saem da tela mas ficam com o lugar marcado no `NativeHost`.
  - Os scripts das extensões e o das senhas entram quando o documento fica pronto (um script de
    usuário do motor avisa a Jangada), sem esperar a página acabar de carregar.
  - `prompt()` numa janela própria com o campo do texto (Esc e fechar a janela cancelam; fechar a
    aba com a pergunta aberta não derruba nada).
  - Ler a área de transferência por script: a pergunta numa faixa no alto da página (Negar, Só
    desta vez, Permitir). Prender o mouse: a Jangada permite (o aviso "Esc solta" sai quando
    prende de fato).
  - A faixa de abas rola quando não cabem todas (setas nas pontas, a roda, acompanha a escolhida).
  - Reabrir as abas da última vez: cada uma carrega ao ser escolhida, já com o título guardado
    (16 abas reabertas: 1 processo de página em vez de 16).
  - As opções da linha de comando da Jangada em inglês (`--install`, `--extensions`, `--remove`,
    `--private`, `--debug`, `--yes`, com `--help` e `--version`); as de antes continuam aceitas.
  - CI do Slackware: o Rust do sistema serve (o script lia `~/.cargo/env`, que só existe com o
    rustup, e o pacote falhava depois de três horas compilando o motor); o motor vai para o cache
    assim que existe, mesmo que um passo de depois falhe.
- [ ] 23.11 Pendências da Jangada:
  - (M) Prender o mouse no Glove: o compositor ainda não oferece os protocolos de travar o
    ponteiro e de movimento relativo (pointer-constraints e relative-pointer); sem eles o pedido
    da página falha, mesmo com a Jangada permitindo.
  - (P) Honrar `run_at: document_start` nas extensões (hoje todas entram com o documento pronto).
  - (G) Chamadas de vídeo: o WebKitGTK do Debian 13 vem sem WebRTC (e sem DRM). Só muda com o
    motor compilado à parte, como já é no Slackware.
  - (motor) Em 125% e 150% o WebKitGTK desenha a página em 200% e o GTK reduz: acompanhar a
    escala fracionária no motor.
  - (P) O desenho do kit ocupa a janela inteira (transparente sobre a página): em 4K são dezenas
    de MB; limitar às barras.
  - (P) O kit ainda chama `glove-senha --campos` e `glove-prefs impressao` pelos nomes de antes
    (que continuam aceitos): trocar na próxima versão do kit.
  - (decisão) Os nomes dos próprios programas em português (`glove-senha`, `glove-chaves`,
    `glove-sair`, `glove-arquivo`, `glove-instalador`, `glove-escolher`… uns 25): passar para o
    inglês mexe nos pacotes, nos atalhos, no Dock de quem já usa e nos identificadores das
    janelas; só com a decisão do usuário, e com atalhos de compatibilidade.
- [x] 23.12 (v0.14.5) A linha de comando em inglês em todas as ferramentas do Glove (regra do
  projeto: comandos e opções sempre em inglês). Os nomes de antes, em português, continuam aceitos
  em silêncio (atalhos e scripts antigos), e as chamadas entre os componentes já usam os novos.
  - `glove`: `--session`, `--window`, `--displays` (eram `--sessao`, `--janela`, `--monitores`).
  - `glove-files`: `--open`, `--transfer move|copy`, `--view-options`, `--desktop-view-options`,
    `--preferences`, `--connect [--now]`, `--eject`, `--eject-warning`, `--duplicate`, `--trash`,
    `--empty-trash`, `--show`.
  - `glove-prefs`: `--join`, `--install-language`, `--system-theme`, `--check-updates`, e os
    painéis pelo nome em inglês (`glove-prefs network`, `sound`, `displays`, `date-time`,
    `login-items`, `software-update`…; o nome de dentro, guardado nos favoritos, não mudou).
  - `glove-sair shutdown|restart|logout|quit` (e `cancelled`, `failed`); `glove-arquivo
    compress|expand`; `glove-notes new`; `glove-escolher open|save|folder --title --name --in
    --types`; `glove-senha --ask|--fields`; `glove-chaves --count`; `glove-disks --write`;
    `glove-instalador --dry-run`; `glove-login --theme`; `glove-view --draw`; `glove-arte
    icons|preview`.
  - `glove-setup-tool.sh`: os textos em português também mostram os comandos em inglês
    (`install`, `uninstall`, `deps`, `all`, `iso test`, `login remove`, `grub remove`, `help`).

## Fase 24 — Tudo em inglês (decisão de 24/11/2026)

Decisão do usuário: todo nome do projeto fica em inglês (programas, identificadores das janelas,
serviços, variáveis de ambiente, arquivos de dados e o código por dentro), sem manter os nomes
antigos ("corte limpo"); a interface continua traduzida e segue o idioma do sistema. São cerca de
104 mil linhas de Rust (72 mil no Glove, 25 mil no kit, 6 mil na Jangada), então vai em etapas,
cada uma compilada e conferida na sessão oculta antes da seguinte.

Os nomes novos dos programas (todos com `glove-`): agenda→calendar, ajuda→help, arquivo→archive,
arte→art, bateria→battery, boasvindas→welcome, central→appcenter, chaves→keychain,
contatos→contacts, copias→backup, correio→mail, dicionario→dictionary, escolher→chooser,
fontes→fonts, fotos→photos, gravacao→screenrec, gravador→voicerec, instalador→installer,
mensagens→messages, musica→music, navegador→browser, registros→console, sair→logout, senha→auth,
utilrede→netutil.

- [x] 24.1 (v0.15.0, Jangada 2.1.0) Linha de comando sem os apelidos em português (os que a
  0.14.5 e a Jangada 2.0.4 ainda aceitavam). Os painéis do `glove-prefs` têm só o nome em inglês.
- [x] 24.2 (v0.15.0, glove-ui 0.2.70, Jangada 2.1.0) Programas: os 25 executáveis, os pacotes e
  as pastas dos crates deles (`crates/senha` → `crates/auth`…) e os identificadores das janelas
  (`glove.Sair` → `glove.Logout`…, a Jangada é `glove.Browser`). O que a configuração de quem já
  usa guardou com os nomes de antes (os apps do Dock, os painéis favoritos) passa para os de agora
  ao ser lida (`Config::rename_old_names`).
- [x] 24.3 (v0.15.0) Serviços e variáveis: `glove-battery.service` (a pós-instalação desliga o
  `glove-bateria` que uma versão anterior deixou ligado), `GLOVE_SCALE`, `GLOVE_AUTH_*`,
  `GLOVE_NO_X11`, `GLOVE_TEST*`, `GLOVE_USE_PACKAGES`, `GLOVE_NO_REPOSITORY`, `JANGADA_NO_GPU`…
- [ ] 24.4 Arquivos de dados e chaves de configuração, com migração do que já existe no
  computador de quem usa.
  - [x] Jangada 2.1.0: `settings.conf` (chaves e valores em inglês), `bookmarks.tsv`,
    `history.tsv`, `session.tsv`, `permissions.tsv`, `passwords-never.txt`, `shields-off.txt`,
    as pastas `extensions/` e `shields/`.
  - [ ] Glove: os valores em português do `config.toml`, `sessao.log` e os arquivos de dados de
    cada app. Levantamento de 24/11/2026 (uns 215 pontos no código; os valores novos já
    escolhidos, com migração ao ler a configuração, em `Config::rename_old_names`):
    - Dock: `position` embaixo/esquerda/direita → bottom/left/right; `minimize_effect` escala →
      scale (genio → genie).
    - Monitores: `arrangement` estender/espelhar/um → extend/mirror/single; `side`
      direita/esquerda/abaixo → right/left/below.
    - Papel de parede: ondas/aurora/liso → waves/aurora/plain; `cor:R,G,B` → `color:R,G,B`;
      `wallpaper_mode` igual/por_tela/panorama → same/per-display/panorama; `wallpaper_fit`
      preencher/ajustar/esticar/centralizar/lado_a_lado → fill/fit/stretch/center/tile.
    - Aparências (`theme`, e as telas do sistema): claro/listras/metal/grafite/moderno/
      moderno-escuro → light/pinstripe/metal/graphite/modern/modern-dark; "aparencia" →
      "appearance". Atenção: os mesmos nomes estão nas pastas do tema de abertura do Plymouth
      (`glove-moderno`, `glove-moderno-escuro`) e no que `glove-prefs --system-theme` grava no
      sistema: o pacote precisa trocar o tema padrão de quem já instalou.
    - Destaque: azul/grafite/prata/dourado/laranja/roxo/vermelho/verde →
      blue/graphite/silver/gold/orange/purple/red/green.
    - Protetor de tela: aurora/estrelas/bolhas/ondas/relogio → aurora/stars/bubbles/waves/clock.
    - Sons de alerta (hoje o nome mostrado é o próprio valor): Gota/Sino/Vidro/Bolha/Toque/Sopro
      → drop/bell/glass/bubble/tap/blow, com o nome mostrado vindo da tradução.
    - Cantos ativos: todas/app/mesa → all/app/desktop. Mídia e discos: abrir/nada/video →
      open/nothing/video. Mesa `arrange`: nome/data/tamanho/tipo → name/date/size/kind. Arquivos
      `new_window`: inicio → home. Terminal: basico → basic (conferir os outros temas).
    - A chave `navegador` → `browser`.
  - [ ] Pastas de saída e scripts: `pacotes/`, e na Jangada `preparar.sh`, `empacotar*.sh`,
    `exemplos/`, o caminho do motor no Slackware (`/usr/lib/jangada/motor`).
- [ ] 24.5 Código por dentro: primeiro a API pública do kit, depois cada crate (módulos,
  funções, campos, comentários). O que nasce novo já nasce em inglês.
- [ ] 24.6 Textos de interface: o texto de origem passa a ser o inglês (um `pt.po` no lugar do
  `en.po`), sem mudar o que a pessoa vê.

Atenção (decisão do usuário, "corte limpo mesmo"): depois de atualizar para a 0.15.0, a sessão
que já estava aberta ainda roda o compositor antigo, que chama os programas pelos nomes de antes.
Até sair e entrar de novo, o menu Reiniciar / Desligar / Encerrar Sessão não abre e abrir arquivos
pela mesa não funciona: reinicie pelo terminal (`systemctl reboot`) ou saia com Ctrl+Alt+Backspace.
