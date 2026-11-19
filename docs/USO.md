# Como usar o Glove — recursos novos (Fases 10 a 15)

*In English: [USAGE.md](USAGE.md)*

Este guia explica o que chegou da Fase 10 em diante (v0.10.1 até agora): como abrir cada recurso, o
que ele faz e os atalhos. Os atalhos com **Super** usam a tecla do Windows; com o Glove aberto
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
- Encoste o mouse no alto da tela: a barra de menu desce e o Dock sobe, deslizando, e a janela
  **se ajusta** ao espaço entre os dois (nada fica coberto, nem o topo nem o rodapé do app). Tirando
  o mouse de lá, eles saem e a janela volta a ocupar a tela inteira. A janela muda de tamanho uma
  vez só em cada sentido, não a cada quadro, para apps pesados (navegador, vídeo) não engasgarem.
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
- **Cabeçalho, rodapé, número de página e sumário** são elementos da folha, que você põe ou tira
  pelo menu **Inserir** (ficam marcados quando existem) e configura com **duplo clique** em cima
  deles:
  - **Cabeçalho…** e **Rodapé…**: texto à esquerda, no centro e à direita (campos {página},
    {páginas}, {título} e {data}), a letra, o tamanho, negrito e itálico, a cor, a distância da borda
    da folha, o recuo nas laterais, um fio separando do texto e a primeira página diferente (capa).
    **Remover** tira o elemento.
  - **Número de Página…**: no alto ou embaixo, à esquerda, no centro ou à direita, o formato ("1",
    "Página 1", "Página 1 de 3"…), o número da primeira folha e se aparece nela.
  - **Sumário…**: o título, até que nível entra (capítulos, seções, subseções) e se mostra as
    páginas; com ele no documento, a mesma janela atualiza ou remove.
- **Notas**: Inserir > Nota de Rodapé… (Ctrl+Alt+N; clique duplo na marca edita a nota).
- **Tabelas**: Inserir > Tabela…; Tab vai de célula em célula (na última cria uma linha);
  arraste a borda entre colunas; Formatar > Tabela para linhas, colunas, juntar/separar
  células, bordas e larguras iguais.
- **Imagens**: Inserir > Imagem… (PNG, JPEG ou WebP) ou arraste a imagem para dentro do
  documento. Ela fica na linha do texto (centralize o parágrafo para centralizar a imagem);
  clique nela para selecionar e puxe o canto para mudar o tamanho. Vai junto no RTF, no
  PDF e na impressão.
- **Sumário**: Inserir > Sumário… (dos capítulos e seções, com as páginas) e Atualizar Sumário.
- **ABNT**: Arquivo > Novo com Modelo ABNT, ou Formatar > Aplicar Formatação ABNT (A4,
  margens 3/2 cm, Times 12, 1,5, recuo de 1,25 cm, número da página em cima à direita,
  capa sem número).
- **Imprimir e exportar**: Imprimir (Ctrl+P) sai com as folhas como na tela; Arquivo >
  Exportar como PDF (vetorial, com as fontes embutidas). O formato do Editor é o RTF.

## Captura de tela com desenho

- **Temporizador para todos os modos:** em Captura ▸ Quando ("Agora", "Em 5 segundos", "Em 10
  segundos") vale também para "Uma área" e "Uma janela": dá tempo de abrir um menu ou deixar a tela
  como quer.
- **A tela congela:** quando a escolha da área (ou da janela) começa, o Glove fotografa a tela naquele
  instante e a mostra parada por baixo da película; a captura sai dessa imagem, mesmo que algo mude
  depois (um menu que fecha, um vídeo que anda).

- O app **Captura** já abre com **Uma área** escolhida (depois vêm Uma janela e Tela
  inteira): Enter ou Capturar, e arraste a área.
- Toda captura (pelo app ou pelos atalhos) já vai para a área de transferência: é só
  colar com Ctrl+V num chat, e-mail ou editor.
- A captura abre no **Visualizador** com a caneta na mão: escolha a **cor** e a
  **espessura do traço** na faixa de cima e desenhe por cima da imagem. Desfazer
  (Ctrl+Z), Limpar e **Salvar** (Ctrl+S), que grava por cima da captura e já copia a
  imagem com os desenhos. Ctrl+C copia como está na tela, mesmo sem salvar.
- **Formas** (na barra, ao lado de Desenhar): à mão livre, **linha**, **tracejado**, **seta**,
  **retângulo** e **elipse**, do ponto em que aperta até onde solta. Com **Shift**, o retângulo vira
  quadrado, a elipse vira círculo e as linhas ficam retas (na horizontal, na vertical ou a 45°).
- **Zoom** (também desenhando): os botões − e + da barra, ou **Ctrl + roda do mouse**, que aumenta e
  diminui em volta do ponteiro, sem deformar a imagem. Maior que a janela, aparecem as barras de
  rolagem; a roda rola (com Shift, para os lados) e arrastar fora do modo desenho move a imagem.
- Em qualquer imagem aberta no Visualizador: botão **Desenhar** (Ctrl+D). Fotos JPEG
  ou reduzidas ao abrir são salvas num arquivo novo ao lado, "nome (desenho).png".

## Abrir e Salvar

- Apps de fora (VS Code, navegador e outros que usam o portal do sistema) mostram a janela
  Abrir/Salvar do Glove. Ela pode ser redimensionada pelas bordas ou pelo canto inferior
  direito; quanto mais larga, mais colunas de pastas aparecem.
- Nos apps do Glove, Abrir/Salvar é uma folha que desce da barra de título. Arraste o canto
  inferior direito da folha para mudar o tamanho dela (até o tamanho da janela do app).

## Onde as janelas novas aparecem

- Toda janela nova abre no centro da área útil (entre a barra de menu e o Dock) do monitor
  principal, seja de app Wayland ou X11.
- Se o app muda o tamanho da janela logo depois de abrir (comum nos apps X11), ela volta para
  o centro com o tamanho novo. Depois que você move a janela, o Glove não mexe mais nela.
- Quando o app já tem uma janela no centro, a seguinte abre em cascata: um degrau para baixo
  e para a direita de cada janela do mesmo app que ainda está no lugar. Janela de outro app
  abre sempre no centro, e os diálogos também.
- Um app que lembra que estava em zoom abre em zoom, ocupando a área útil.

## Quando um monitor é desconectado

- As janelas que estavam no monitor que saiu vão para o que ficou, no centro dele.
- Se a janela era maior que o monitor que ficou, ela encolhe para caber: no máximo 90% da
  área útil (entre a barra de menu e o Dock).
- Janela em zoom (maximizada) continua em zoom, ocupando a área útil do monitor que ficou; ao
  sair do zoom, volta para um tamanho que cabe nele.
- Janela em tela cheia continua em tela cheia no monitor que ficou.
- As janelas do monitor que ficou continuam onde estavam nele (mesmo que ele passe a ser o
  principal). Vale também para as minimizadas, as ocultas e as de outras mesas.

## Redimensionar janelas

- Na borda de uma janela a seta do mouse vira a de duas pontas (↔, ↕ ou diagonal) e
  continua assim enquanto você arrasta. Perto dos cantos a borda redimensiona na
  diagonal, nas duas direções de uma vez.
- A janela das Preferências também pode ser aumentada, pelas bordas ou pelo canto inferior
  direito. O tamanho de cada painel é o mínimo: a janela nunca fica menor que ele. O que
  você aumentou continua valendo ao trocar de painel; se só alargou, a altura segue
  acompanhando cada painel. Mais larga, "Mostrar Todos" põe mais ícones por linha e a barra
  mostra mais favoritos; mais alta, os painéis longos aparecem sem rolagem. As listas e
  as caixas de abas de cada painel crescem junto, ocupando a janela toda.

## Papel de parede: vários monitores, panorama e animado

Em Preferências ▸ Mesa e Protetor de Tela ▸ Mesa:

- **Favoritas** (a primeira coleção): as imagens que você juntou, em grade. A última casa, **+
  Adicionar…**, abre a janela Abrir para buscar uma imagem (PNG, JPEG ou WebP) em qualquer pasta; ela
  entra nas Favoritas e vira o papel de parede. Nas outras coleções, clique numa imagem com o botão
  direito para juntá-la às Favoritas. Com o mouse sobre uma favorita, o **×** no canto a tira da
  coleção (o arquivo continua onde está).
- **Monitores** (aparece com mais de um monitor, no modo estender):
  - **A mesma imagem em todos:** cada monitor mostra o papel no tamanho dele;
  - **Uma imagem para cada monitor:** clique num monitor na miniatura e depois numa imagem;
  - **Uma imagem cobrindo todos (panorama):** uma imagem só atravessa os monitores, na posição
    real deles. Dois monitores 1920×1080 lado a lado formam 3840×1080: uma imagem desse tamanho
    aparece inteira, metade em cada um. Um em cima do outro: 1920×2160.
- **Ajuste:** Preencher a tela (corta o que sobra), Ajustar à tela (inteira, com faixas), Esticar,
  Centralizar e Lado a lado.
- **Vídeos e Animações:** vídeos (MP4, WebM, MKV, MOV…) e GIF das pastas Vídeos e Imagens viram
  papel animado (precisa do ffmpeg). Opções: quadros por segundo, **Leve** (metade da resolução,
  bom para computadores simples), **Parar na bateria** e **Parar com app em tela cheia**. Na primeira
  vez o Glove prepara, em segundo plano, uma cópia do vídeo já no tamanho da tela (em
  `~/.cache/glove/papel`, sem perda visível); com ela, o papel gasta cerca de 5 vezes menos. Ele também
  para com a tela bloqueada e com "Reduzir efeitos". O animado vale nos três modos (no panorama, um
  vídeo largo atravessa os monitores).
- A Pasta Imagens aceita PNG, JPEG e WebP.

## Central de controle

O ícone de dois botões na barra de menu (ao lado do relógio) abre a **central de controle**, como nos
Mac mais novos:

- **Wi-Fi** (com o nome da rede), **Bluetooth** e **Não perturbe**: clique para ligar ou desligar.
- **Tela** (brilho), **Luz do teclado** (só nos computadores que têm) e **Som**: arraste o trilho.
- A central abre na hora, com o que o Glove já sabe; o estado do Wi-Fi, do Bluetooth e do som é
  conferido em segundo plano e aparece logo em seguida. Se o serviço de Bluetooth do sistema
  (bluetoothd) não estiver rodando, o botão do Bluetooth fica apagado.

**Luz do teclado:** **Ctrl + teclas de brilho** aumentam e diminuem a luz do teclado (no Chromebook e
nos notebooks com teclado iluminado), com o aviso na tela; as teclas próprias de luz do teclado
também funcionam.

## Arquivos: nome inteiro, várias escolhas e "Mostrar na pasta"

- O Arquivos abre em **lista** (a visualização muda em Visualizar, ou nos botões da barra).
- **Nome inteiro:** parado sobre um nome cortado (na lista, nas colunas, nos ícones e nas janelas de
  Abrir e Salvar de todos os apps), aparece um balão com o nome todo.
- **Várias escolhas:** **Ctrl + clique** escolhe ou tira um item; **Shift + clique** escolhe a faixa.
- **"Mostrar na pasta"** dos outros apps (o ícone de pasta nos downloads do Firefox, o do VS Code…)
  abre o Arquivos na pasta, com o arquivo escolhido.

## AppImage

Um **AppImage** (programa num arquivo só) abre com **dois cliques** no Arquivos ou na mesa: aparece
uma janela com o ícone do programa e a pasta **Aplicativos**. **Arraste o ícone para a pasta** (ou
clique em Instalar) para copiá-lo para `~/Aplicativos`; ele passa a aparecer em Todos os Apps.
**Abrir sem Instalar** só roda o programa. Os AppImage antigos precisam da libfuse2, que já vem no
Glove OS (sem ela, o Glove os roda do mesmo jeito, abrindo o arquivo antes).

## Firefox com o visual do Glove

No Glove OS o Firefox já vem com o **tema do Glove**: metal escovado atrás das abas e da barra, campo
de endereço branco com o foco azul e os três botões da janela, como o navegador clássico do Mac. Para
voltar ao tema padrão, use Configurações › Extensões e Temas › Temas no Firefox.

## Discos e disco inicializável

O **Discos** (no Dock, em Utilitários) mostra à esquerda os discos (internos, pendrives, cartões e
imagens de disco em uso) e, à direita, o disco escolhido: modelo, tamanho, tabela (GPT ou MBR),
conexão e o **mapa dos volumes**, com as partições do tamanho proporcional e o **espaço livre**.
Clique num volume para ver o conteúdo (Ext4, FAT, NTFS…), o dispositivo, o tipo de partição, o UUID,
quanto do disco ele ocupa (%) e onde está montado. **Quanto cada volume tem ocupado** aparece numa
faixa embaixo dele no mapa e, nos detalhes, numa barra ("X usados (Y%), Z livres"). Volumes
montados são medidos sozinhos; nos desmontados, **Medir o Uso** monta só para leitura, em silêncio
(sem ícone na mesa), mede e desmonta. Montar, Desmontar, Abrir, Ejetar e **Verificar** (confere o
sistema de arquivos e, com erros, conserta) ficam embaixo do mapa e no menu Volume.

**Saúde do disco:** embaixo do nome do disco, "Saúde: Boa", "Boa, com avisos" (setores ruins) ou
"Falhando" (copie os arquivos já), com a temperatura. **Detalhes…** (ou Disco ▸ Saúde do Disco…)
mostra o tempo ligado, os setores ruins, o desgaste dos SSD NVMe e o último teste, e começa o
**Teste Rápido** do próprio disco (uns 2 minutos, o disco continua usável).

**Imagem do disco** (menu Disco): **Criar Imagem do Disco…** copia o disco inteiro para um arquivo
`.img` (cópia exata, para guardar); **Restaurar Imagem no Disco…** grava uma imagem de volta,
bloco a bloco (pede confirmação; apaga o disco). Os dois pedem a senha de administrador.

**Partições e formatação** (escolha o pedaço no mapa; os botões de baixo mudam conforme ele):

- **Formatar Disco…** (no alto): tabela nova **GPT** (computadores com UEFI, o recomendado),
  **MBR** (computadores antigos, TVs, aparelhos de som) ou nenhuma; opção de apagar com zeros.
  Apaga tudo o que está no disco.
- **Criar Partição…** (com um espaço **Livre** escolhido): tamanho em GB (o sugerido é o espaço
  todo), formato (**Ext4** para Linux e o Glove OS, **FAT32** para tudo com arquivos até 4 GB,
  **exFAT** para pendrives e cartões, **NTFS** para o Windows, **Btrfs**) e nome.
- **Formatar…**: apaga o volume e o refaz no formato escolhido, com nome.
- **Redimensionar…**: aumenta (até o fim do espaço livre logo depois) ou diminui o volume, junto com
  o sistema de arquivos; os arquivos ficam. Para diminuir, o volume é desmontado e conferido antes.
  É assim que se abre espaço para instalar o Glove OS ao lado do Windows: diminua a partição do
  Windows (NTFS) e o instalador usa o espaço livre. Faça antes uma cópia do que for importante.
- **Editar…**: o tipo da partição (Linux, Windows, FAT32, Sistema EFI, troca, inicialização da BIOS)
  e o nome do volume.
- **Apagar**: apaga a partição (pede confirmação); o espaço fica livre.

Os volumes do sistema em uso (`/`, `/boot`, `/home`…) não podem ser formatados, apagados nem
redimensionados. Mexer em discos internos pede a senha de administrador; em pendrives, não.

**Criar disco inicializável** (pendrive que liga o computador, em UEFI), como o Rufus:

1. Escolha o pendrive à esquerda e clique em **Disco Inicializável…** (ou Disco ▸ Criar Disco
   Inicializável…).
2. Escolha a imagem (`.iso` ou `.img`; a folha abre em Downloads).
3. Confira o aviso: ele diz qual disco será **apagado** (nome, dispositivo e tamanho). **Apagar e
   Gravar** começa; o rodapé mostra o andamento e tem **Cancelar**.

O Discos reconhece a imagem sozinho:

- **Linux** (Glove OS, Debian, Ubuntu, Fedora… as ISOs "híbridas"): gravada inteira, bloco a bloco,
  e conferida no fim (lida de volta e comparada). Inicia em UEFI e BIOS. Pede a senha de
  administrador (gravar o disco inteiro precisa dela).
- **Instalador do Windows** (10 e 11): o pendrive vira GPT com uma partição FAT32 e os arquivos da
  ISO. Se o `install.wim` passar de 4 GB (o limite do FAT32), ele é dividido em partes
  (`install.swm`…) pelo `wimtools`, que o instalador do Windows aceita. Inicia em computadores com
  UEFI, **inclusive com o Secure Boot ligado**, sem o WoeUSB. Computadores só com BIOS antiga não
  iniciam esse pendrive.

O disco do sistema em uso (com `/`, `/boot`, `/home`…) nunca aparece como opção para apagar. Para
scripts: `glove-disks --gravar imagem.iso /dev/sdX` faz o mesmo no terminal.

## Atualizações do Glove OS

O Glove anda no próprio ritmo, sem esperar o Debian: a cada versão nova, o GitHub publica o `glove` e
o `glove-os` num **repositório apt do Glove OS** (assinado), que o Glove OS já traz configurado
(`/etc/apt/sources.list.d/glove-os.sources`). O Debian 13 continua vindo do Debian.

- **Aviso:** de tempos em tempos (2 minutos depois de entrar e a cada 6 horas) o Glove confere se há
  versão nova. Se houver, aparece um aviso ("Glove OS 0.12.50 disponível") no canto da tela e no
  contador da barra de menu. **Ver** abre o painel. Cada versão avisa uma vez só.
- **Atualizações do sistema:** o Glove OS deixa o próprio sistema renovar a lista de pacotes todo dia
  (o `apt-daily` do Debian, sem instalar nada), e o Glove confere essa lista de hora em hora. Quando
  aparece algo novo, vem o aviso "3 atualizações do sistema disponíveis", com os nomes.
- O aviso fica na **central de avisos** (o balãozinho da barra de menu) mesmo depois que o balão some:
  clicar nele lá também abre a Atualização de Software.
- **Preferências ▸ Atualização de Software:** em cima, o **Glove OS**: a versão instalada, a nova, **o
  que há de novo** e o botão **Atualizar o Glove** (atualiza só o Glove, pelo repositório dele).
  Embaixo, as atualizações do sistema inteiro e **Instalar Tudo**. **Verificar Agora** consulta os
  repositórios de novo (o `apt update`). Tudo sem o Terminal: a senha de administrador é pedida numa
  folha da própria janela, e a instalação mostra uma **barra de progresso** com o que está sendo
  baixado e instalado. No fim, **Encerrar Sessão…** para entrar já no Glove novo.
- **Glove noutro Debian ou Ubuntu:** o painel oferece **Adicionar o Repositório do Glove**, e daí em
  diante o Glove se atualiza pelo apt também (sem a identidade do Glove OS, que só o pacote
  `glove-os` traz). Pelo Terminal, os comandos estão em https://fabioaacarneiro.github.io/glove.
- Pelo Terminal, também vale o de sempre: `sudo apt update && sudo apt upgrade`.
- **Glove no Fedora e no Arch:** o painel também oferece **Adicionar o Repositório do Glove** (uma
  vez) e depois **Atualizar o Glove**. Com o repositório, o Glove passa a chegar junto com as
  atualizações do sistema (`sudo dnf upgrade`, `sudo pacman -Syu`). As atualizações são conferidas
  pela chave do Glove, que vem no pacote.
- **Glove no Slackware:** não há repositório; **Atualizar o Glove** baixa o pacote da versão nova da
  página de releases, confere a soma (`SHA256SUMS`) e instala com o `upgradepkg`.
- **Glove instalado pelo código** (`./glove-setup-tool.sh instalar`): o painel avisa da versão nova e
  diz como atualizar (`git pull` e instalar de novo).
- Fora do apt, a atualização mostra um giratório no lugar da barra de progresso.

## Senha do sistema (pedidos de administrador)

Quando um programa precisa de permissão de administrador (formatar um disco no Discos, instalar pelo
Flatpak, mudar uma rede do sistema, `pkexec` no Terminal), abre a janela **Autenticação** com o que
foi pedido. Digite a sua senha e clique em **Autenticar**; **Cancelar** (ou Esc) nega o pedido. A
senha é conferida pelo próprio sistema (polkit e PAM); o Glove não a guarda. Contas que não são de
administrador veem o nome de um administrador e precisam da senha dele.

Os apps do Glove que precisam de administrador (Atualização de Software, Itens de Início ▸ Serviços,
Segurança, Central de Apps, Instalador) usam a mesma janela **Autenticação**, separada do app, em vez
de uma folha dentro da janela dele. A senha errada é avisada ali mesmo. As outras senhas também
abrem nela: a da rede Wi-Fi, trocar a senha da conta (Preferências ▸ Contas), trocar a senha do
Chaveiro, o Disco de Inicialização e o **Instalar Tudo** da Atualização de Software no Fedora, no Arch
e no openSUSE (antes esses dois abriam o Terminal para pedir a senha).

## Senha ao voltar do repouso e da hibernação

Com **Preferências ▸ Segurança ▸ Pedir a senha ao sair do repouso ou do protetor de tela**
ligado:

- A tela bloqueia antes de qualquer repouso ou hibernação do sistema, peça quem pedir: o
  menu Repouso, fechar a tampa, o botão de energia, a bateria no fim ou `systemctl
  hibernate`. Ao voltar, aparece o painel da senha, nunca a mesa.
- **Repouso** (menu principal): a tela de bloqueio aparece, o monitor apaga 3 segundos depois e
  então o computador dorme. Se ele não conseguir dormir, o monitor continua apagado e a sessão
  bloqueada.
- Fechar a tampa com um monitor externo ligado só desliga a tela do notebook (dá para
  continuar no externo). Se o monitor externo sair com a tampa fechada, a tela apaga e
  bloqueia na hora.

## Computadores mais simples (Fase 13)

- Os apps não redesenham mais a janela só porque o mouse passou por cima: se nada
  muda na tela, não gastam nada. Num processador fraco era isso que mais pesava.
- Os apps redesenham só o pedaço da janela que mudou (a letra digitada, a linha
  escolhida), não a janela inteira. Se algum app mostrar um pedaço velho na tela, rode-o
  com `GLOVE_FULL_REDRAW=1` (o jeito antigo) e avise; `GLOVE_CHECK_DAMAGE=1` confere
  cada quadro e escreve no terminal quando acha diferença.
- Arrastar janelas não dá mais um tranco a cada segundo (a rede de segurança do
  compositor descartava o quadro em curso; correção de darkstarfire).
- **Preferências > Aparência > Reduzir efeitos**: sem animações (minimizar, Exposé,
  mesas, Todos os Apps), sem ampliação do Dock, sem o ícone pulando ao abrir, sem
  sombras nas janelas e sem o botão do alerta pulsando. Vale na hora, sem reiniciar.
- **Preferências > Itens de Início** (também em Contas, ou Monitor de Atividade > Processo >
  Itens de Início… / Serviços…): na aba **Aplicativos**, o que abre sozinho ao entrar e
  quanta memória cada um gasta agora; na aba **Serviços**, os serviços do sistema que
  costumam pesar (impressão, Bluetooth, AnyDesk, Docker, SSH, atualizações automáticas…),
  com estado e memória. Desmarcar desliga na hora e nas próximas vezes; os do sistema
  pedem a senha de administrador.
- Para achar o que ainda pesa: rode um app com `GLOVE_PROF=1` no terminal (mostra o
  tempo de cada quadro) ou a sessão inteira com `GLOVE_PROF=1`.

## Instalar o Glove pela ISO

1. **Baixar a ISO** pronta da versão na página de [Releases](https://github.com/fabioaacarneiro/glove/releases) (o GitHub monta uma a cada
   versão; confira com `sha256sum -c SHA256SUMS`) **ou montar** (no computador onde está o código):
   `./glove-setup-tool.sh iso`. Ela fica em `iso/glove-os-VERSÃO-amd64.iso`.
2. **Testar numa máquina virtual** antes de usar num computador de verdade:
   `./glove-setup-tool.sh iso testar` (precisa do QEMU: `sudo apt install qemu-system-x86 ovmf`). O
   disco virtual de 32 GB fica em `iso/disco-teste.qcow2`. GNOME Boxes e VirtualBox também abrem a ISO.
3. **Gravar num pendrive** (apaga o pendrive): `sudo dd if=iso/glove-os-VERSÃO-amd64.iso of=/dev/sdX
   bs=4M status=progress oflag=sync`, trocando `sdX` pelo pendrive (confira com `lsblk`).
4. **Ligar o computador pelo pendrive** (menu de boot do computador). O Glove ao vivo entra sozinho e
   abre **Instalar o Glove OS**. Dá para usar o Glove ao vivo antes de instalar; o instalador também está
   na Busca Rápida. No sistema ao vivo, o usuário é `glove` e a senha é **`live`** (para o `sudo` e
   para entrar pelo console, e para desbloquear se você bloquear a tela com Super+L); o `root` não tem
   senha. No sistema ao vivo, a tela que apaga sozinha e o repouso não pedem senha ao voltar.

A ISO já sai com a cara do Glove OS do começo ao fim, sem nada para configurar: o menu de boot com o
papel de parede, a tela de início e de desligamento do sistema (Plymouth, com o tema do Glove) e a
tela de login. O sistema instalado fica igual: GRUB com o papel de parede e a tela de início do Glove.

O instalador pergunta:

- **Idioma e teclado** do sistema instalado (com um campo para testar o teclado).
- **Fuso horário:** procure a cidade.
- **Disco:**
  - **Apagar o disco inteiro e instalar o Glove**: o aviso em vermelho mostra qual disco será apagado;
  - **Instalar ao lado do sistema atual**: usa o maior espaço livre do disco. Precisa de UEFI, de uma
    partição EFI e de 20 GB livres. Para abrir esse espaço, clique em **Preparar o disco com o
    Discos…**: no Discos, escolha a partição do Windows (NTFS), **Redimensionar…** para um tamanho
    menor e feche o Discos; a lista do instalador se atualiza sozinha e passa a oferecer o espaço
    livre. (Também dá para diminuir pelo próprio Windows, em "Gerenciamento de Disco ▸ Diminuir
    volume".)
  - **Instalar numa partição escolhida**: formata só a partição escolhida (20 GB ou mais) e instala
    nela; as outras partições do disco ficam como estão. Precisa de UEFI e de uma partição EFI no
    disco (a do Windows, por exemplo). Boa para reaproveitar a partição de um Linux antigo, ou uma
    criada antes no Discos.
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
| Alt+Tab | Faixa com as janelas do app em uso (como a do Super+Tab, com as miniaturas). Com o Alt preso, Tab e Shift+Tab (ou ← →) escolhem, Q fecha a janela escolhida, Esc cancela; soltar o Alt traz a escolhida |

## Ícones modernos (Mide)

Em **Preferências › Aparência › Ícones**, além do tema do Glove (o padrão, da época), há os ícones do
**Mide**: **Mide**, **Mide Claro** e **Mide Escuro**, mais modernos, desenhados pelo projeto Mide (a
partir do WhiteSur, sem arte da Apple). Eles já vêm no Glove OS e chegam pelas atualizações (pacote
`glove-mide-icons`). Os apps do Glove usam os desenhos do Mide quando ele tem um próprio (Arquivos,
Terminal, Editor de Texto, Preferências…). Os painéis das Preferências também mudam para os ícones do
tema escolhido, e no Dock a sombra de cada ícone acompanha o desenho novo.

## Aparência Moderna (Claro e Escuro)

Em **Preferências › Aparência › Aparência**, além dos temas da época (Claro, Listras, Metal e Grafite,
o padrão continua o Claro), há o **Moderno Claro** e o **Moderno Escuro**: janelas lisas de cantos
arredondados, botões de fechar, minimizar e ampliar planos, barra de menu translúcida, menus
arredondados, Dock de vidro fosco, botões e caixas lisos e barras de rolagem finas. O Escuro deixa as
janelas em grafite com o texto claro. Combina com os ícones do Mide (Mide Claro e Mide Escuro). Os
menus de escolha (como Aparência, Cor de destaque e Ícones) também ficam lisos, com as setas num
quadradinho na cor de destaque.

