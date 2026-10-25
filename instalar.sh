#!/usr/bin/env bash
# Installs Organizador PC for the current user (no sudo needed).
# Instala o Organizador PC para o usuário atual (não precisa de sudo).
#
#   ./instalar.sh                  build (if needed), install and start the service
#   ./instalar.sh --uninstall      stop the service and remove the binary (keeps the config)
#   (aliases / equivalentes: --desinstalar)
#
# To move it to another PC, build the package with ./empacotar.sh, copy the
# folder (or the .tar.gz) from dist/ and run this script there.
# Para levar para outro PC, gere o pacote com ./empacotar.sh, copie a pasta
# (ou o .tar.gz) de dist/ e rode este script lá.
set -euo pipefail

cd "$(dirname "$(readlink -f "$0")")"

# Message language: Portuguese if the locale starts with "pt", English otherwise.
case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in
    pt*) PT=1 ;;
    *) PT=0 ;;
esac
t() { if [[ $PT == 1 ]]; then printf '%s' "$1"; else printf '%s' "$2"; fi; }

BIN_DESTINO="$HOME/.local/bin/organizador-pc"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/organizador-pc"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"

if [[ "${1:-}" == "--desinstalar" || "${1:-}" == "--uninstall" ]]; then
    systemctl --user disable --now organizador-pc.service 2>/dev/null || true
    rm -f "$UNIT_DIR/organizador-pc.service" "$BIN_DESTINO"
    systemctl --user daemon-reload
    echo "$(t "Desinstalado. O config foi mantido em" "Uninstalled. The config was kept in") $CONFIG_DIR/config.txt"
    exit 0
fi

# 1. Binary: use a ready-made one next to the script; otherwise build it.
if [[ -x ./organizador-pc ]]; then
    BIN=./organizador-pc
elif command -v cargo >/dev/null; then
    # With the musl target installed (Arch: rust-musl package) the binary is
    # static and runs on any distro; without it, it depends on this PC's glibc.
    if rustc --print sysroot 2>/dev/null | xargs -I{} test -d "{}/lib/rustlib/x86_64-unknown-linux-musl"; then
        cargo build --release --target x86_64-unknown-linux-musl
        BIN=target/x86_64-unknown-linux-musl/release/organizador-pc
    else
        cargo build --release
        BIN=target/release/organizador-pc
    fi
else
    t "Erro: não há binário 'organizador-pc' nesta pasta e o cargo (Rust) não está instalado." \
      "Error: there is no 'organizador-pc' binary in this folder and cargo (Rust) is not installed." >&2
    echo >&2
    exit 1
fi
install -Dm755 "$BIN" "$BIN_DESTINO"
echo "$(t "Binário instalado em" "Binary installed in") $BIN_DESTINO"

# 2. Config: never overwrites an existing one. Uses the config.txt next to the
#    script (yours, from the package) or, if absent, the example in your language.
mkdir -p "$CONFIG_DIR"
if [[ ! -f "$CONFIG_DIR/config.txt" ]]; then
    if [[ -f config.txt ]]; then
        ORIGEM=config.txt
    elif [[ $PT == 1 ]]; then
        ORIGEM=config.exemplo.txt
    else
        ORIGEM=config.example.txt
    fi
    cp "$ORIGEM" "$CONFIG_DIR/config.txt"
    echo "$(t "Config criado em" "Config created in") $CONFIG_DIR/config.txt — $(t "revise antes de continuar." "review it before continuing.")"
    echo
    echo "$(t "Simulação com o config atual (nada é movido):" "Dry run with the current config (nothing is moved):")"
    "$BIN_DESTINO" --dry-run | tail -n 20
    echo
    read -rp "$(t "Iniciar o serviço agora? [s/N] " "Start the service now? [y/N] ")" resp
    [[ "$resp" =~ ^[sSyY] ]] || {
        echo "$(t "Edite o config e rode ./instalar.sh de novo." "Edit the config and run ./instalar.sh again.")"
        exit 0
    }
fi

# 3. User systemd service: starts with the session.
install -Dm644 organizador-pc.service "$UNIT_DIR/organizador-pc.service"
systemctl --user daemon-reload
systemctl --user enable organizador-pc.service
systemctl --user restart organizador-pc.service
echo
echo "$(t "Serviço ativo. Comandos úteis:" "Service active. Useful commands:")"
echo "  journalctl --user -u organizador-pc -f     # $(t "ver o que está sendo movido" "see what is being moved")"
echo "  ($(t "mudanças no config valem assim que o arquivo é salvo" "config changes apply as soon as the file is saved"))"
echo "  systemctl --user status organizador-pc"
echo
echo "$(t "Para iniciar já no boot (antes do login):" "To start at boot (before login):") loginctl enable-linger $USER"
