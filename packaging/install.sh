#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Instala o halo no usuário, sem root: compila o halo e o halod, instala os
# binários, a unidade do systemd do usuário e a ativação pelo D-Bus, e sobe o
# daemon. Instala também a extensão do GNOME Shell e a habilita pelo
# gsettings. Com --uninstall, remove só o que instalou e mantém perfis e
# snapshots. A regra udev é só conferida: os comandos com sudo são mostrados,
# nunca executados.
#
# Caminhos (para testes): BINDIR, XDG_CONFIG_HOME, XDG_DATA_HOME,
# XDG_STATE_HOME, CARGO_TARGET_DIR, HALO_UDEV_RULES_DIR e HALO_EXTENSION_SRC
# (a pasta da extensão a instalar; padrão extension/ do repositório).

set -eu

UNIT=halod.service
BUS_NAME=io.github.leoodaviid.Halo1
RULE=70-halo.rules
UUID=halo@leoodaviid.github.io

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
  extensão   \$XDG_DATA_HOME/gnome-shell/extensions/$UUID
e habilita e (re)inicia o $UNIT. A extensão do GNOME é habilitada pelo
gsettings e carrega depois de sair e entrar na sessão. Nada é escrito fora
do \$HOME.

  --uninstall  para e desabilita o daemon, desabilita a extensão e remove
               os arquivos acima. Perfis e snapshots ficam.
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
EXT_SRC=${HALO_EXTENSION_SRC:-$ROOT/extension}

UNIT_DIR=$CONFIG_HOME/systemd/user
DBUS_DIR=$DATA_HOME/dbus-1/services
UNIT_FILE=$UNIT_DIR/$UNIT
DBUS_FILE=$DBUS_DIR/$BUS_NAME.service
EXT_DIR=$DATA_HOME/gnome-shell/extensions/$UUID

case $BINDIR in
    /*) ;;
    *) die "BINDIR precisa ser um caminho absoluto: $BINDIR" ;;
esac
case $BINDIR in
    *'|'* | *'&'* | *'\'*) die "BINDIR não pode ter |, & nem \\: $BINDIR" ;;
esac

# Copia $1 para $2 por um temporário no mesmo diretório e um mv, para que o
# destino nunca fique pela metade.
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

# Lista de extensões habilitadas, como o gsettings devolve: '@as []' ou
# ['a', 'b']. Qualquer outro texto não é editado.
ext_list_ok() {
    printf '%s\n' "$1" | grep -qE "^(@as \\[\\]|\\[('[^']*'(, '[^']*')*)?\\])\$"
}
ext_list_has() {
    case $1 in
        *"'$UUID'"*) return 0 ;;
        *) return 1 ;;
    esac
}
# Acrescenta o UUID no fim, sem mexer nas outras.
ext_list_add() {
    case $1 in
        '@as []' | '[]') printf "['%s']" "$UUID" ;;
        *) printf '%s' "$1" | sed "s/]\$/, '$UUID']/" ;;
    esac
}
# Tira o UUID e a vírgula dele; lista que fica vazia vira '@as []'.
ext_list_remove() {
    re=$(printf '%s' "$UUID" | sed 's/\./\\./g')
    left=$(printf '%s' "$1" | sed "s/'$re', //g; s/, '$re'//g; s/'$re'//g")
    if [ "$left" = '[]' ]; then printf '@as []'; else printf '%s' "$left"; fi
}

enable_later() {
    say "Para habilitar depois, saia e entre na sessão e rode:"
    printf '\n  gnome-extensions enable %s\n\n' "$UUID"
}

# Põe o UUID em org.gnome.shell enabled-extensions. Não usa o
# gnome-extensions enable: com o Shell já rodando, ele não conhece a extensão
# recém-copiada e falha. Nada aqui interrompe a instalação.
enable_extension() {
    if ! list=$(gsettings get org.gnome.shell enabled-extensions 2>/dev/null); then
        say "aviso: não consegui ler as extensões habilitadas do GNOME (sem sessão gráfica?)"
        enable_later
        return
    fi
    if ext_list_has "$list"; then
        say "extensão já habilitada no GNOME"
    elif ! ext_list_ok "$list"; then
        say "aviso: formato inesperado da lista de extensões habilitadas; não alterei: $list"
        enable_later
    elif gsettings set org.gnome.shell enabled-extensions "$(ext_list_add "$list")" 2>/dev/null &&
        # Sem barramento de sessão o gsettings pode gravar só na memória e
        # não falhar; relê para ter certeza de que ficou.
        list=$(gsettings get org.gnome.shell enabled-extensions 2>/dev/null) &&
        ext_list_has "$list"; then
        say "extensão habilitada no GNOME"
    else
        say "aviso: não consegui gravar a lista de extensões habilitadas do GNOME"
        enable_later
    fi
    if [ "$(gsettings get org.gnome.shell disable-user-extensions 2>/dev/null || true)" = true ]; then
        say "aviso: as extensões do usuário estão desligadas no GNOME (disable-user-extensions);"
        say "ligue-as no app Extensões para a do halo aparecer"
    fi
}

# Copia a extensão sem tests/ nem arquivos ocultos ou de backup de editor
# para <dest>.new e só troca a instalada se algo mudou, para não pedir à toa
# que o usuário saia da sessão.
install_extension() {
    staging=$EXT_DIR.new
    rm -rf "$staging"
    mkdir -p "$staging"
    cp -R "$EXT_SRC/." "$staging/"
    find "$staging" -mindepth 1 \( -path "$staging/tests" -o -name '.*' -o -name '*~' \) \
        -prune -exec rm -rf {} +
    if diff -r "$staging" "$EXT_DIR" > /dev/null 2>&1; then
        rm -rf "$staging"
        changed=0
        say "extensão em dia ($EXT_DIR)"
    else
        rm -rf "$EXT_DIR"
        mv "$staging" "$EXT_DIR"
        changed=1
        say "extensão instalada em $EXT_DIR"
    fi
    enable_extension
    if [ "$changed" = 1 ]; then
        say "aviso: a extensão mudou; saia e entre na sessão para carregar a extensão"
    fi
}

# O halod em $1 conhece o modo de proteção do ExecStopPost.
has_safe() {
    "$1" --help 2>/dev/null | grep -q -- --waterforce-cpu-safe
}

do_install() {
    say "instalando para $(id -un): binários em $BINDIR"
    say "unidade em $UNIT_DIR, ativação D-Bus em $DBUS_DIR"
    [ -f "$EXT_SRC/metadata.json" ] ||
        die "extensão sem metadata.json em $EXT_SRC; nada instalado foi alterado"

    # Compila antes de tocar no que já está instalado.
    cargo build --release --locked --manifest-path "$ROOT/Cargo.toml" \
        -p halo-cli -p halo-daemon ||
        die "a compilação falhou; nada instalado foi alterado"
    for bin in halo halod; do
        [ -x "$TARGET_DIR/release/$bin" ] ||
            die "compilação sem $TARGET_DIR/release/$bin; nada instalado foi alterado"
    done
    # A unidade nova chama o helper no ExecStopPost: o binário precisa ter.
    has_safe "$TARGET_DIR/release/halod" ||
        die "o halod compilado não tem --waterforce-cpu-safe; nada instalado foi alterado"

    # Para a unidade instalada antes de trocar qualquer arquivo: o stop roda
    # o ExecStopPost do par antigo (binário e unidade compatíveis), que
    # deixa o X 240 com E0 de 80 °C. Só então o par novo entra.
    if [ -e "$UNIT_FILE" ]; then
        systemctl --user stop "$UNIT" ||
            die "não consegui parar o $UNIT; nada instalado foi alterado"
    fi

    mkdir -p "$BINDIR" "$UNIT_DIR" "$DBUS_DIR"
    put 755 "$TARGET_DIR/release/halo" "$BINDIR/halo"
    put 755 "$TARGET_DIR/release/halod" "$BINDIR/halod"
    put_template "$ROOT/packaging/systemd/$UNIT" "$UNIT_FILE"
    put_template "$ROOT/packaging/dbus/$BUS_NAME.service" "$DBUS_FILE"
    has_safe "$BINDIR/halod" ||
        die "o halod instalado em $BINDIR não tem --waterforce-cpu-safe; o $UNIT ficou parado"

    systemctl --user daemon-reload
    systemctl --user enable "$UNIT"
    systemctl --user start "$UNIT" ||
        die "o $UNIT não subiu; veja journalctl --user -u $UNIT (um halod iniciado à mão segura o nome)"
    say "$UNIT ativo"

    case ":${PATH:-}:" in
        *":$BINDIR:"*) ;;
        *) say "aviso: $BINDIR não está no PATH; o comando halo não vai ser encontrado pelo nome" ;;
    esac

    # Depois do daemon: a extensão só faz sentido com o halod instalado, e uma
    # falha no restart acima para tudo antes de tocar na extensão.
    install_extension

    check_udev
    say "pronto"
}

# Tira o UUID da lista de habilitadas, sem mexer nas outras. Devolve 0 se
# tirou.
disable_extension() {
    if ! list=$(gsettings get org.gnome.shell enabled-extensions 2>/dev/null); then
        say "aviso: não consegui ler as extensões habilitadas do GNOME; se $UUID estiver lá, rode:"
        printf '\n  gnome-extensions disable %s\n\n' "$UUID"
        return 1
    fi
    ext_list_has "$list" || return 1
    if ext_list_ok "$list" &&
        gsettings set org.gnome.shell enabled-extensions "$(ext_list_remove "$list")" 2>/dev/null; then
        say "extensão desabilitada no GNOME"
        return 0
    fi
    say "aviso: não consegui tirar $UUID da lista de extensões habilitadas; rode:"
    printf '\n  gnome-extensions disable %s\n\n' "$UUID"
    return 1
}

do_uninstall() {
    removed=0
    ext_removed=0
    # Para antes de remover: o ExecStopPost ainda acha o halod instalado e
    # deixa o X 240 com E0 de 80 °C.
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
    # Primeiro sai da lista, para o Shell desligar a extensão com os arquivos
    # ainda no lugar; depois a pasta.
    if disable_extension; then
        ext_removed=1
    fi
    if [ -e "$EXT_DIR" ]; then
        rm -rf "$EXT_DIR"
        say "removido $EXT_DIR"
        ext_removed=1
    fi
    if [ "$removed" = 0 ] && [ "$ext_removed" = 0 ]; then
        say "o halo não estava instalado; nada a remover"
        return
    fi
    if [ "$removed" = 1 ]; then
        systemctl --user daemon-reload
    fi
    say "perfis mantidos em $CONFIG_HOME/halo e snapshots em $STATE_HOME/halo"
    say "a regra udev, se instalada, fica em $UDEV_DIR/$RULE (ver docs/permissions.md)"
}

case "${1:-}" in
    '') [ $# -le 1 ] || { usage >&2; exit 2; }; do_install ;;
    --uninstall) [ $# -eq 1 ] || { usage >&2; exit 2; }; do_uninstall ;;
    --help | -h) usage ;;
    *) usage >&2; exit 2 ;;
esac
