#!/usr/bin/env bash
# Builds the package to install Organizador PC on another PC:
# Gera o pacote para instalar o Organizador PC em outro PC:
#
#   dist/organizador-pc/          ready-to-copy folder / pasta pronta para copiar
#   dist/organizador-pc.tar.gz    the same folder, compressed / a mesma pasta compactada
#
# The package carries the compiled executable (the other PC does not need
# Rust) and your current config (~/.config/organizador-pc/config.txt), if any.
# O pacote leva o executável já compilado e o seu config atual, se ele existir.
# On the other PC / No outro PC: tar xzf organizador-pc.tar.gz, cd organizador-pc, ./instalar.sh
set -euo pipefail

cd "$(dirname "$(readlink -f "$0")")"

case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in
    pt*) PT=1 ;;
    *) PT=0 ;;
esac
t() { if [[ $PT == 1 ]]; then printf '%s' "$1"; else printf '%s' "$2"; fi; }

# With the musl target installed (Arch: rust-musl package) the executable is
# static and runs on any distro; without it, it needs glibc 2.34 or newer.
if rustc --print sysroot 2>/dev/null | xargs -I{} test -d "{}/lib/rustlib/x86_64-unknown-linux-musl"; then
    cargo build --release --target x86_64-unknown-linux-musl
    BIN=target/x86_64-unknown-linux-musl/release/organizador-pc
    TIPO="$(t "estático (roda em qualquer distro Linux x86_64)" "static (runs on any x86_64 Linux distro)")"
else
    cargo build --release
    BIN=target/release/organizador-pc
    TIPO="$(t "requer glibc 2.34+ (Ubuntu 22.04+, Debian 12+, Fedora 35+, Arch)" "needs glibc 2.34+ (Ubuntu 22.04+, Debian 12+, Fedora 35+, Arch)")"
fi

PASTA=dist/organizador-pc
rm -rf "$PASTA" dist/organizador-pc.tar.gz
mkdir -p "$PASTA"
install -m755 "$BIN" "$PASTA/organizador-pc"
install -m755 instalar.sh "$PASTA/instalar.sh"
install -m644 organizador-pc.service config.example.txt config.exemplo.txt README.md README.pt-BR.md LICENSE "$PASTA/"

CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/organizador-pc/config.txt"
if [[ -f "$CONFIG" ]]; then
    install -m644 "$CONFIG" "$PASTA/config.txt"
    echo "$(t "Incluído o seu config:" "Included your config:") $CONFIG"
    if grep -q "$HOME" "$PASTA/config.txt"; then
        t "Atenção: o config tem caminhos com $HOME; prefira ~ ou \$XDG_..._DIR para funcionar com outro usuário." \
          "Warning: the config has paths containing $HOME; prefer ~ or \$XDG_..._DIR so it works for another user."
        echo
    fi
else
    t "Nenhum config instalado neste PC; o pacote usará o exemplo." \
      "No config installed on this PC; the package will use the example."
    echo
fi

tar -czf dist/organizador-pc.tar.gz -C dist organizador-pc

echo
echo "$(t "Pacote pronto. Executável:" "Package ready. Executable:") $TIPO"
echo "  $(t "pasta:  " "folder: ") $PWD/$PASTA"
echo "  $(t "arquivo:" "archive:") $PWD/dist/organizador-pc.tar.gz"
