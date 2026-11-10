#!/bin/sh
# Builds Glove, first deleting what earlier builds of the project left behind
# (the dependencies already built stay, so nothing is rebuilt from scratch).
#   ./build.sh         builds (release)
#   ./build.sh --all   deletes everything and builds from scratch (frees more space)
set -e
cd "$(dirname "$0")"
# Message language: Portuguese if the system locale is Portuguese, English otherwise.
case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in pt*) lang=pt ;; *) lang=en ;; esac
t() { if [ "$lang" = pt ]; then printf '%s' "$1"; else printf '%s' "$2"; fi; }

case "${1:-}" in
    --all|--tudo) cargo clean ;;
    *)
        for p in $(sed -n 's/^name = "\(.*\)"/\1/p' crates/*/Cargo.toml); do
            cargo clean --release -p "$p" >/dev/null 2>&1 || true
        done
        rm -rf target/debug target/release/incremental
        ;;
esac
cargo build --release
printf '%s %s\n' "$(du -sh target | cut -f1)" "$(t 'usados pela compilação' 'used by the build')"
