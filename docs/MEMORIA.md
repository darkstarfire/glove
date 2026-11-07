# Memória e desempenho do Glove (estudo de 07/11/2026, v0.9.20)

Como medir:

- Por processo: `awk '/^Pss:/{print $2}' /proc/PID/smaps_rollup` (PSS = a parte da
  memória que é de fato daquele processo; o que é dividido conta só a fração dele).
- Dentro do compositor (modo de teste, `GLOVE_DEBUG_CTL=1`): o comando `mem` registra o
  heap (total, em uso, livre retido), o que cada cache guarda (ícones, sombras do Dock,
  metal, ícones de tema, letras) e o RSS; `trim` devolve ao sistema a memória livre que o
  malloc segura e mede de novo.
- Quanto o Rust alocou de verdade: compile com `cargo build --release -p glove --features
  memstats` e use `mem` (mostra o que está vivo e o pico). Deixa o Glove um pouco mais lento;
  só para medir.

## O que foi encontrado

| Onde | Quanto | Observação |
|---|---|---|
| Compositor, Mesa (driver OpenGL) | ~45 MB de heap + libLLVM ~16 MB | custo fixo de qualquer compositor acelerado; um contexto só; variáveis da Mesa (`mesa_glthread`, cache de shaders) não mudam nada |
| Compositor, o Glove em si | ~4 MB (pico 8 MB) | ícones, sombras, letras e texturas: pouco |
| Apps | 9 a 13 MB cada | metade era código repetido (cada app tinha a sua cópia) e a cópia das fontes |
| Processador parado | 0,1% de um núcleo | o Glove só redesenha quando algo muda |
| Disco | 36 executáveis, 166 MB | o código de desenho repetido em cada um |

Limitar as arenas do malloc (`MALLOC_ARENA_MAX=2`) e o `malloc_trim` não mudaram nada: o
heap quase não tem memória livre retida.

## O que mudou

1. **Um executável para todos os apps (`glove-apps`)**, como o BusyBox: `glove-files`,
   `glove-prefs` e os outros 30 são atalhos para ele, e ele abre o app pelo nome com que
   foi chamado. Cada app continua um processo separado (o mesmo comportamento), mas as
   páginas de código são divididas entre todos. Os atalhos são criados pelo `build.rs` do
   `glove-apps` ao lado do executável. O compositor (`glove`), o portal e o `glove-arte`
   continuam separados.
2. **Fontes mapeadas do arquivo** em vez de copiadas: as páginas vêm do cache de disco do
   sistema e são as mesmas para todos os processos.

Resultado com Arquivos, Preferências e Boas-vindas abertos: **35,0 MB → 26,0 MB** (−26%),
e a economia cresce a cada app aberto. Disco: **166 MB → 44 MB**. Compilação completa:
de ~7 para ~2,5 minutos. Nada muda na aparência nem no comportamento.

## Tema de ícones

PNG sem perda com compressão máxima, e as versões 2× que têm os mesmos pixels de um tamanho
1× viram atalhos para ele: 1.344 → 840 imagens, 7,5 MB → 4,0 MB.
