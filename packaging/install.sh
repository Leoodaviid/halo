#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Instala o halo no usuário, sem root: compila o halo e o halod, instala os
# binários, a unidade do systemd do usuário e a ativação pelo D-Bus, e sobe o
# daemon. Com --uninstall, remove só o que instalou e mantém perfis e
# snapshots. A regra udev é só conferida: os comandos com sudo são mostrados,
# nunca executados.
#
# Caminhos (para testes): BINDIR, XDG_CONFIG_HOME, XDG_DATA_HOME,
# XDG_STATE_HOME, CARGO_TARGET_DIR e HALO_UDEV_RULES_DIR.

set -eu

UNIT=halod.service
BUS_NAME=io.github.leoodaviid.Halo1
RULE=70-halo.rules

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)

say() { printf 'halo: %s\n' "$*"; }
die() { printf 'halo: erro: %s\n' "$*" >&2; exit 1; }

usage() {
    cat <<EOF
Uso: packaging/install.sh [--uninstall]

Sem argumentos, compila e instala (ou atualiza) o halo no usuário:
  binários   \$BINDIR (padrão ~/.local/bin): halo e halod
  unidade    \$XDG_CONFIG_HOME/systemd/user/$UNIT
  ativação   \$XDG_DATA_HOME/dbus-1/services/$BUS_NAME.service
e habilita e (re)inicia o $UNIT. Nada é escrito fora do \$HOME.

  --uninstall  para e desabilita o daemon e remove os arquivos acima.
               Perfis e snapshots ficam.
  --help       mostra esta ajuda.
EOF
}

[ -n "${HOME:-}" ] || die "HOME não está definido"

# Diretório XDG absoluto, ou o padrão; vazio ou relativo é ignorado, como
# manda a especificação XDG.
xdg() {
    case "${1:-}" in
        /*) printf '%s' "$1" ;;
        *) printf '%s' "$2" ;;
    esac
}

CONFIG_HOME=$(xdg "${XDG_CONFIG_HOME:-}" "$HOME/.config")
DATA_HOME=$(xdg "${XDG_DATA_HOME:-}" "$HOME/.local/share")
STATE_HOME=$(xdg "${XDG_STATE_HOME:-}" "$HOME/.local/state")
BINDIR=${BINDIR:-$HOME/.local/bin}
TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target}
UDEV_DIR=${HALO_UDEV_RULES_DIR:-/etc/udev/rules.d}

UNIT_DIR=$CONFIG_HOME/systemd/user
DBUS_DIR=$DATA_HOME/dbus-1/services
UNIT_FILE=$UNIT_DIR/$UNIT
DBUS_FILE=$DBUS_DIR/$BUS_NAME.service

case $BINDIR in
    /*) ;;
    *) die "BINDIR precisa ser um caminho absoluto: $BINDIR" ;;
esac
case $BINDIR in
    *'|'* | *'&'* | *'\'*) die "BINDIR não pode ter |, & nem \\: $BINDIR" ;;
esac

# Copia $1 para $2 por um temporário no mesmo diretório e um mv, para que o
# destino nunca fique pela metade e o halod em execução mantenha o binário
# antigo até o restart.
put() {
    mode=$1 src=$2 dest=$3
    install -m "$mode" "$src" "$dest.new"
    mv -f "$dest.new" "$dest"
}

# Gera $2 a partir do modelo $1, com @BINDIR@ trocado.
put_template() {
    sed "s|@BINDIR@|$BINDIR|g" "$1" > "$2.new"
    chmod 644 "$2.new"
    mv -f "$2.new" "$2"
}

check_udev() {
    src=$ROOT/packaging/udev/$RULE
    installed=$UDEV_DIR/$RULE
    if cmp -s "$src" "$installed"; then
        say "regra udev em dia ($installed)"
        return
    fi
    if [ -e "$installed" ]; then
        say "aviso: $installed está desatualizada em relação a $src."
    else
        say "aviso: a regra udev não está instalada ($installed)."
        say "O LED e a velocidade do Waterforce precisam dela; a telemetria não."
    fi
    say "Para instalar, rode (pede a sua senha):"
    printf '\n  sudo install -m 644 %s %s/\n' "$src" "$UDEV_DIR"
    printf '  sudo udevadm control --reload\n'
    printf '  sudo udevadm trigger --subsystem-match=hidraw --action=change\n\n'
}

do_install() {
    say "instalando para $(id -un): binários em $BINDIR"
    say "unidade em $UNIT_DIR, ativação D-Bus em $DBUS_DIR"

    # Compila antes de tocar no que já está instalado.
    cargo build --release --locked --manifest-path "$ROOT/Cargo.toml" \
        -p halo-cli -p halo-daemon ||
        die "a compilação falhou; nada instalado foi alterado"
    for bin in halo halod; do
        [ -x "$TARGET_DIR/release/$bin" ] ||
            die "compilação sem $TARGET_DIR/release/$bin; nada instalado foi alterado"
    done

    mkdir -p "$BINDIR" "$UNIT_DIR" "$DBUS_DIR"
    put 755 "$TARGET_DIR/release/halo" "$BINDIR/halo"
    put 755 "$TARGET_DIR/release/halod" "$BINDIR/halod"
    put_template "$ROOT/packaging/systemd/$UNIT" "$UNIT_FILE"
    put_template "$ROOT/packaging/dbus/$BUS_NAME.service" "$DBUS_FILE"

    systemctl --user daemon-reload
    systemctl --user enable "$UNIT"
    # Na primeira instalação inicia; numa atualização troca o binário em uso.
    systemctl --user restart "$UNIT" ||
        die "o $UNIT não subiu; veja journalctl --user -u $UNIT (um halod iniciado à mão segura o nome)"
    say "$UNIT ativo"

    case ":${PATH:-}:" in
        *":$BINDIR:"*) ;;
        *) say "aviso: $BINDIR não está no PATH; o comando halo não vai ser encontrado pelo nome" ;;
    esac

    check_udev
    say "pronto"
}

do_uninstall() {
    removed=0
    if [ -e "$UNIT_FILE" ]; then
        systemctl --user disable --now "$UNIT" ||
            say "aviso: não consegui parar e desabilitar o $UNIT"
    fi
    for f in "$UNIT_FILE" "$DBUS_FILE" "$BINDIR/halo" "$BINDIR/halod"; do
        if [ -e "$f" ]; then
            rm -f "$f"
            say "removido $f"
            removed=1
        fi
    done
    if [ "$removed" = 0 ]; then
        say "o halo não estava instalado; nada a remover"
        return
    fi
    systemctl --user daemon-reload
    say "perfis mantidos em $CONFIG_HOME/halo e snapshots em $STATE_HOME/halo"
    say "a regra udev, se instalada, fica em $UDEV_DIR/$RULE (ver docs/permissions.md)"
}

case "${1:-}" in
    '') [ $# -le 1 ] || { usage >&2; exit 2; }; do_install ;;
    --uninstall) [ $# -eq 1 ] || { usage >&2; exit 2; }; do_uninstall ;;
    --help | -h) usage ;;
    *) usage >&2; exit 2 ;;
esac
