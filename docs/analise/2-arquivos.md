# Análise 2 — Arquivos (gerenciador de arquivos) e diálogos de arquivo

Escopo: `crates/files`, `crates/find`, `crates/disks`, a folha de abrir/salvar de `crates/editor`, widgets de `crates/kit/src/ui.rs`, ícones de `crates/paint/src/icons.rs`.
Regra: nada de arte, logotipo, nome ou código da Apple. Copiamos comportamento, proporções e linguagem visual; os desenhos são nossos (tiny-skia). Nos textos, "Finder" vira **Arquivos**, "iDisk" e "Sherlock" somem e "Favorites" vira **Favoritos**.

Legenda: prioridade **Alta/Média/Baixa** · esforço **P** (até 1 dia), **M** (2–4 dias), **G** (1 semana ou mais).

---

## 1. Janela do Arquivos: barra de ferramentas e proporções

**(a) Referências**
- **10.0–10.2** (`jaguar/10-2-Jaguar-Finder-Home-scaled.png`, `puma/10-1-Puma-Finder-Home-scaled.png`, `cheetah/10-0-Cheetah-Finder-Home-scaled.png`, `jaguar/…-Finder-Network-…`):
  - Barra listrada de cerca de 58 pt de altura. Todos os itens têm **ícone de 32 px com rótulo de 11 pt embaixo**.
  - Ordem da esquerda para a direita: **Back** e **Forward** (círculos cinza com setas, Forward desativado e esmaecido), **View** (controle segmentado de 3 partes com glifos de grade, linhas e colunas; a parte ativa fica em azul gel), um **separador vertical pontilhado**, depois **Computer, Home, Favorites, Applications** e por fim o campo **Search** arredondado (cápsula) com o rótulo embaixo. Cheetah não tem Search na barra; ele chega no 10.2.
  - O botão em pílula no canto direito da barra de título esconde a barra de ferramentas.
  - A janela inicial tem cerca de 640×440 pt.
- **10.3 Panther** (`panther/system-managers-filemanager_-1-1…1-6.png`):
  - Barra compacta sobre **metal escovado**: Voltar/Avançar num segmentado único de ~44×20 pt, View com 3 glifos, **menu de ação com engrenagem** e, à direita, a **busca** em cápsula com lupa e o texto "local disks".
  - A janela tem cerca de 780×450 pt.
  - Com a barra escondida (`…-1-7.png`) a janela volta a ser Aqua listrada e perde a barra lateral.

**(b) Glove hoje** (`glove-files.png`, `Files::toolbar` em `crates/files/src/main.rs`)
- Barra de 62 px com ◀ ▶ em gel, sem rótulo.
- Três botões de **texto** ("Ícones", "Lista", "Colunas") separados entre si.
- Lugares alinhados à direita: Computador, Início, Documentos e Imagens. Não há busca, separador nem pílula.
- Tamanho 780×470 (`main()`).

**(c) Lacuna**
- O visual "Ícones / Lista / Colunas" em texto é o que mais denuncia a diferença.
- Faltam os rótulos sob Voltar/Avançar, o separador pontilhado, a busca e o atalho Aplicativos.
- A ordem difere: na referência os lugares vêm logo após o separador, e não encostados na borda direita.

**(d) Tarefas**
1. Criar `Ui::segmented(rect, &[Glyph], sel)` em `kit/src/ui.rs` (cápsula gel com divisórias e o segmento ativo em azul). Desenhar os glifos de grade (4 quadrados), lista (4 linhas) e colunas (3 retângulos) em `paint/src/icons.rs` como `Icon::ViewIcons/ViewList/ViewColumns`. Substituir os 3 `gel_button` de `toolbar()`. — **Alta, P**
2. Refazer o layout de `toolbar()` no estilo 10.2:
   - Voltar e Avançar como círculos de 28 px com rótulo "Voltar"/"Avançar" a 11 pt.
   - O segmento "Visualizar".
   - Separador pontilhado.
   - Computador, Início, Favoritos e Aplicativos.
   - Campo "Buscar" (cápsula) que chama `glove-find` com o texto e a pasta atual.
   - Desativar Voltar/Avançar quando a pilha estiver vazia.
   — **Alta, M**
3. Variante Panther, quando `theme.variant == Metal`: barra compacta de 34 px com segmentado ◀▶, View, engrenagem (menu de contexto) e busca à direita. Ver a seção 2. — **Média, M**
4. Botão em pílula na barra de título (`paint/src/chrome.rs` mais protocolo no compositor) que alterna `Files.toolbar_visible`. — **Baixa, M**
5. Tamanho inicial de 680×440 e lembrar tamanho e visualização por pasta num arquivo leve (`~/.config/glove/arquivos-pastas.toml`). — **Baixa, P**

## 2. Barra lateral e metal escovado (Panther)

**(a) Referências** (`panther/system-managers-filemanager_-1-1.png`, `system-features-trashcan_.png`, `interface-dialogs-openfile_-1-1.png`)
- Barra lateral branca de cerca de 118 pt, embutida no metal com borda chanfrada.
- Duas seções separadas por uma linha:
  - em cima, **Rede** e os **discos**;
  - embaixo, **Mesa, Início (nome do usuário), Aplicativos, Documentos, Filmes, Música, Imagens**.
- Ícones de 32 px e texto de 12 pt. O item selecionado tem fundo **azul sólido** e texto branco.
- Divisória arrastável com um ponto ("•") no meio.
- Metal escovado cobre a barra de título, a barra de ferramentas e o rodapé.
- O status fica **embaixo**, no rodapé de metal, centralizado: "6 items, 1.09 GB available". No 10.2 ele ficava em cima, abaixo da barra.

**(b) Glove** não tem barra lateral. O status fica em cima, o que é fiel ao 10.0–10.2. O metal existe só em `chrome::titlebar` (`Variant::Metal`), não no conteúdo.

**(c) Lacuna** Toda a cara do Panther.

**(d) Tarefas**
1. `Files::sidebar(ui, rect)`:
   - Lugares fixos: Computador/Disco (`/`), discos removíveis de `/proc/mounts`, separador, Mesa, Início, Aplicativos, Documentos, Downloads, Música, Imagens, Vídeos.
   - Seleção em azul, divisória com ponto, largura salva.
   - Mostrar só quando o tema for Metal, ou por uma preferência "Barra lateral".
   — **Alta, M**
2. Pintar `brushed_metal` em toda a área cromada (barra, rodapé e laterais com margem de 7 pt em volta do conteúdo branco). Mover a linha de status para o rodapé no modo Panther. Usar `ui.theme.variant` para escolher a posição. — **Média, P**
3. Itens arrastáveis para a barra lateral (favoritos do usuário), guardados em `~/.config/glove/lugares`. — **Baixa, M**

## 3. Linha de status

**(a)** 10.0–10.2: "1 of 9 items selected, 35.75 GB available", 11 pt centralizado. À **esquerda** aparecem um ícone de "barra oculta/ícones" e um **lápis riscado** em pastas só de leitura (`jaguar/…-Finder-Network…`, `…-Search…`). Na Lixeira do Panther há um ícone de lixeira à esquerda.

**(b)** `Files::status` mostra "12 itens, 8,5 GB disponíveis" e não indica a seleção.

**(d) Tarefas**
- Mostrar "1 de 12 selecionado, …".
- Ícone de lápis riscado quando `access(dir, W_OK)` falhar (glifo novo `Icon::NoWrite` de 12 px).
- Mensagens de erro como alerta (seção 8), em vez de substituir o texto do status.

**Média, P**

## 4. Visualização por ícones

**(a) Referências** (`jaguar/…-Home`, `…-Applications-Utilities…`, `panther/…-1-2.png`)
- Fundo branco, grade larga (~167 pt de passo no 10.2, ~190 pt no Panther), ícones de 32–48 px e rótulo de 12 pt centralizado.
- Seleção: ícone **escurecido** e rótulo com **fundo azul-claro arredondado** (Jaguar) ou azul sólido com texto branco.
- **Pastas especiais com emblema próprio**: Mesa, Documentos, Biblioteca, Filmes, Música, Imagens, Público, Sites. Atalhos (alias) levam uma **seta preta** no canto inferior esquerdo (Send Registration, Servers).

**(b)** `icons_view`: célula de 104×92 e ícone de 52 px. Seleção com um retângulo de 40% de preto e rótulo azul. Só há 4 tipos de ícone (`Kind::Folder/Text/Picture/Other`) e todas as pastas são iguais.

**(c) Lacuna** As pastas especiais e os tipos de arquivo são o que mais pesa no visual. O passo da grade é apertado demais.

**(d) Tarefas**
1. Em `icons.rs`, `Icon::FolderBadge(Badge)` desenha a pasta azul com um **emblema original gravado**: tela (Mesa), folha (Documentos), nota musical (Música), moldura (Imagens), claquete (Vídeos), seta para baixo (Downloads), pessoa (Público), livro (Modelos). Em `read_dir`, mapear os diretórios XDG (`xdg_dir`). — **Alta, M**
2. Novos tipos em `kind_of`, decididos pela extensão (sem libmagic, por leveza): PDF, áudio, vídeo, compactado (.zip/.tar), executável/script, `.desktop` (aplicativo) e link simbólico, que recebe o emblema de seta via `icons::draw(..., Some(overlay))`. — **Alta, M**
3. Escurecer o ícone selecionado de verdade (multiplicar por 0,6 numa camada) em vez do retângulo escuro. Passo da grade de 128×100 e tamanho do ícone ajustável (32/48/64) em "Opções de visualização". — **Média, P**
4. Seleção por **retângulo elástico** e múltipla (Ctrl/Shift), com `selected: Vec<PathBuf>`. — **Média, M**

## 5. Visualização em lista

**(a)** `panther/…-1-3.png` e `…-1-4.png`, `jaguar/…-Finder-Search…`:
- Cabeçalho Aqua com colunas **Nome, Data de modificação, Tamanho, Tipo**. A coluna ordenada fica em **azul** com um triângulo ▲.
- Linhas de 18 pt, **sem zebra** no Finder.
- **Triângulos de expansão** abrem a subpasta no lugar, com recuo de 16 pt (`…-1-4.png`).
- Datas relativas: "Yesterday, 8:51 PM". Pastas mostram "--" no tamanho.

**(b)** `list_view` tem cabeçalho cinza estático, zebra azul (`237,243,254`), não ordena, não expande e mostra "—".

**(d) Tarefas**
- Clicar no cabeçalho ordena por coluna (`sort: (Col, asc)`), com a coluna ativa em gradiente azul e ▲/▼.
- Remover a zebra e usar "--".
- `human_date` com "Hoje, 14:03" e "Ontem, 20:51".
- Triângulos de expansão com `expanded: HashSet<PathBuf>` e recuo.
- Colunas redimensionáveis por arraste.

**Alta, M**

## 6. Visualização em colunas

**(a)** `panther/…-1-5/1-6/1-7.png`, `interface-dialogs-openfile_-1-2.png`:
- Colunas de ~190 pt, linhas de 18 pt com ícone de 16 px e ▶ cinza à direita das pastas.
- O caminho em colunas anteriores fica em **cinza-claro** e só a última em **azul**.
- Cada coluna tem **barra de rolagem própria em gel azul** e uma alça "||" no rodapé para redimensionar.
- A **coluna de visualização** mostra um ícone grande e, abaixo, rótulos alinhados à direita: "Name / Kind / Size (6.1 MB on disk (6,087,005 bytes)) / Created / Modified / Version".
- Há barra horizontal quando as colunas não cabem.

**(b)** `columns_view` e `column` estão boas: cinza no caminho e azul na última. `preview` mostra Tipo, Tamanho e Modificado centralizados. Faltam a barra de rolagem por coluna, a alça e a rolagem horizontal; colunas antigas são simplesmente cortadas (`first = total - fit`).

**(d) Tarefas**
- Extrair um widget `Ui::scrollbar(rect, off, content)` com gel azul em `kit/ui.rs` (útil para todo o sistema) e usá-lo em cada coluna. — **Alta, M**
- Rolagem horizontal das colunas em vez de cortar. — **Média, M**
- `preview` com rótulos à direita mais "Nome", "Criado" (statx btime), "Tamanho: 6,1 MB (6.087.005 bytes)" e miniatura real para PNG/JPG (decodificada pelo `glove-view`, cache pequeno, máximo de 128 px). — **Média, M**

## 7. Get Info → "Obter Informações" (Ctrl+I)

**(a) Referências**
- **Cheetah/Puma** (`cheetah/…-Get-Info-1…4`, `puma/…-Get-Info-1…5`, `puma/…-Get-Info-2` mostra o painel Name & Extension): janela flutuante estreita (~265 pt) com um **menu "Show:"** que troca entre os painéis General, Name & Extension, Open with, Preview e Privileges.
- **Jaguar** (`jaguar/10-2-Jaguar-Finder-Get-Info-scaled.png`): janela alta de ~270 pt com **seções colapsáveis por triângulo**:
  - **General**: ícone de 32 px, nome, Kind, Size "106.4 MB on disk (111,624,652 bytes)", Where, Created, Modified, caixas "Stationery Pad" e "Locked".
  - **Name & Extension**: campo e "Hide extension".
  - **Open with**: popup e botão "Change All…".
  - **Preview**.
  - **Ownership & Permissions**: Owner, Access, Group, Others em popups e um **cadeado**.
  - **Comments**: área de texto.
  - Rótulos em negrito-regular de 12 pt alinhados à direita.

**(b)** Não existe.

**(d) Tarefas**
- Novo modo `glove-files --info <caminho>`: janela de 280×variável, `WindowOptions{fixed:true}`, título "<nome> Informações".
- Widget `Ui::disclosure(&mut bool, título)` em `kit/ui.rs`.
- Seções:
  - **Geral**: Tipo, Tamanho, Onde, Criado, Modificado e "Protegido", que equivale a `chattr`/`chmod a-w`.
  - **Nome e extensão**: renomear.
  - **Abrir com**: lista dos `.desktop` com o MIME, gravando em `mimeapps.list`.
  - **Dono e permissões**: popups de rwx para dono, grupo e outros via `chmod`.
  - **Comentários**: xattr `user.xdg.comment`.
- Atalho Ctrl+I e item no menu Arquivo.

**Alta, G**

## 8. Diálogos de operação: cópia, substituir, Lixeira, ejetar

**(a) Referências**
- **Copy** (`jaguar/…-Finder-Copy-scaled.png`, `puma/…`, `cheetah/…`):
  - Janela "Copiar" de ~400×140 pt, listrada.
  - Triângulo, "Copying "X" to "Desktop"" em negrito.
  - Barra de progresso azul com o botão **Stop** ao lado.
  - Linhas "Item:", "Copied: 5.5 MB of 106.4 MB" e "Estimating time remaining…".
  - Várias cópias empilham na mesma janela.
- **Copy-Replace** (`…-Copy-Replace-…`): barra **indeterminada listrada (barber pole)**, ícone ⚠ amarelo e o texto "An item named "X" with extension ".pdf" already exists in this location. Do you want to replace it…", com botões Stop e **Replace** (padrão).
- **Empty Trash** (`jaguar/…-Empty-Trash`, `cheetah/…`):
  - Alerta com **octógono vermelho com "!"** e o ícone do app sobreposto.
  - Título em negrito 13 pt: "Are you sure you want to remove the items in the Trash permanently?"
  - Corpo de 11 pt: "You cannot undo this action."
  - Botões Cancel e **OK** em azul pulsante, alinhados à direita.
- **Eject Warning** (`jaguar/…-Eject-Warning`): octógono vermelho e texto em negrito "The device you removed was not properly put away…".
  - No Cheetah (`cheetah/10-0-Cheetah-USB-Dismount-scaled.png`) o ícone é ⚠.
  - No Puma (`puma/…-Eject-Warning`) é um "i" azul.

**(b)** Não há cópia/colar nem arrastar entre pastas (`move_to_trash` só faz `rename`/`copy`). Não há "Esvaziar Lixeira". Os erros vão para a linha de status. `glove-disks` ejeta, mas não avisa sobre remoção sem ejetar.

**(d) Tarefas**
1. `kit`: `Ui::alert(icon: AlertIcon::{Stop, Caution, Note}, titulo, texto, botões)` como folha padrão. Desenhar o octógono vermelho, o triângulo âmbar e o círculo azul "i" **originais** em `icons.rs`. Botão padrão com pulsação: a fase do gel varia com o tempo e o redraw é feito só a 10 Hz, para não gastar CPU. — **Alta, M**
2. Copiar, recortar e colar arquivos (Ctrl+C / Ctrl+V; "Mover" com Ctrl+Alt+V) e arrastar e soltar entre janelas. Isso exige DnD no compositor (`wl_data_device` já existe no Smithay); como alternativa simples, arrastar dentro da mesma janela e para a barra lateral. — **Alta, G**
3. Thread de cópia com progresso, usando `copy_file_range` (sem buffer grande, ~64 KB):
   - Janela "Copiar" com barra, "Parar", "Item:", "Copiado: X de Y" e "Calculando tempo restante…".
   - Conflito usa a barra listrada indeterminada (widget `Ui::progress(Option<f32>)` com animação barber pole) e o texto: "Já existe um item chamado "X" com a extensão ".pdf" neste local. Deseja substituí-lo pelo que está sendo movido?" com [Parar] [Substituir].
   — **Alta, G**
4. **Esvaziar Lixeira** (Ctrl+Shift+Backspace, no menu "Arquivos" e no menu de contexto do ícone da Lixeira no Dock). Alerta Stop: "Tem certeza de que deseja apagar permanentemente os itens da Lixeira?" / "Esta ação não pode ser desfeita." [Cancelar] [OK]. Apagar `files/` e `info/` em thread. Mais o ícone da Lixeira **cheia e vazia** no Dock (`Icon::Trash` com estado, verificando `files/` por inotify). — **Alta, M**
5. "Devolver" (restaurar) usando o `.trashinfo`, com título "Lixeira" e o ícone da lixeira na linha de status quando o diretório for a Lixeira (`system-features-trashcan_.png`). — **Média, P**
6. Aviso de ejeção: o compositor ou o `glove-disks` vigia `/proc/self/mountinfo` (poll) e, quando um removível some sem `udisksctl unmount`, mostra o alerta Stop: "O dispositivo removido não foi ejetado corretamente. Dados podem ter sido perdidos ou danificados. Antes de desconectar, selecione o ícone dele no Arquivos e escolha Ejetar no menu Arquivo." [OK]. Adicionar "Ejetar" (Ctrl+E) ao Arquivos e ícone do pendrive na mesa. — **Média, M**

## 9. Menu Ir, "Ir para a Pasta", "Conectar ao Servidor"

**(a) Referências**
- **Menu Go** (`jaguar/…-Go-Menu-scaled.png`): Back ⌘[, Forward ⌘], separador, **Computer ⇧⌘C, Home ⇧⌘H, iDisk, Applications ⇧⌘A, Favorites ⇧⌘F**, cada item **com mini-ícone de 16 px**. Depois "Favorites ▶", "Recent Folders ▶", "Go to Folder… ⇧⌘G" e "Connect to Server… ⌘K".
- **Go To Folder** (`jaguar/…-Go-To-scaled.png`, `puma/…`, `cheetah/…`):
  - Folha de ~370 pt: "Go to the folder:", campo com **autocompletar por Tab** (a parte completada aparece selecionada, como em "~/Documents/"), [Cancel] [**Go**].
- **Connect to Server** (`jaguar/…-Connect-to-Server`, `puma/…`):
  - "Choose a server from the list or enter a server address".
  - "At: [Network ▾]" e um botão de recolher.
  - Navegador em colunas com servidores encontrados (globo), "5 items" e "Searching…" com spinner.
  - "Address:" e os botões [Add to Favorites] [Cancel] [**Connect**].
- **Network** (`jaguar/…-Finder-Network…`, `cheetah/…`): pasta com Applications, Library, Servers (alias) e Users.

**(b)** O menu "Ir" tem Voltar, Avançar, Pasta acima, Computador, Início, Documentos, Imagens e Lixeira. Não tem ícones, Aplicativos, recentes, "Ir para a pasta" nem conexão de rede.

**(d) Tarefas**
1. `MenuItem::icon(Icon)` em `kit` e no menu do compositor (mini-ícone de 16 px à esquerda). — **Média, P**
2. Completar o menu "Ir": Aplicativos (Ctrl+Shift+A), Utilitários, Favoritos, **Pastas recentes ▶** (submenu, últimas 10 pastas em `~/.local/state/glove/recentes`), **Ir para a pasta… (Ctrl+Shift+G)** e **Conectar ao servidor… (Ctrl+K)**. — **Alta, P**
3. Folha "Ir para a pasta" com `text_field`, Tab completando o prefixo do diretório e a parte sugerida selecionada, e o erro "A pasta não pode ser encontrada." — **Alta, P**
4. "Conectar ao servidor": campo de endereço (`smb://`, `sftp://`, `ftp://`) que monta via `gio mount` ou `sshfs`, se instalado, e abre a pasta. Favoritos simples. A descoberta mDNS fica para depois (Avahi custa memória; usar `avahi-browse` sob demanda). — **Baixa, M**
5. Lugar "Rede" na barra lateral, listando montagens `gvfs` e `fuse` ativas. — **Baixa, M**

## 10. Pastas Aplicativos e Utilitários

**(a)** `jaguar/10-2-Jaguar-Applications-scaled.png`, `…-Applications-Utilities…`, `panther/…-1-2`: uma pasta comum que mostra **apps com ícones grandes**. "Utilities" é uma subpasta. O status mostra "26 itens, 35,75 GB disponíveis". Clique duplo abre o app.

**(b)** Não existe. Os apps só aparecem no Dock e no menu.

**(d) Tarefas** Pasta virtual `glove:aplicativos` (sem criar arquivos):
- lista os binários `glove-*`, com nome em pt-BR e `Icon::from_name`, mais os `.desktop` de `/usr/share/applications`, com o ícone genérico ou um PNG pequeno em cache;
- subpasta "Utilitários" com Terminal, Monitor, Discos, Captura, Caracteres e Busca;
- clique duplo lança o app.
Reaproveitar o `icons_view` com uma fonte alternativa a `read_dir`.

**Alta, M**

## 11. Busca

**(a) Referências**
- **10.0/10.1** usam o Sherlock (`cheetah/…-Finder-Search…`): janela de canais. Não copiar.
- **10.2** (`jaguar/…-Finder-Search-scaled.png`):
  - Busca dentro da janela do Finder, com título "Search Results for 'Connect' in 'shackett'" e status "2 items".
  - Lista com Nome, Data e Tamanho.
  - **Painel inferior** com o caminho do item selecionado, separado por uma alça "═" arrastável.
- **Panther**:
  - `system-features-search_-1-1.png` é a janela **Find** (~560×270): "Search in: [Home ▾]", um grupo "Search for items whose:" com linhas de critério [Name ▾][contains ▾][campo] (−)(+) e [Content ▾] includes […], e o botão [Search].
  - `…-2-1.png` mostra os resultados: tabela Nome / **Pasta-mãe** / Data, e no rodapé o **caminho em migalhas** (Midnite ▶ Library ▶ Preferences ▶ arquivo, com ícones).

**(b)** `glove-find` (`glove-find.png`): barra com lupa, campo, "em [Início ▾]" e [Buscar]. Lista e texto de ajuda. Busca só por nome. Não há campo de busca no Arquivos.

**(d) Tarefas**
1. No `glove-find`, a tabela de resultados ganha a coluna "Pasta" e o **painel de caminho em migalhas** na parte de baixo (clicar num segmento abre a pasta). Título "Resultados para “x” em “Início”". — **Média, M**
2. Critérios extras ao estilo Panther: Nome {contém, começa com, é}, Conteúdo (grep em arquivos de texto até N MB), Data de modificação e Tamanho, com (−)(+). Manter um único *walker* em thread. — **Baixa, M**
3. Campo de busca na barra do Arquivos: Enter abre o `glove-find` com `--em <pasta> --texto <q>`. — **Alta, P**

## 12. Abrir/Salvar (folha padrão de arquivo)

**(a) Referências**
- **10.0–10.2** (`jaguar/…-TextEdit-File-Open-scaled.png`, `puma/…`, `cheetah/…`):
  - Janela "Open" com o popup **"From: [📁 Documents ▾]"**, navegador **em colunas** com 3 colunas e barra horizontal em gel, campo **"Go to:"**, área de acessórios do app (Plain Text Encoding, "Ignore rich text commands") e os botões [Add to Favorites] … [Cancel] [**Open**] (desativado até haver seleção).
  - No Puma há uma **coluna de pré-visualização** com "Kind:".
- **Panther** (`panther/interface-dialogs-openfile_-1-1.png`, `-1-2.png`):
  - Janela de ~575×400.
  - Em cima: ◀▶, alternância lista/colunas e um popup de caminho centralizado.
  - **Barra lateral** igual à do Finder.
  - Lista (Nome / Data de modificação) ou colunas.
  - Embaixo: [New Folder] … [Cancel] [Open].
- **Salvar** (convenção da época): folha que desce da barra de título com "Salvar como: [nome]", "Onde: [popup]" e um triângulo que expande o navegador.

**(b)** Na folha do `glove-edit` (`Editor::sheet`, 440×150) o usuário **digita o caminho** num `text_field`: "Abrir arquivo / Digite o caminho do arquivo." O `glove-view` provavelmente faz o mesmo.

**(c) Lacuna** É a maior diferença funcional depois de copiar/colar: não há como navegar.

**(d) Tarefas**
1. Extrair do `crates/files` um módulo reutilizável `glove_kit::filebrowser` com `column()`, `list_view()`, `read_dir`, `kind_of`, ícones e a barra lateral, para não duplicar código e manter os binários pequenos. — **Alta, M**
2. `kit::file_sheet(mode: Open|Save, filtros)`:
   - **Abrir**: 575×400, popup de caminho, barra lateral, colunas, [Nova pasta] [Cancelar] [Abrir].
   - **Salvar**: compacta, com "Salvar como:" e "Onde: [popup de lugares]"; o botão ▼ expande para o navegador.
   - Digitar "/" ou "~" abre o "Ir para:" como atalho.
   Usar no `glove-edit`, no `glove-view` e no `glove-notes`. — **Alta, G**
3. Opcional: expor como `xdg-desktop-portal` FileChooser mínimo (apps GTK e Qt usariam a folha do Glove). — **Baixa, G**

## 13. Imprimir

**(a)** `jaguar/10-2-Jaguar-Print-scaled.png` (e `puma/…`, `cheetah/…`):
- Folha que desce da barra de título, com Printer: [No Printer Selected ▾], Presets: [Standard ▾] e o popup de painéis ("Copies & Pages / Layout / Summary").
- Copies [1] ☑ Collated, Pages ◉ All ○ From [1] to [1].
- Embaixo: (?) [Preview] [Save As PDF…] … [Cancel] [Print].

**(b)** Não existe.

**(d) Tarefas** Folha "Imprimir" no `glove-edit` com Impressora (lista de `lpstat -a`), Cópias e Páginas, mais [Salvar como PDF…] (renderizar texto para PDF exigiria uma dependência; avaliar `lp` com o texto puro primeiro). **Baixa, M**

## 14. Utilitário de Disco → "Discos"

**(a)** `jaguar/10-2-Jaguar-Disk-Utility-scaled.png` (e `cheetah/…`, `puma/…`):
- Janela de ~735×540.
- À esquerda, contagem "0 Disks and 1 Volume Selected" e uma **árvore Disco > Volume** (ícone de HD de 16 px, volume recuado).
- À direita, **abas** Information / First Aid / Erase / Partition / RAID. A aba ativa fica em azul e é ligada à caixa por uma faixa azul.
- No painel: ícone de HD de 64 px e pares alinhados à direita: Mount Point, Format, Capacity "38.18 GB (40,995,252,224 Bytes)", Available, Used, Number of Files e Folders.
- Rodapé com cadeado: "Click the lock to prevent further changes."

**(b)** `glove-disks` (`glove-disks.png`) mostra cartões por volume (nome, ponto de montagem, dispositivo, sistema de arquivos, barra de uso e "Ejetar"). É agradável e leve, mas não tem árvore nem abas.

**(d) Tarefas**
- Layout de duas colunas: árvore disco/partição a partir de `/sys/block` e `/proc/partitions`; à direita, `Ui::tabs` (já existe) com **Informações** (pares alinhados à direita e bytes formatados) e **Primeiros socorros** (`fsck -n` em volume desmontado, com saída num `text_area`). Apagar e Particionar ficam para depois, por risco.
- O cadeado chama `pkexec`.
- Manter a barra de uso como extra.

**Média, M**

## 15. Comportamentos gerais que faltam no Arquivos
- **Renomear**: clicar no nome selecionado ou Enter abre um campo no lugar, como no Finder (o Enter **renomeia** e o Ctrl+O/Ctrl+↓ abre). — **Alta, P**
- **Menu de contexto** (botão direito ou Ctrl+clique): Abrir, Obter informações, Duplicar (Ctrl+D), Criar atalho (symlink, Ctrl+L), Mover para a Lixeira, Copiar "x". O mesmo menu serve para a engrenagem do Panther. — **Alta, M**
- **Duplicar** gera "x cópia". **Atalho (alias)** gera um symlink com emblema de seta. — **Média, P**
- **Pastas que se abrem ao arrastar por cima** (spring-loaded, 10.2) — **Baixa, M**
- **Atualização automática**: o cache já relê por mtime. Acrescentar inotify via `watch_fd`, como o `glove-find` faz, para não ficar redesenhando em sondagem. — **Média, P**

---

## Lista priorizada

| # | Tarefa | Arquivos | Pri. | Esf. |
|---|---|---|---|---|
| 1 | Segmentado com glifos (Ícones/Lista/Colunas) e barra 10.2 com rótulos, separador, Aplicativos e Buscar | `kit/ui.rs`, `paint/icons.rs`, `files::toolbar` | Alta | M |
| 2 | Alertas padrão (Stop/Cuidado/Nota) e botão padrão pulsante | `kit/ui.rs`, `paint/icons.rs` | Alta | M |
| 3 | Esvaziar Lixeira com alerta, Lixeira cheia/vazia no Dock, Devolver | `files`, `compositor/shell.rs`, `icons.rs` | Alta | M |
| 4 | Folha Abrir/Salvar com navegador (módulo `kit::filebrowser`) | `kit`, `editor`, `viewer`, `notes` | Alta | G |
| 5 | Copiar/colar/mover com janela de progresso e alerta de substituição | `files` (thread de cópia), `kit::progress` | Alta | G |
| 6 | Ícones de pastas especiais e de tipos, mais emblema de atalho | `paint/icons.rs`, `files::kind_of` | Alta | M |
| 7 | Renomear no lugar e menu de contexto (Duplicar, Atalho, Info) | `files` | Alta | M |
| 8 | Obter Informações com seções colapsáveis | `files --info`, `kit::disclosure` | Alta | G |
| 9 | Menu Ir completo e folha "Ir para a pasta" com Tab | `files::menus`, `kit::MenuItem::icon` | Alta | P |
| 10 | Pasta virtual Aplicativos/Utilitários | `files` | Alta | M |
| 11 | Lista ordenável com triângulos de expansão e datas "Hoje/Ontem" | `files::list_view` | Alta | M |
| 12 | Barra de rolagem gel por coluna e rolagem horizontal | `kit::scrollbar`, `files::columns_view` | Alta/Média | M |
| 13 | Modo Panther: barra lateral, metal no conteúdo, status no rodapé, engrenagem | `files`, `paint/draw.rs` | Média | M |
| 14 | Linha de status com seleção e indicador "somente leitura" | `files::status` | Média | P |
| 15 | Aviso de remoção sem ejetar e Ejetar no Arquivos | `disks`/`compositor`, `files` | Média | M |
| 16 | Busca: painel de caminho, coluna Pasta, critérios (+/−) | `find` | Média | M |
| 17 | Discos: árvore, abas Informações e Primeiros socorros | `disks` | Média | M |
| 18 | Coluna de pré-visualização com miniatura e "bytes" | `files::preview` | Média | M |
| 19 | inotify, seleção múltipla e retângulo elástico | `files` | Média | M |
| 20 | Conectar ao servidor, Rede, Imprimir, pílula de esconder a barra, portal xdg | vários | Baixa | M–G |

Leveza: tudo acima cabe sem dependências novas. Os ícones são vetoriais e desenhados uma vez em cache. Cópia e busca rodam em threads com buffers pequenos. Rede e impressão usam ferramentas externas (`gio`, `sshfs`, `lpstat`) apenas sob demanda.
