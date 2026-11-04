# 4 — Início e fim da sessão: login, splash, desligar, bloqueio e utilitários de sistema

Escopo: tudo o que acontece **antes** do desktop aparecer e **depois** que o usuário pede para sair, mais os utilitários ligados à sessão. Regra fixa: nada de arte, logotipo ou código da Apple. Onde a referência mostra a maçã, o Glove usa a **gota de gel azul** (`glove_paint::icons::logo`). Onde mostra o "X" de metal escovado, usa uma **gota grande translúcida** como marca d'água.

Legenda: prioridade **Alta/Média/Baixa** · esforço **P** (≤1 dia), **M** (2–5 dias), **G** (>1 semana).

---

## 0. Estado atual do Glove (base para tudo abaixo)

- A sessão já é própria: `data/glove.desktop` → `data/glove-session` → `glove --sessao` (DRM/KMS + libinput via libseat, `crates/compositor/src/udev.rs`). Ela é aberta por **GDM/LightDM**, então hoje o Glove não tem tela de login.
- No menu principal (`shell.rs:590`, `open_main_menu`) há **"Encerrar Sessão…"**, que chama `loop_signal.stop()` **na hora**: não pergunta nada, não pede aos apps para fechar e não oferece Reiniciar, Desligar nem Repouso.
- Não existe bloqueio de tela, protetor de tela, controle de ociosidade (idle-notify/idle-inhibit), Forçar Encerrar, splash nem assistente de primeiro uso.
- O `glove-monitor` (screenshot `glove-monitor.png`) já tem a lista de processos e encerrar/forçar (`crates/monitor/src/main.rs:134`).
- Peças reaproveitáveis: `gel_circle`/`gel_rect`, `pinstripes`, `brushed_metal` e `Canvas::shadow` (`crates/paint/src/draw.rs`); `text_field`, `default_button`, `gel_button`, `tabs`, `slider` e `checkbox` (`crates/kit/src/ui.rs`); papel de parede "Ondas" (`paint/wallpaper.rs`).

---

## 1. Tela de login

**(a) Referências.** `panther/startupshutdown-login_-1-1.png`: sobre o papel de parede azul (o mesmo da mesa), um painel centralizado de ~470×395 px, branco-gelo e com listras finas, cantos retos e sombra suave embaixo. No topo fica o logotipo prateado (~64 px), o nome do sistema em negrito ~26 px e, logo abaixo, "Computador de Fulano" em cinza ~12 px. Mais abaixo vem uma **caixa rebaixada** (cinza mais escuro, borda de 1 px, cantos de ~4 px) com a lista de usuários: foto de 48 px com moldura branca e o nome em 15 px ao lado. O último item é "Outro…", com uma silhueta. Embaixo, três botões redondos de ~30 px com contorno cinza e ícone em traço, e o rótulo abaixo: **Repouso** (círculo com traço), **Reiniciar** (triângulo) e **Desligar** (símbolo de energia).
`-1-2.png`: depois de escolher o usuário, a lista dá lugar a um único usuário e ao campo "Senha:", que tem anel de foco azul. Os botões de baixo passam a ser **Voltar** (à esquerda) e **Entrar** (à direita, botão azul de gel com uma casinha).
`-1-3.png`: no lugar do campo aparece uma **barra de progresso indeterminada listrada em azul** com "Entrando…".
Em senha errada, o painel real **balança na horizontal** (comportamento conhecido do loginwindow).
`jaguar/…Accounts-scaled.png` e `cheetah/…Login-scaled.png` mostram a configuração: as abas Usuários / Opções de Login, "Entrar automaticamente como…" e a lista com Imagem/Nome/Tipo.

**(b) Glove.** Não tem. Quem mostra o login é o GDM/LightDM, com a aparência deles.

**(c) Lacuna.** Falta o programa inteiro: o greeter, a integração com o gerenciador de login, a lista de usuários com fotos, os botões de energia e a tela de preferências de contas.

**(d) Tarefas.** Ficam na seção 2, que é a recomendação de arquitetura.

---

## 2. Arquitetura do login: recomendação

### Opções avaliadas

| | (1) **greetd + greeter próprio no compositor Glove** | (2) Greeter do LightDM | (3) Gerenciador de login próprio (PAM + logind) |
|---|---|---|---|
| Quem faz PAM, logind, VT e utmp | greetd (Rust, ~1 MB de RAM, maduro, usado pelo tuigreet/ReGreet) | LightDM | **Nós** |
| Ligação com o greeter | Socket Unix, JSON com prefixo de tamanho (4 tipos de mensagem) | `liblightdm-gobject` (GLib/GObject, FFI em C) | — |
| Wayland | Nativo: o greeter é um compositor qualquer | O LightDM ainda é centrado em X11; greeter Wayland é precário | Nativo |
| Memória na tela de login | greetd ~1 MB + `glove --greeter` ~17 MB + `glove-login` ~4 MB ≈ **22 MB** | LightDM ~10 MB + Xorg (~40 MB) + GLib | ~20 MB |
| Segurança | O greeter roda como o usuário sem privilégios `greeter`; só o greetd é root | Boa, mas com mais código em C | **Ruim**: nós escreveríamos o processo root (troca de UID, initgroups, sessão PAM, abertura de VT, limpeza de ambiente). É fácil errar |
| Esforço | M | G (bindings GObject) | G+ e manutenção eterna |
| Contras | Não vem instalado no Ubuntu e no Fedora (é preciso trocar de DM); troca rápida de usuário fraca (só um greeter, numa VT) | Visual preso ao modelo do LightDM, dependência pesada | Reinventar o gdm/greetd |

**Recomendação: opção (1), greetd + `glove --greeter` + o novo app `glove-login`.** É a mais leve (bate com a meta de 500 MB), é toda em Rust, tem o menor código privilegiado e reaproveita o compositor e o glove-kit. O greeter tem o mesmo visual do desktop e não precisa de nenhuma lib nova pesada. Continuamos aceitando GDM e LightDM: `glove.desktop` segue em `wayland-sessions`, então o greetd é opcional e recomendado.

### Como funciona

```
systemd → greetd (root) ──PAM "greetd-greeter"──> usuário "greeter", logind seat0, VT1
            │                                     └─ glove --greeter   (quiosque: sem Dock, sem menu, sem atalhos)
            │                                           └─ glove-login (app kit, painel central)
            │  $GREETD_SOCK  ◄── create_session{username}
            │                ──► auth_message{type: secret, "Senha:"}
            │                ◄── post_auth_message_response{response}
            │                ──► success | error{auth_error}
            │                ◄── start_session{cmd:["glove-session"], env:[...]}
            └─ ao sair o greeter: PAM "greetd" do usuário → glove-session (já existe)
```

### Plano passo a passo

1. **Modo quiosque do compositor** (`crates/compositor/src/main.rs`, `state.rs`, `shell.rs`, `input.rs`). Criar a flag `--greeter`: desenha só o papel de parede, sem barra de menu, sem Dock e sem ícone de disco. Atalhos desligados, exceto troca de VT. A janela do primeiro cliente fica centralizada, sem barra de título e sempre focada. Abre `glove-login` e **termina quando o filho termina**. Lê `/etc/glove/login.toml`, não `~/.config`. — **Alta, M**
2. **Crate `crates/login` (bin `glove-login`)** sobre o glove-kit:
   - Lista de usuários: ler `/etc/passwd` com UID entre `UID_MIN` e `UID_MAX` de `/etc/login.defs` (padrão 1000–60000), descartando shells `nologin`/`false`. O nome vem do GECOS (antes da 1ª vírgula), com o login como alternativa.
   - Fotos: `/var/lib/AccountsService/icons/<usuário>`, que é legível para todos (o `~/.face` normalmente não é legível pelo greeter). Sem foto, usar um **avatar original desenhado em código** (gota, folha, bola, guitarra…), escolhido por hash do nome (`paint/icons.rs`: nova `avatar(seed)`).
   - Telas: lista → senha → "Entrando…" (barra listrada indeterminada nova em `kit/ui.rs`: `progress_indeterminate`) → `start_session`.
   - Em erro: `cancel_session`, balançar o painel (animação de 300 ms) e limpar o campo.
   - Item "Outro…": campos de nome e senha.
   - Botões redondos Repouso, Reiniciar e Desligar (novo `kit::round_icon_button`), configuráveis.
   — **Alta, M**
3. **Cliente IPC do greetd** (`crates/login/src/greetd.rs`): uns 150 linhas com `serde_json` e prefixo u32 em ordem nativa. A crate `greetd_ipc` existe, mas é fácil manter a nossa sem dependências. Modo `--fake` simula o servidor para testar dentro de uma janela (backend winit). — **Alta, P**
4. **Energia sem D-Bus pesado:** chamar `systemctl poweroff|reboot|suspend` (logind + polkit liberam isso para a sessão ativa local, inclusive a do greeter). Assim não é preciso puxar `zbus`. — **Alta, P**
5. **Instalação** (`data/greetd/config.toml`, `data/instalar-login.sh`): `[default_session] command="glove --greeter" user="greeter"`. O script instala o arquivo, cria `/etc/glove/login.toml`, mostra como trocar (`systemctl disable gdm && systemctl enable greetd`) e **explica como voltar**. Por segurança, nunca troca o DM sem confirmação. — **Alta, P**
6. **Entrada automática:** `[initial_session] user=… command="glove-session"` no greetd (equivale a "Entrar automaticamente como" em `jaguar/…Accounts`). — **Média, P**
7. **Preferências → Contas** (`crates/prefs/src/main.rs`, novo painel):
   - Aba **Usuários**: foto, nome e tipo (admin se estiver no grupo `sudo`/`wheel`). Trocar a foto grava `~/.face` e chama o AccountsService `SetIconFile` via `busctl`.
   - Aba **Opções de Login**: "Mostrar: lista de usuários / nome e senha", "Mostrar botões Repouso/Reiniciar/Desligar", entrada automática. Grava `/etc/glove/login.toml` com `pkexec`.
   - Aba **Itens de Início**: lista em `~/.config/glove/inicio` + XDG autostart (`cheetah/…Login-scaled.png`: "These items will open automatically when you log in").
   — **Média, M**

**Segurança:**
- A senha vive só num `String` que é zerado ao enviar (`zeroize` ou `fill(0)`) e nunca vai para log.
- O greeter não abre terminal nem nenhum app.
- `glove --greeter` ignora `config.toml` de usuário.
- O bloqueio após tentativas fica com o PAM (`pam_faillock`), não conosco.
- Para o chaveiro abrir sozinho, `/etc/pam.d/greetd` inclui `pam_gnome_keyring.so auto_start` (ver Chaveiro).
- O socket do greetd só é acessível ao usuário `greeter`.

---

## 3. Splash de boot ("Iniciando")

**(a) Referências.** `panther/startupshutdown-splash_-1-1.png` e `-1-2.png`: tela inteira **cinza claro uniforme (#BFBFBF)**, logotipo cinza-chumbo (~90 px) no centro e um **giratório de 12 raios** (~28 px) bem abaixo, a ~75 % da altura. `-2-1.png` a `-2-9.png` (Painel "Iniciando", 10.0–10.2): **fundo azul liso (#4269AE)** com painel listrado de ~470×360 px, logotipo prateado, o nome em negrito, caixa rebaixada com **barra de progresso determinada em gel azul** (~215 px) e o status em 12 px ("Welcome to Macintosh.", "Waiting for AMD server", …). O mesmo painel do login é reaproveitado.

**(b) Glove.** Não tem. Entre o kernel e o GDM aparece o splash da distribuição.

**(c) Lacuna.** Falta tema de boot e falta o painel "Iniciando sessão".

**(d) Plymouth × splash do compositor.** O compositor só existe depois que o DM sobe, então ele **não consegue** cobrir o boot. O caminho é usar os dois:
- **Tema Plymouth `glove`** (`data/plymouth/glove/glove.plymouth` + `glove.script`, plugin *script*): fundo #BFBFBF, gota cinza no centro e giratório animado por troca de 12 quadros. O mesmo tema cobre o **desligamento** (modo shutdown: fundo azul + giratório, como em `startupshutdown-shuttingdown_.png`). Como o Plymouth exige PNG, os PNGs são **gerados por código** com o novo bin `glove-arte` (`crates/paint/src/bin/arte.rs`), que usa `gel_circle` e um `spinner()` novo, no momento da instalação. Assim continuamos sem arte binária no repositório. — **Média, M**
- **Painel "Iniciando sessão…" no compositor:** na primeira moldura de `glove --sessao`, desenhar o painel de `splash_-2-x` (fundo com o papel de parede do usuário, gota, "Glove", barra de progresso e status: "Bem-vindo ao Glove.", "Carregando preferências…", "Abrindo o Dock…"). Some com fade de 200 ms quando os itens de início estiverem abertos. Também esconde o "piscar" da troca greeter → sessão. Implementar em `render.rs` + `shell.rs` (`SplashState`), reaproveitando o desenho do painel de `glove-login`, que vai para `paint/src/panel.rs`. — **Alta, P**
- **Giratório reutilizável** `paint::draw::spinner(c, cx, cy, r, fase)` (12 raios com alfa decrescente). É usado no Plymouth, no desligamento, no "Entrando…" e em cursores de espera. — **Alta, P**

---

## 4. Diálogos de Desligar, Reiniciar e Encerrar Sessão

**(a) Referências.** `panther/startupshutdown-shutdownwindow_-1-1.png` e `-2-1.png`: alerta de ~480×170 px com listras e cantos superiores arredondados, sem título. À esquerda, **ícone de 64 px: círculo cinza médio em gel com o símbolo em branco** (energia ou triângulo). Pergunta em negrito 13 px ("Tem certeza de que deseja desligar o computador agora?"). Texto menor com **contagem regressiva ao vivo**: "Se você não fizer nada, o sistema será desligado automaticamente em 116 segundos." Botões **Cancelar** (branco) e **Desligar** (padrão, azul pulsante) à direita. O alerta flutua sobre a mesa com sombra grande.

**(b) Glove.** "Encerrar Sessão…" mata o laço na hora. Apps com documento não salvo perdem dados, porque o Editor só pergunta quando recebe `close`.

**(c) Lacuna.** Faltam os diálogos, a contagem, o fechamento ordenado dos apps e as opções de energia.

**(d) Tarefas.**
- **Itens no menu principal** (`shell.rs::open_main_menu`, `MenuAction`): "Repouso", "Reiniciar…", "Desligar…", separador, "Encerrar Sessão de *Nome*…" (⇧⌃Q) e "Bloquear Tela" (⌃⌥L). No modo aninhado o item continua "Sair do Glove". — **Alta, P**
- **Alerta modal desenhado pelo compositor** (`crates/compositor/src/alert.rs`, novo): escurece levemente a mesa e mostra o painel com `pinstripes`, o ícone em `gel_circle` cinza e um botão padrão que pulsa (já existe a animação do gel em `kit`, que deve ser portada para `paint`). Contagem de 120 s com timer do calloop, Enter/Esc e foco preso. Desenhar no compositor (e não num app) garante que o diálogo apareça mesmo com apps travados. — **Alta, M**
- **Saída ordenada** (`shell.rs`): mandar `close` a todas as toplevels, esperar até 10 s. Se algum app recusar (por exemplo, o Editor com folha "Salvar alterações?"), cancelar e mostrar "O Editor cancelou o encerramento da sessão." Depois `loop_signal.stop()`. Para Reiniciar e Desligar, chamar `systemctl reboot|poweroff` em seguida. — **Alta, M**
- **Tela de desligamento** (`startupshutdown-shuttingdown_.png`): fundo azul liso + `spinner` a 75 % da altura enquanto os apps fecham. O Plymouth assume a partir daí. — **Média, P**
- **Botão de energia físico:** tratar `XF86PowerOff` em `input.rs` abrindo o alerta "Deseja reiniciar, repousar ou desligar?" (três botões). Para o logind não desligar direto, o compositor segura um *inhibitor* `handle-power-key` via `systemd-inhibit`. — **Média, P**

---

## 5. Bloqueio de tela, protetor de tela e repouso

**(a) Referências.** `panther/settings-screensaver_-1-1.png` e `-1-2.png`: aba "Protetor de Tela" com lista à esquerda (ícones em espiral azul), **pré-visualização ao vivo** em fundo preto (fitas luminosas tipo "Flurry"), botões Opções/Testar, slider "Iniciar protetor" de 3, 5, 15, 30 min, 1 h, 2 h ou Nunca, "Usar protetor aleatório" e "Cantos Ativos…". No 10.3, ao sair do protetor com senha exigida, aparece um painel pequeno no estilo do login (foto, nome, campo Senha, Cancelar/OK).

**(b) Glove.** Nada: sem detecção de ociosidade, sem DPMS e sem bloqueio.

**(c) Lacuna.** Tudo, e isso é crítico em notebook.

**(d) Tarefas.**
- **Ociosidade no compositor** (`state.rs`, `input.rs`): carimbar o último evento de entrada. Implementar `ext-idle-notify-v1` e `zwp-idle-inhibit-v1`, que o Smithay já oferece (`IdleNotifierState`, `IdleInhibitManagerState`), para players de vídeo segurarem a tela. Timers: protetor → apagar tela (DPMS via DRM `set_active(false)` em `udev.rs`) → repouso opcional. — **Alta, M**
- **Bloqueio interno ao compositor** (`crates/compositor/src/lock.rs`): enquanto bloqueado, **nenhuma superfície de cliente é desenhada** nem recebe entrada. O compositor mostra o papel de parede desfocado/escurecido + o painel de login (foto, nome, "Senha:", Trocar Usuário, OK). Fazer no próprio compositor (e não com um cliente `ext-session-lock`) evita a falha clássica: se o locker cai, a tela fica exposta. Também bloqueia antes do repouso, escutando `PrepareForSleep`… ou, mais simples, sendo o próprio Glove a chamar `systemctl suspend` depois de bloquear. — **Alta, M**
- **Verificação de senha:** pequeno binário `glove-senha` (crate `crates/senha`, FFI direto em `libpam`, uns 120 linhas, serviço PAM `glove-lock` = `@include common-auth`). Lê a senha de um pipe e devolve 0/1. Com `pam_unix` como usuário comum, quem confere é o `unix_chkpwd` setuid, sem privilégio nosso. Ficar fora do compositor isola a libpam e os módulos dela. — **Alta, P**
- **Protetores desenhados pelo compositor**, sem processo extra:
  - "Nome do Computador": gota + hostname flutuando em fundo escuro.
  - "Fluxo": fitas luminosas originais, com curvas Bézier aditivas em `tiny_skia` a 30 fps, desligado em máquina fraca.
  - "Ondas": o papel de parede animado.
  — **Média, M**
- **Preferências → Mesa e Protetor de Tela** (`prefs`): aba nova com lista, pré-visualização (renderizando o mesmo código), slider de tempo, "Pedir senha ao sair do protetor ou do repouso" e Cantos Ativos. — **Média, M**
- **`loginctl lock-session`:** responder ao sinal `Lock` do logind. Exige escuta D-Bus, então pode ficar para depois com `busctl monitor` em processo auxiliar, ou com `zbus` só se já estiver no projeto. — **Baixa, M**

---

## 6. Troca rápida de usuário (Panther)

**(a)** No 10.3, o nome do usuário à direita da barra de menu abre um menu com os outros usuários e "Janela de Início de Sessão…". A troca usa a transição de **cubo girando**.

**(b)** Não existe.

**(c/d)** O greetd só tem **um** greeter: depois que a sessão começa, ele não mostra outro login. Plano realista:
- Com **GDM/LightDM**: o menu do usuário na barra (`shell.rs`, extra à direita do relógio) chama `gdmflexiserver` ou `dm-tool switch-to-greeter`. A lista de sessões ativas vem de `loginctl list-sessions`, e clicar num usuário já logado faz `loginctl activate <id>`. Bloquear antes de trocar. — **Baixa, M**
- Com **greetd**: mostrar o menu só com "Bloquear Tela" e as sessões já ativas (troca por VT). Login novo em paralelo fica fora até o greetd ganhar multi-greeter.
- Transição de cubo: **não fazer**. É cara em GPU fraca; um fade de 150 ms basta.

---

## 7. Primeiro uso: assistente de configuração e instalador

**(a) Referências.**
- `panther/installation-welcome_.png`, `-licence_-1-1.png`, `-copying_-1-3.png`: janela de ~620×440 px com **lista de etapas à esquerda**, cada uma com uma bolinha de gel (azul na atual, cinza nas futuras, com o texto apagado). Atrás das etapas, um enorme "X" de metal como marca d'água. À direita, uma caixa branca com borda de 1 px e conteúdo (título 17 px, ícones, texto). Embaixo, **Voltar** e **Continuar**. Na licença: pop-up de idioma + texto rolável. Na cópia: "Preparando…", barra em gel azul, "Tempo restante: cerca de…".
- `panther/desktop-firstrun_.png`: o resultado do primeiro uso, com uma janela do Finder já aberta na pasta pessoal, o Dock completo e o disco na mesa.

**(b)** Nada. A instalação é `cargo build` + `instalar-sessao.sh`.

**(c)** Num DE para Linux, **instalar o SO é papel da distribuição**. Faz sentido ter um **assistente de primeiro uso** e, depois, um instalador do próprio Glove (pacote).

**(d) Tarefas.**
- **Crate `crates/assistente` (bin `glove-assistente`):** roda uma vez, quando falta `~/.config/glove/config.toml`. Usa o mesmo layout de etapas à esquerda (bolinhas `gel_circle`; marca d'água = **gota gigante translúcida**, não um "X"). Etapas:
  1. **Boas-vindas**: "Bem-vindo ao Glove", rodando a saudação em vários idiomas, à moda do vídeo de boas-vindas.
  2. **Teclado** (layout com teste ao vivo).
  3. **Data e hora** (fuso via `timedatectl`).
  4. **Aparência**: tema Listras/Metal/Grafite e papel de parede, com prévia.
  5. **Dock**: tamanho e ampliação.
  6. **Foto da conta**: avatares desenhados ou PNG; grava `~/.face`.
  7. **Pronto**: abre o Arquivos na pasta pessoal, como em `desktop-firstrun_.png`.

  Tudo grava `glove-config`. — **Média, M**
- **Componente `kit::Steps`** (lista de etapas + caixa + Voltar/Continuar) em `crates/kit/src/ui.rs`. Também serve ao instalador e a assistentes futuros (rede, impressora). — **Média, P**
- **Instalador do Glove** (`glove-instalador`, crate `crates/instalador`): Introdução → Leia-me → **Licença (GPL-3.0, texto real do projeto)** → Destino (`/usr/local` ou `~/.local`) → Instalando (barra + log) → Concluir. Copia os binários, o `.desktop`, o tema Plymouth e a config do greetd, com `pkexec` só para os passos de sistema. Faz sentido só quando houver releases binárias; até lá, um `make install` basta. — **Baixa, G**

---

## 8. Utilitários ligados à sessão

### 8.1 Forçar Encerrar (⌘⌥Esc)
**(a)** `jaguar/…Force-Quit-scaled.png` (e equivalentes do cheetah e do puma): janela pequena com listras e só o botão vermelho de fechar ativo. Texto de 2 linhas: "Se um aplicativo não responder por algum tempo, selecione o nome dele e clique em Forçar Encerrar". Lista com ícone de 16 px + nome, item travado em vermelho "(não está respondendo)". Rodapé: "Você pode abrir esta janela pressionando…" e botão padrão **Forçar Encerrar**. Se o app escolhido for o gerenciador de arquivos, o botão vira "Reiniciar".
**(b)** Só dentro do Monitor (Processo → Forçar encerramento).
**(c/d)** Fazer no **compositor** (`crates/compositor/src/forcequit.rs`) para que funcione mesmo com apps travados. A lista sai das janelas e dos PIDs que o compositor já conhece (`windows_of_pid`). "Não está respondendo" = não respondeu a `xdg_wm_base.ping` em 3 s (implementar o ping em `handlers`). Ação: SIGKILL, com confirmação "Deseja forçar o encerramento de X? Alterações não salvas serão perdidas." Atalho ⌃⌥Esc em `config` (`default_shortcuts`) e item no menu principal. — **Alta, M**

### 8.2 Monitor de Atividade / Process Viewer
**(a)** `jaguar/…Process-Viewer-scaled.png`: campo "Buscar", pop-up "Mostrar: Todos os processos / Meus processos", colunas Nome/Usuário/Estado/%CPU/%Memória, "36 processos", "Amostrar a cada [20] segundos" e "Mais informações" recolhível. `panther/system-managers-tasks_-1-1.png` a `-1-5.png`: barra de ferramentas (Exportar, Encerrar Processo, Inspecionar), filtro, Mostrar, e abaixo **abas CPU / Memória do Sistema / Atividade de Disco / Uso de Disco / Rede**, com legendas coloridas e um gráfico de barras verde/vermelho em fundo preto.
**(b)** O `glove-monitor` tem gráfico de CPU, memória e lista com PID/usuário/CPU/memória.
**(d)** Acrescentar campo de filtro, pop-up Meus/Todos/Do sistema, intervalo de amostragem, as abas inferiores Memória (pizza: livre/ativa/cache) e Disco/Rede (de `/proc/diskstats` e `/proc/net/dev`), e "Inspecionar" (pai, cmdline, arquivos abertos de `/proc/<pid>`). Arquivo: `crates/monitor/src/main.rs`. — **Média, M**

### 8.3 Ajuda (Help Viewer)
**(a)** `panther/system-features-help_.png`: janela de ~580×420 px com barra cinza contendo ◀▶, casinha e campo redondo "Faça uma pergunta". Página inicial dividida por uma linha vertical: à esquerda, o "X" grande de metal + "Ajuda do Mac OS" em 26 px cinza; à direita, links grandes em cinza ("Explorar a Ajuda", "Novidades", "Problemas comuns" com sub-links). `cheetah|puma|jaguar/…Help-scaled.png` e `…Help-Search-scaled.png`: lista de resultados com **barras de relevância**.
**(b)** Nada.
**(d)** Crate `crates/ajuda` (`glove-ajuda`), com páginas em texto marcado simples em `/usr/share/glove/ajuda/*.txt` (títulos, parágrafos, links `[[página]]`), histórico, busca por palavra com barra de relevância e item "Ajuda do Glove" no menu Ajuda de todo app (protocolo de menus já existente). Página inicial com a gota grande à esquerda. — **Média, M**

### 8.4 Console
**(a)** `jaguar/…Console-scaled.png`: janela com ícone de documento no título "console.log" e texto monoespaçado puro, começando pela versão do sistema.
**(d)** Crate `crates/console` (`glove-console`): mostra `~/.local/state/glove/sessao.log` (já gerado por `glove-session`) e `journalctl --user -f -o short`. Pop-up de origem, Limpar, Buscar. Reaproveita a fonte mono do terminal. Ajuda muito a depurar a sessão sem terminal. — **Média, P**

### 8.5 Perfil do Sistema
**(a)** `jaguar/…System-Profiler-scaled.png`: abas (Perfil do Sistema, Dispositivos e Volumes, Extensões, Aplicativos, Logs) e seções **recolhíveis com triângulo** ("Visão geral do software", "…do hardware", "…da memória", "…da rede"), com pares rótulo/valor alinhados e linhas finas separando.
**(d)** Crate `crates/perfil` (`glove-perfil`), aberto pelo botão "Mais informações…" no `glove-about`. Dados de `/etc/os-release`, `uname`, `/proc/cpuinfo`, `/proc/meminfo`, `/sys/class/dmi/id`, `/sys/class/net`, `lsblk -J` e `lspci`/`lsusb` se houver. Abas: Visão geral, Dispositivos, Discos, Rede, Logs. Precisa de `kit::disclosure` (triângulo). — **Média, M**

### 8.6 Chaveiro
**(a)** `jaguar/…Keychain-Access-scaled.png`: título "Chaveiro: fulano (desbloqueado)", barra com Senha/Nota/Apagar/Ir/**Cadeado** e Chaveiros, lista Nome/Tipo/Criado/Modificado e abas Atributos/Controle de Acesso.
**(d)** Não criar um cofre próprio. A ponte certa é **Secret Service**: usar `gnome-keyring` (ou `keepassxc`) desbloqueado no login pelo PAM (`pam_gnome_keyring` no `/etc/pam.d/greetd`). Um visualizador `glove-chaveiro` sobre D-Bus Secret Service exigiria `zbus`, que pesa. — **Baixa, M** (só a integração PAM: **Média, P**)

### 8.7 Utilitário de Rede e Conexão à Internet
**(a)** `jaguar/…Network-Utility-scaled.png`: abas Info, Netstat, Ping, Lookup, Traceroute, Whois, Finger e Port Scan. Info tem o pop-up da interface e duas caixas agrupadas ("Informações da interface": MAC, IP, velocidade, estado; "Estatísticas de transferência"). `puma/…Internet-Connect-scaled.png`: formulário de discagem (Configuração, Telefone, Senha, Status: Desconectando…).
**(d)** `glove-rede` (crate `crates/rede`) com as abas Info (de `/sys/class/net` + `ip -j addr`), Ping, Lookup (`getaddrinfo`), Traceroute (subprocesso `tracepath`) e Netstat (`ss -tunap`). Whois, Finger e Port Scan ficam de fora. — **Baixa, M**. "Conexão à Internet" vira o menu Wi-Fi na barra + NetworkManager (`nmcli`), que é de outra área; aqui fica só registrado. — **Baixa**

---

## 9. Ordem sugerida

1. **Alta, rápido** (≈1 semana): `spinner`; itens Reiniciar/Desligar/Repouso/Bloquear no menu principal; alerta com contagem + saída ordenada; painel "Iniciando sessão…".
2. **Alta, núcleo** (≈2–3 semanas): ociosidade + DPMS + bloqueio no compositor + `glove-senha`; Forçar Encerrar com ping.
3. **Alta, login** (≈2 semanas): `glove --greeter` + `glove-login` + IPC do greetd + `instalar-login.sh` + avatares desenhados.
4. **Média:** tema Plymouth gerado por `glove-arte`; Preferências Contas/Protetor; assistente de primeiro uso; Console; Perfil do Sistema; Ajuda; melhorias no Monitor.
5. **Baixa:** troca rápida de usuário, Utilitário de Rede, Chaveiro visual, instalador.

**Meta de memória:** tela de login < 25 MB no total (greetd + compositor em quiosque + `glove-login`); bloqueio e protetor sem nenhum processo novo residente (só `glove-senha`, que vive alguns milissegundos por tentativa).
