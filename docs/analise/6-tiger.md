# Tiger (10.4) comparado ao Glove

Referências em [`referencias/tiger/`](../../referencias/tiger/) (91 capturas da 512pixels).
O Glove de hoje segue o 10.0–10.2 (Jaguar). O Tiger mantém a linguagem gel, mas é mais
**sóbrio**: menos listras, mais cinza e metal, e barras de ferramentas unidas ao título.

## Diferenças de visual

| Área | Glove hoje (Jaguar) | Tiger (referência) |
|---|---|---|
| Barra de menu | Branca com listras | Degradê branco liso, linha fina embaixo (`Apple-Menu`) |
| Canto direito da barra | Relógio | Extras (Wi-Fi, monitor, volume), relógio, **nome do usuário em negrito** e o **botão azul quadrado da busca** (`Spotlight`) |
| Menus | Listras opacas, separador é um vão | Fundo cinza muito claro, listras quase invisíveis, **separador em linha fina** (#D4D4D4), destaque azul liso (#3875D7) |
| Dock | Prateleira listrada | Prateleira translúcida **lisa**, sem listras visíveis, separador em linha dupla (`Apple-Menu`, `Expose`) |
| Janelas | Título com listras, corpo com listras | Título em degradê cinza claro, corpo cinza quase liso (`System-Preferences-0`, `TextEdit`) |
| Barra de ferramentas | Separada do título, botões em gel grandes | **Unida ao título** (mesmo degradê ou metal), botões pequenos em cápsula, campo de busca em cápsula à direita |
| Arquivos (Finder) | Toolbar do 10.2 com ícones grandes, sem barra lateral | Metal escovado, **barra lateral** (discos, Mesa, Início, Aplicativos…), segmentado de visualização, engrenagem, busca (`Finder-Home`) |
| Preferências | Abas | Grade "Mostrar Todos" com faixas de categoria alternando cinza (`System-Preferences-0`) |
| Papel de parede | Ondas (original) | Azul com feixes de luz diagonais; as Ondas continuam valendo (arte própria) |

## Recursos novos da era (já no plano)

- **Busca Rápida** (`Spotlight`, `Spotlight-More-Results`): botão azul no canto, painel que desce
  com o campo no topo, "Melhor resultado" destacado e resultados agrupados por categoria (nome
  da categoria em cinza numa coluna à esquerda). → etapa 3.9
- **Exposé** (`Expose`, `System-Preferences-Dashboard-and-Expose`). → etapa 3.10
- **Dicionário** (`Dictionary`), **Livro de Fontes** (`Font-Book`). → 8.13 e 8.14
- **Alternador Super+Tab** do Tiger (`Command-Tab`): faixa de ícones grandes no meio da tela
  (diferente do 10.0–10.2, que usava o próprio Dock). → revisar a 4.1

## Decisões

1. **Tema "Claro" (visual do Tiger, com nome próprio), padrão do Glove** (etapa 1.8): barra de menu lisa, menus com separador em
   linha, Dock liso, janelas em degradê cinza. O tema **Listras** continua como o visual 10.0–10.2,
   com o trabalho já feito na Fase 1.
2. **Arquivos mira o Finder do Tiger**: as etapas 6.1 (toolbar do 10.2) e 6.10 (modo Panther)
   viram uma só, com barra lateral, metal e busca.
3. **Super+Tab no estilo do Tiger** (faixa central), por ser mais útil que percorrer o Dock.
