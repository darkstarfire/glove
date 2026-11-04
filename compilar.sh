#!/bin/sh
# Compila o Glove apagando antes o que sobrou das compilações anteriores do projeto
# (as dependências já compiladas ficam, para não recompilar tudo do zero).
#   ./compilar.sh          compila (release)
#   ./compilar.sh --tudo   apaga tudo e compila do zero (libera mais espaço)
set -e
cd "$(dirname "$0")"
if [ "$1" = "--tudo" ]; then
    cargo clean
else
    for p in $(sed -n 's/^name = "\(.*\)"/\1/p' crates/*/Cargo.toml); do
        cargo clean --release -p "$p" >/dev/null 2>&1 || true
    done
    rm -rf target/debug target/release/incremental
fi
cargo build --release
du -sh target | sed 's/\t.*/ usados pela compilação/'
