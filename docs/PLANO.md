# Plano do Glove

Mapa do que falta para o Glove ficar fiel ao Mac OS X 10.0–10.3 (Cheetah, Puma, Jaguar e
Panther) **no comportamento, nas proporções e na linguagem visual**, sempre com arte própria
desenhada por código (sem logotipos, ícones ou código da Apple).

O levantamento comparou 334 capturas de referência com o Glove atual. As análises
completas, com medidas, cores e arquivos de referência, estão em [`docs/analise/`](analise/):

| Análise | Área |
|---|---|
| [1-shell.md](analise/1-shell.md) | Barra de menu, menus, Dock, janelas, controles, alternador, Forçar Encerrar |
| [2-arquivos.md](analise/2-arquivos.md) | Arquivos (Finder), Abrir/Salvar, lixeira, cópia, Obter Informações |
| [3-preferencias.md](analise/3-preferencias.md) | Preferências: grade "Mostrar Todos" e cada painel, com o serviço Linux de cada um |
| [4-sessao-login.md](analise/4-sessao-login.md) | Tela de login, splash, desligar/reiniciar, bloqueio, primeiro uso, utilitários |
| [5-apps.md](analise/5-apps.md) | Apps existentes e apps que faltam |

As capturas de referência ficam em [`referencias/`](../referencias/) (com as fontes de cada uma). Origem:
use as páginas da [512pixels](https://512pixels.net/projects/aqua-screenshot-library/) (10.0–10.2) e da
[GUIdebook](https://guidebookgallery.org/screenshots/macosx103) (10.3).

## Como cada etapa é feita

1. Implementar a etapa (uma por vez, na ordem abaixo).
2. Compilar com `./compilar.sh` (apaga antes os restos de compilações antigas).
3. Testar: `cargo test --release` e capturas de tela comparadas com as referências.
4. Medir a memória quando a etapa mexer em processo residente (meta: máquina de 500 MB).
5. Marcar `[x]` aqui e fazer **um commit por etapa**, enviado ao GitHub.

Legenda de esforço: **P** pequeno (horas), **M** médio (1–2 dias), **G** grande (vários dias).

---

## Fase 0 — Feito

- [x] Sessão própria (DRM/KMS + libinput), Monitor/Mouse/Trackpad nas Preferências
- [x] 0.1 Dock: ampliação entra e sai suave (sem salto no primeiro movimento)
- [x] 0.2 `compilar.sh` limpa compilações antigas; referências baixadas

## Fase 1 — Acabamento visual rápido (o que mais salta aos olhos)

- [x] 1.1 Rótulo do Dock: texto branco em negrito com contorno escuro, sem pílula (P)
- [x] 1.2 Prateleira do Dock listrada e encostada na borda; ícones sem a sombra elíptica (P)
- [ ] 1.3 Papel de parede: tirar a faixa clara (emenda do degradê) (P)
- [ ] 1.4 Menus: listras opacas, cantos retos, sombra forte, itens de 19 pt, atalhos em 2 colunas (P)
- [ ] 1.5 Janela inativa: botões só com contorno e barra translúcida; botões de título desabilitáveis (P)
- [ ] 1.6 Calculadora com visor verde-claro e botões neutros, como a referência (P)
- [ ] 1.7 "Sobre o Glove" com memória, processador e versão (P)

## Fase 2 — glove-kit: os controles que faltam (destrava as outras fases)

- [ ] 2.1 Barra de rolagem em gel genérica (listas, tabelas, colunas) (M)
- [ ] 2.2 Barra de progresso com listras animadas (indeterminada) e giratório (M)
- [ ] 2.3 Alerta padrão com ícones originais de Pare/Cuidado/Nota e botão padrão pulsante (M)
- [ ] 2.4 Controle segmentado com glifos; barra de ferramentas com ícone + rótulo; campo de busca em cápsula (M)
- [ ] 2.5 Formulário com rótulos à direita (`form_row`), caixa com abas (`tab_box`), slider com marcas e polegar em gota (M)
- [ ] 2.6 Tabela com cabeçalho em gel, colunas ordenáveis e triângulos de expansão (M)
- [ ] 2.7 Triângulo de abrir/fechar seção (`disclosure`) e janela que muda de altura pelo app (P)

## Fase 3 — Menus e sessão (comportamento do sistema)

- [ ] 3.1 Submenus nos menus (▸), com navegação por teclado (M)
- [ ] 3.2 Menu principal da era: Sobre, Preferências, Dock ▸, Itens Recentes ▸, Forçar Encerrar, Repouso, Reiniciar, Desligar, Encerrar Sessão (M)
- [ ] 3.3 Desligar/Reiniciar/Encerrar com alerta e contagem de 120 s, apps avisados antes de sair, tela de desligando (M)
- [ ] 3.4 Menu do app completo (Sobre, Preferências, Ocultar Outros, Mostrar Todos, Encerrar) e menu Janela do compositor (M)
- [ ] 3.5 Forçar Encerrar desenhado pelo compositor, com detecção de "não responde" (ping) e cursor de espera original (M)
- [ ] 3.6 Extras na barra de menu: volume, bateria e monitor, com menus próprios; teclas de volume e brilho (M)
- [ ] 3.7 Ociosidade: apagar a tela (DPMS), bloqueio de tela no compositor (senha via PAM) e tampa do notebook (G)
- [ ] 3.8 Painel "Iniciando sessão…" com barra de progresso ao entrar (P)

## Fase 4 — Dock completo

- [ ] 4.1 Super+Tab percorre os apps no próprio Dock, com rótulo (10.0–10.2) (M)
- [ ] 4.2 Ícone pula ao abrir um app (M)
- [ ] 4.3 Animação de minimizar até o Dock (Escala) e miniatura viva da janela minimizada (G)
- [ ] 4.4 Menu de contexto do Dock com bico apontando para o ícone (Manter, Mostrar no Arquivos, Encerrar) (M)
- [ ] 4.5 Ocultar automaticamente, posição esquerda/direita, ampliação até 112 (M)

## Fase 5 — Tela de login e inicialização

Arquitetura escolhida (ver [4-sessao-login.md](analise/4-sessao-login.md)): **greetd** cuida de PAM e logind; o
Glove roda em modo quiosque (`glove --greeter`) e o app `glove-login` (glove-kit) conversa com o greetd.
Meta: tela de login inteira abaixo de 25 MB.

- [ ] 5.1 Modo quiosque do compositor (`--greeter`: sem Dock nem menus) (M)
- [ ] 5.2 Cliente do protocolo do greetd e `glove-login`: lista de usuários com fotos, senha, painel que balança no erro, Reiniciar/Desligar (G)
- [ ] 5.3 Avatares originais desenhados por código; `data/greetd/` + `instalar-login.sh` (com volta ao gerenciador anterior) (M)
- [ ] 5.4 Splash de boot e desligamento: tema Plymouth com arte gerada por código (`glove-arte`) (M)
- [ ] 5.5 Assistente de primeiro uso (teclado, data e hora, tema, foto) (M)

## Fase 6 — Arquivos

- [ ] 6.1 Barra de ferramentas do 10.2: Voltar/Avançar com rótulos, segmentado de visualização, Computador, Início, Aplicativos, Buscar (M)
- [ ] 6.2 Lixeira: Esvaziar com alerta, Devolver, ícone cheia/vazia no Dock (M)
- [ ] 6.3 Ícones de pastas especiais (Mesa, Documentos, Música, Imagens, Vídeos…), de tipos e emblema de atalho (M)
- [ ] 6.4 Copiar/colar/mover/arrastar com janela de progresso e alerta de substituição (G)
- [ ] 6.5 Renomear no lugar, menu de contexto, Duplicar, Criar atalho (M)
- [ ] 6.6 Obter Informações (Ctrl+I) com seções Geral, Nome, Abrir com, Permissões, Comentários (G)
- [ ] 6.7 Menu Ir completo, "Ir para a pasta…" com Tab, pasta Aplicativos/Utilitários (M)
- [ ] 6.8 Lista ordenável com expansão e datas "Hoje/Ontem"; status com seleção e "somente leitura" (M)
- [ ] 6.9 Diálogo Abrir/Salvar em colunas (`kit::filebrowser`), usado no Editor, Visualizador e Lembretes (G)
- [ ] 6.10 Modo Panther: barra lateral, metal escovado, engrenagem (M)
- [ ] 6.11 Atualização automática (inotify), seleção múltipla e retângulo elástico; pendrives na mesa (M)

## Fase 7 — Preferências

- [ ] 7.1 Janela "Mostrar Todos": grade em 4 categorias, barra com favoritos, altura muda por painel (M)
- [ ] 7.2 Refazer no layout da era: Aparência (destaque, duplo clique minimiza), Mesa, Dock, Teclado (+ aba Atalhos), Mouse, Trackpad, Monitores (M)
- [ ] 7.3 Som: volume, mudo, entrada/saída (PipeWire/PulseAudio) (M)
- [ ] 7.4 Economizador de Energia: repouso, tela, ações (logind, UPower) (M)
- [ ] 7.5 Rede e Wi-Fi (NetworkManager) (G)
- [ ] 7.6 Data e Hora: fuso, NTP (timedated), aba Relógio da barra (M)
- [ ] 7.7 Itens de Início (autostart XDG) e Menu de Entrada (vários layouts) (M)
- [ ] 7.8 Acesso Universal: cursor grande, zoom, cores invertidas (no compositor) (M)
- [ ] 7.9 Protetor de Tela com protetores originais (G)
- [ ] 7.10 Contas/Minha Conta (AccountsService), Internacional (idioma e formatos) (G)
- [ ] 7.11 Compartilhamento, Mídia Removível, Aplicativos Padrão, Bluetooth (M cada)
- [ ] 7.12 Baixa: Atualização de Software, Disco de Inicialização, Fala, Impressão
- Pular: ColorSync, QuickTime, Classic, .Mac/iDisk, AppleTalk, reconhecimento de fala

## Fase 8 — Apps

- [ ] 8.1 Navegador que caiba em 500 MB (NetSurf abaixo de 1 GB de RAM, Firefox acima) (P)
- [ ] 8.2 Lembretes com moldura própria (faixa colorida) e um processo só para todas as notas (M)
- [ ] 8.3 Editor: régua, alinhamento, Localizar/Substituir (M)
- [ ] 8.4 Visualizador: barra de ferramentas, miniaturas, PDF (M)
- [ ] 8.5 Monitor: ordenar, filtrar, abas CPU/Memória/Disco/Rede (M)
- [ ] 8.6 Busca: colunas Data/Tamanho/Tipo, painel de caminho; Discos em dois painéis (M)
- [ ] 8.7 Música (Rust puro, meta ~10 MB) (G)
- [ ] 8.8 Contatos (vCard) e Agenda (.ics) (G)
- [ ] 8.9 Painel de Fontes e Painel de Cores no kit (M)
- [ ] 8.10 Calculadora científica, Caracteres em modo Teclado, Captura de seleção (M)
- [ ] 8.11 Vídeo (mpv com controles próprios), Registros, Informações do Sistema, Ajuda (M cada)
- [ ] 8.12 Correio (por último) (G)

## Fase 9 — Sessão avançada

- [ ] 9.1 Vários monitores ao mesmo tempo (estender/espelhar) (G)
- [ ] 9.2 Apps X11 (XWayland) (M)
- [ ] 9.3 Troca rápida de usuário (Baixa)
