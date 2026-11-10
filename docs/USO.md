# Como usar o Glove — recursos novos (Fase 10)

*In English: [USAGE.md](USAGE.md)*

Este guia explica o que chegou na Fase 10 (v0.10.1 a v0.10.9): como abrir cada recurso, o que
ele faz e os atalhos. Os atalhos com **Super** usam a tecla do Windows; com o Glove aberto
numa janela dentro de outro desktop, a tecla **Alt** faz o papel do Super. Todos os atalhos
podem ser trocados em **Preferências ▸ Teclado ▸ Atalhos de teclado**.

## Todos os Apps (grade de apps)

Todos os programas instalados em tela cheia, em páginas, sobre o papel de parede desfocado. O Dock
continua na frente (clicar num app dele abre o app e fecha a grade).

- **Abrir:** o ícone do foguete no Dock (logo depois do Arquivos), a tecla **F4** ou um canto
  ativo (Preferências ▸ Exposé e Painel ▸ Cantos ativos ▸ Todos os Apps). No touchpad, **pinçar
  com quatro dedos**.
- **Procurar:** é só digitar; a busca mostra todos os apps, inclusive os que estão em pastas.
- **Escolher:** setas e Enter, ou clique.
- **Páginas:** roda do mouse, dois dedos no touchpad, Page Up/Down ou os pontinhos embaixo.
- **Pastas:** clicar numa pasta abre uma faixa embaixo dela com os apps. Clicar no nome (no canto
  da faixa) muda o nome. Clicar fora fecha a pasta. Os apps de sistema começam na pasta
  Utilitários.
- **Arrumar:** **segure** um ícone até todos balançarem (ou já comece a arrastar):
  - arraste para mudar o lugar; na borda da esquerda ou da direita, a página vira;
  - pare um instante **sobre outro app** e solte: os dois viram uma pasta, já aberta para você
    dar o nome;
  - solte sobre uma pasta para pôr o app nela;
  - arraste um app para fora da faixa para tirá-lo da pasta (pasta vazia some);
  - **Esc** ou um clique no vazio param o balanço.
  A arrumação fica salva (`~/.config/glove/grade.json`); apps instalados depois entram no fim (os
  utilitários, na pasta Utilitários).
- **Fechar:** Esc (primeiro limpa a busca, depois fecha a pasta e para o balanço), F4 de novo ou
  clique fora dos ícones.

## Mesas virtuais

Várias mesas, cada uma com as suas janelas. São 4 por padrão; o número muda em Preferências ▸
Exposé e Painel ▸ Mesas virtuais (1 desliga).

- **Trocar de mesa:** **Ctrl+Super+←** e **Ctrl+Super+→**. As janelas deslizam de lado e um aviso
  no meio da tela mostra em que mesa você está. No touchpad: **três dedos para os lados**.
- **Levar a janela junto:** acrescente **Shift** (Ctrl+Super+Shift+← ou →).
- **No Exposé (F9):** a faixa no alto mostra as mesas, com as janelas em miniatura.
  - Clique numa mesa para ir até ela.
  - Arraste uma janela até uma mesa para mandá-la para lá (ela encolhe sobre a faixa).
- **Encontrar uma janela:** clicar no app no Dock, escolhê-lo com Super+Tab ou escolher a
  janela no menu **Janela** leva até a mesa onde ela está. O menu Janela lista as janelas das
  outras mesas com o número da mesa.

Uma janela nova abre sempre na mesa em que você está. Janelas minimizadas e ocultas voltam na
mesa atual.

## Olhada Rápida (Arquivos)

Vê um arquivo sem abrir o programa dele.

- No Arquivos, escolha um item e aperte **Espaço** (ou Arquivo ▸ Olhada Rápida, Ctrl+Y).
- Mostra imagens (com as dimensões), a primeira página de PDFs, o começo de textos e código
  (role com a roda) e, para o resto, o ícone grande com o tipo, o tamanho e a data.
- As **setas** passam para o próximo item, tanto na janela da Olhada quanto no Arquivos.
- **Abrir** abre no app do tipo. **Espaço** ou **Esc** fecham.

A prévia de PDF usa o `pdftoppm` (pacote `poppler-utils`).

## Central de avisos

O Glove mostra as notificações dos apps: Firefox, Thunderbird, `notify-send` e os próprios apps
do Glove.

- Cada aviso aparece num **balão** no canto de cima à direita por 6 segundos. Os críticos, como
  bateria fraca, ficam até você clicar.
- **Clicar no balão** responde ao app e traz o app para a frente. O **×** dispensa o aviso.
- O **balãozinho na barra de menu** (à direita do relógio) mostra quantos avisos não foram lidos.
  Clique nele para abrir a central com os últimos 60.
- Na central, **Não perturbe** esconde os balões, menos os críticos. **Limpar** apaga a lista.
  Esc ou um clique fora fecha a central.

Para testar: `notify-send "Olá" "Um aviso de teste"` no Terminal.

## Tela cheia

- **Ctrl+Super+F** ou **Janela ▸ Entrar em Tela Cheia** põe a janela da frente no monitor
  inteiro, sem barra de título. A barra de menu e o Dock somem.
- Encoste o mouse no alto da tela para a barra de menu (e o Dock) aparecer.
- O mesmo atalho, ou Janela ▸ Sair da Tela Cheia, volta ao tamanho de antes.
- Os apps que pedem tela cheia sozinhos (F11 do Firefox, vídeos) também funcionam.

## Abas (Arquivos, Editor e Terminal)

A faixa de abas aparece quando há mais de uma aba. Cada aba tem o **×** para fechar, e o **+** no
fim abre outra.

| | Nova aba | Trocar de aba | Fechar a aba |
|---|---|---|---|
| Arquivos | Ctrl+T (na mesma pasta), ou "Abrir em Nova Aba" no menu de uma pasta | Ctrl+Page Up/Down | Ctrl+W |
| Editor | Ctrl+Shift+T (Abrir… também abre numa aba nova) | Ctrl+Page Up/Down | Ctrl+W (pergunta se há alterações) |
| Terminal | Ctrl+Shift+T (cada aba com o seu shell) | Ctrl+Page Up/Down ou Ctrl+Tab | Ctrl+Shift+W |

## Gestos do touchpad

Só na sessão Glove (não com o Glove numa janela dentro de outro desktop).

| Gesto | O que faz |
|---|---|
| 3 dedos para cima | Exposé (todas as janelas); de novo, volta |
| 3 dedos para baixo | As janelas do app da frente |
| 3 dedos para os lados | A mesa virtual vizinha (o conteúdo acompanha os dedos) |
| Pinçar com 4 dedos | Todos os Apps |
| Abrir os 4 dedos | Mostrar a mesa |

Os gestos ligam e desligam em Preferências ▸ Trackpad ▸ Vários dedos.

## Cópias de Segurança

Cópias automáticas da pasta pessoal num disco externo, com uma linha do tempo.

1. Abra **Cópias de Segurança** (Todos os Apps ▸ Utilitários, ou a Busca Rápida).
2. **Escolher Disco…** e escolha o disco externo (ou uma pasta).
3. **Fazer Cópia Agora** faz a primeira cópia. Com **Cópias automáticas a cada hora** marcado, o
   Glove faz uma por hora, sozinho, enquanto o disco estiver conectado, e avisa na central.

- Só o que mudou ocupa espaço: os arquivos iguais aos da cópia anterior viram links fixos.
- Ficam todas as cópias das últimas 24 horas, uma por dia no último mês e uma por semana antes
  disso. Com o disco quase cheio, as mais velhas saem.
- **Linha do tempo:** à esquerda, escolha a cópia. À direita, navegue pelas pastas (clique duplo
  entra, **‹ Acima** volta).
- **Restaurar** devolve o item escolhido ao lugar de origem. Se já existir um com o mesmo nome,
  ele volta com "(restaurado)" no nome, sem apagar nada.
- **Mostrar no Arquivos** abre a cópia no Arquivos.

As cópias ficam em `Glove Backups/<nome do computador>/<data>` no disco. Caches, a Lixeira e
pastas de programas (`.cache`, `node_modules`…) ficam de fora. A lista está em
`~/.config/glove/copias.toml`. O disco precisa aceitar links fixos (ext4, btrfs, xfs…); em
FAT/exFAT cada cópia ocupa o espaço inteiro.

## Central de Apps

Instalar e remover programas pelo apt (pacotes do sistema) e pelo Flatpak (Flathub).

- **Buscar:** digite o nome e aperte Enter. Aparecem os resultados do Flatpak e do apt.
- **Instalados:** os programas com janela do apt e os Flatpaks. Daqui dá para **Abrir** ou
  **Remover**.
- **Atualizações:** **Procurar Atualizações** baixa a lista nova (apt update) e **Atualizar
  Tudo** instala.
- Para o apt, a Central pede a **senha de administrador** numa folha da janela. A senha vai só
  para o `sudo`; nunca aparece na tela nem é gravada.
- O Flatpak instala na sua conta, sem senha. Sem nenhum remoto configurado, use **Ativar o
  Flathub**.

## Fotos

A pasta Imagens como biblioteca.

- As fotos aparecem em miniaturas, agrupadas por mês, as mais novas em cima. A barra embaixo
  muda o tamanho das miniaturas.
- Na lateral, **Fotos** mostra todas, e cada subpasta da pasta Imagens vira um **álbum**.
- **Clique duplo** (ou Enter) abre a foto grande:
  - as **setas** passam de foto;
  - **Espaço** liga e desliga a **Apresentação** (4 segundos por foto);
  - **Esc** ou **‹ Biblioteca** volta;
  - **Abrir no Visualizador** abre no Visualizador.
- Fotos ▸ Procurar Fotos Novas (Ctrl+R) relê a pasta.

Formatos: PNG, JPEG e WebP.

## Gravador

- O **botão vermelho** grava; o mesmo botão (quadrado) para. Gravação ▸ Gravar (Ctrl+R) faz o
  mesmo.
- Enquanto grava: o tempo, os LEDs de nível e a onda dos últimos segundos.
- As gravações ficam em **Música ▸ Gravações**, em WAV, com a data no nome.
- Na lista: **Ouvir/Parar** (ou clique duplo, ou Espaço), **Mover para a Lixeira** e **Mostrar no
  Arquivos**.

Grava pelo PipeWire (`pw-record`); sem ele, usa o `parecord` ou o `arecord`.

## Mensagens

Bate-papo no IRC.

1. Na primeira vez, aparece a folha da conta. O padrão é o **Libera.Chat** (aberto e gratuito),
   porta 6697, com TLS. Escolha um **apelido** e os **canais** (por exemplo `#debian`) e clique em
   **Conectar**.
2. Na lateral ficam o servidor (com as mensagens dele), os canais e as conversas particulares,
   com o número de mensagens não lidas.
3. Escreva embaixo e aperte Enter. As suas mensagens aparecem à direita, em azul; as dos outros,
   à esquerda.

- **+** abre um canal (`#nome`) ou uma conversa com alguém (o apelido).
- **Comandos:** `/join #canal`, `/part`, `/msg apelido texto`, `/me ação`, `/nick novo`.
- Mensagem particular ou com o seu apelido, com a janela no fundo, vira aviso na central.
- **Conta…** muda o servidor e o apelido. A conta fica em `~/.config/glove/mensagens.toml`.
  Senhas do NickServ ainda não são guardadas.

## Música

- **Biblioteca:** a pasta Música e as que você juntar (Música ▸ Preferências ▸ Adicionar Pasta…).
  Sem etiquetas no arquivo, o artista e o álbum vêm das pastas (Artista/Álbum/música).
- **Navegador:** Exibir ▸ Mostrar Navegador (Ctrl+B) põe Artistas e Álbuns acima da lista.
- **Listas:** o **+** do rodapé (ou Arquivo ▸ Nova Lista, Ctrl+N) cria uma lista e já pede o nome.
  Escolha uma música e use Arquivo ▸ Adicionar à Lista. Delete tira da lista. Clique duplo no nome
  renomeia. **Tocadas Recentemente** guarda as últimas 50.
- **Aleatório e Repetir:** os botões do rodapé (Repetir alterna entre nada, todas e uma) ou o menu
  Controles.
- **Aviso:** com a janela atrás, a música nova aparece na central de avisos (dá para desligar
  nas Preferências).
- Atalhos: Espaço toca e pausa, Ctrl+→ e Ctrl+← trocam de música, Ctrl+↑ e Ctrl+↓ mudam o volume.

## Vídeo

O mpv mostra a imagem; o painel do Vídeo controla. Formatos: mp4, mkv, webm, avi, mov, m4v, ogv,
mpg, wmv, flv, ts e 3gp (o mpv toca quase qualquer codec).

- **Abrir:** Arquivo ▸ Abrir… (Ctrl+O), **Abrir Endereço…** (Ctrl+L, um link ou caminho) e
  **Abrir Recente**.
- **Controles:** ← e → pulam 10 s, Shift+← e Shift+→ pulam 1 min, Home volta ao início. Ctrl+← e
  Ctrl+→ abrem o vídeo anterior e o próximo da pasta. **Velocidade** (de 0,5× a 2×, ou [ e ]),
  **Repetir**, volume (↑ e ↓) e **Sem Som** (M).
- **Vídeo:** Tela Cheia (Ctrl+F), Tamanho (Ctrl+0, 1 e 2), Proporção e **Capturar Quadro**
  (Ctrl+Shift+S, salva na pasta Imagens).
- **Áudio:** escolher a faixa (dublado, original, comentários) e ajustar o atraso.
- **Legendas:** as legendas do arquivo e as da pasta com nome parecido entram sozinhas. No menu:
  Mostrar Legendas (V), escolher a faixa, **Carregar Legenda…** (srt, ass, vtt…), atraso e tamanho.
- **Preferências** (Ctrl+,): continuar de onde parou, tocar o próximo da pasta ao terminar, usar a
  placa de vídeo e os idiomas preferidos de legenda e áudio (pt, en…).

## Editor de documentos (Fase 12)

O Editor de Texto virou um editor de documentos: dá para escrever livro, trabalho da
faculdade, formulário e relatório sem outro programa. Documento novo salva em **RTF**
(abre no Word, no LibreOffice e no TextEdit); `.txt` continua texto simples.

- **Barra de formatação** (em cima da régua): estilo do parágrafo, fonte (abre o painel de
  Fontes), tamanho, N/I/S, cor do texto, marca-texto, alinhamento, entrelinhas e lista.
- **Caracteres**: Formatar > Fonte (Negrito Ctrl+B, Itálico Ctrl+I, Sublinhado Ctrl+U,
  Tachado, Maior/Menor, Linha de Base, Mostrar Cores Ctrl+Shift+C, Marca-texto).
- **Parágrafos**: alinhamento (Ctrl+Shift+L/E/J/R); na régua, arraste a barrinha (primeira
  linha), os triângulos (recuos) e clique para pôr tabulações (arraste para fora para
  apagar); Formatar > Texto > Espaçamento… para entrelinhas, antes/depois e recuos em cm.
- **Estilos**: Corpo, Título, Capítulo, Seção, Subseção, Citação (barra ou Formatar >
  Estilo). Enter depois de um título volta ao corpo.
- **Listas**: marcadores ou numeradas (barra ou Formatar > Lista); Tab no começo do item
  aprofunda, Shift+Tab volta, Enter num item vazio encerra.
- **Zoom**: na barra do pé da janela, − e + e o menu de porcentagens (no modo página,
  também Ajustar à Largura); Ctrl + roda do mouse amplia em volta do ponteiro; Ctrl+Shift+.
  aumenta, Ctrl+Shift+, diminui e Ctrl+0 volta a 100%. Com a folha maior que a janela
  aparece a barra de rolagem de baixo. A barra também mostra a página e as palavras.
- **Páginas**: Visualizar > Modo Página (Ctrl+Shift+W) e Zoom; Arquivo > Configurar
  Página (Ctrl+Shift+P: papel, orientação, margens); Inserir > Quebra de Página (Ctrl+Enter).
- **Cabeçalho, rodapé e notas**: Formatar > Cabeçalho e Rodapé… (campos {página},
  {páginas}, {título}, {data}; primeira página diferente); Inserir > Número de Página;
  Inserir > Nota de Rodapé… (Ctrl+Alt+N; clique duplo na marca edita a nota).
- **Tabelas**: Inserir > Tabela…; Tab vai de célula em célula (na última cria uma linha);
  arraste a borda entre colunas; Formatar > Tabela para linhas, colunas, juntar/separar
  células, bordas e larguras iguais.
- **Imagens**: Inserir > Imagem… (PNG, JPEG ou WebP) ou arraste a imagem para dentro do
  documento. Ela fica na linha do texto (centralize o parágrafo para centralizar a imagem);
  clique nela para selecionar e puxe o canto para mudar o tamanho. Vai junto no RTF, no
  PDF e na impressão.
- **Sumário**: Inserir > Sumário (dos capítulos e seções, com as páginas) e Atualizar Sumário.
- **ABNT**: Arquivo > Novo com Modelo ABNT, ou Formatar > Aplicar Formatação ABNT (A4,
  margens 3/2 cm, Times 12, 1,5, recuo de 1,25 cm, número da página em cima à direita,
  capa sem número).
- **Imprimir e exportar**: Imprimir (Ctrl+P) sai com as folhas como na tela; Arquivo >
  Exportar como PDF (vetorial, com as fontes embutidas). O formato do Editor é o RTF.

## Computadores mais simples (Fase 13)

- Os apps não redesenham mais a janela só porque o mouse passou por cima: se nada
  muda na tela, não gastam nada. Num processador fraco era isso que mais pesava.
- Arrastar janelas não dá mais um tranco a cada segundo (a rede de segurança do
  compositor descartava o quadro em curso; correção de darkstarfire).
- **Preferências > Aparência > Reduzir efeitos**: sem animações (minimizar, Exposé,
  mesas, Todos os Apps), sem ampliação do Dock, sem o ícone pulando ao abrir, sem
  sombras nas janelas e sem o botão do alerta pulsando. Vale na hora, sem reiniciar.
- **Preferências > Contas > Itens de Início** (ou Monitor de Atividade > Processo >
  Itens de Início… / Serviços…): na aba **Aplicativos**, o que abre sozinho ao entrar e
  quanta memória cada um gasta agora; na aba **Serviços**, os serviços do sistema que
  costumam pesar (impressão, Bluetooth, AnyDesk, Docker, SSH, atualizações automáticas…),
  com estado e memória. Desmarcar desliga na hora e nas próximas vezes; os do sistema
  pedem a senha de administrador.
- Para achar o que ainda pesa: rode um app com `GLOVE_PROF=1` no terminal (mostra o
  tempo de cada quadro) ou a sessão inteira com `GLOVE_PROF=1`.

## Instalar o Glove pela ISO

1. **Montar a ISO** (no computador onde está o código): `./glove-setup-tool.sh iso`. Ela fica em
   `iso/glove-os-VERSÃO-amd64.iso`.
2. **Testar numa máquina virtual** antes de usar num computador de verdade:
   `./glove-setup-tool.sh iso testar` (precisa do QEMU: `sudo apt install qemu-system-x86 ovmf`). O
   disco virtual de 32 GB fica em `iso/disco-teste.qcow2`. GNOME Boxes e VirtualBox também abrem a ISO.
3. **Gravar num pendrive** (apaga o pendrive): `sudo dd if=iso/glove-os-VERSÃO-amd64.iso of=/dev/sdX
   bs=4M status=progress oflag=sync`, trocando `sdX` pelo pendrive (confira com `lsblk`).
4. **Ligar o computador pelo pendrive** (menu de boot do computador). O Glove ao vivo entra sozinho e
   abre **Instalar o Glove**. Dá para usar o Glove ao vivo antes de instalar; o instalador também está
   na Busca Rápida.

O instalador pergunta:

- **Idioma e teclado** do sistema instalado (com um campo para testar o teclado).
- **Fuso horário:** procure a cidade.
- **Disco:**
  - **Apagar o disco inteiro e instalar o Glove**: o aviso em vermelho mostra qual disco será apagado;
  - **Instalar ao lado do sistema atual**: usa o maior espaço livre do disco. Precisa de UEFI, de uma
    partição EFI e de 20 GB livres. O instalador não diminui partições: libere o espaço antes, no
    outro sistema (no Windows, "Gerenciamento de Disco ▸ Diminuir volume") ou com o Discos.
- **Conta:** nome, nome de usuário, senha e nome do computador. A conta é de administrador (`sudo`).
- **Resumo:** confira e marque "Entendi. Pode instalar".

A instalação mostra o andamento e o registro dos comandos. No fim, **Reiniciar**: tire o pendrive.
O sistema instalado já vem com o idioma, o teclado e o fuso escolhidos, sem o assistente de
boas-vindas.

Observações:

- **Secure Boot:** o GRUB instalado não é assinado. Desligue o Secure Boot no computador.
- O instalador só instala no Glove ao vivo. Em outro sistema, `glove-instalador --simular` mostra
  todas as páginas e os comandos que seriam executados, sem mexer em nada.

## Atalhos novos

| Atalho | O que faz |
|---|---|
| F4 | Todos os Apps |
| Ctrl+Super+← / → | Mesa virtual anterior / próxima |
| Ctrl+Super+Shift+← / → | Leva a janela para a mesa anterior / próxima |
| Ctrl+Super+F | Tela cheia |
| Espaço (Arquivos) | Olhada Rápida |
| Ctrl+T / Ctrl+Shift+T | Nova aba (Arquivos / Editor e Terminal) |
| Ctrl+Page Up / Page Down | Aba anterior / próxima |
