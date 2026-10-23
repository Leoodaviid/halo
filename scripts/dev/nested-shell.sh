#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Abre um GNOME Shell aninhado (--devkit, do pacote mutter-dev-bin) numa
# janela, com a extensão do repositório, para ver e clicar no painel sem
# instalar nada. HOME, XDG_* e XDG_RUNTIME_DIR são temporários e o D-Bus é
# próprio (dbus-run-session): o dconf, as extensões e o /run/user da sessão
# real não são tocados. A exceção é o PipeWire: a janela do --devkit recebe a
# imagem do Shell aninhado por ele, então PIPEWIRE_RUNTIME_DIR aponta para o
# socket da sessão real. Ao fechar a janela, o script confere a sessão real.
#
# Uso: sh scripts/dev/nested-shell.sh [--daemon]
#   --daemon  sobe um target/debug/halod no barramento do teste, lendo o /sys
#             real (só leitura: o painel desta versão não envia nada).
#             Sem ele, o painel mostra o daemon fora do ar.

set -eu

UUID=halo@leoodaviid.github.io
ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
HALOD=${HALOD:-$ROOT/target/debug/halod}

daemon=no
case "${1:-}" in
    '') ;;
    --daemon) daemon=yes ;;
    *) echo "uso: sh scripts/dev/nested-shell.sh [--daemon]" >&2; exit 2 ;;
esac

[ -x /usr/libexec/mutter-devkit ] ||
    { echo "halo: falta o mutter-dev-bin (sudo apt install mutter-dev-bin)" >&2; exit 1; }
[ "$daemon" = no ] || [ -x "$HALOD" ] ||
    { echo "halo: $HALOD não existe; rode cargo build -p halo-daemon" >&2; exit 1; }

REAL_RUNTIME=/run/user/$(id -u)
REAL_MARKER=$REAL_RUNTIME/gnome-shell-disable-extensions
case "${WAYLAND_DISPLAY:-}" in
    '') echo "halo: sem WAYLAND_DISPLAY; rode numa sessão Wayland" >&2; exit 1 ;;
    /*) HOST_DISPLAY=$WAYLAND_DISPLAY ;;
    *) HOST_DISPLAY=$REAL_RUNTIME/$WAYLAND_DISPLAY ;;
esac

marker_before=no
[ -e "$REAL_MARKER" ] && marker_before=yes
sessions_before=$(loginctl list-sessions --no-legend 2>/dev/null | wc -l)

T=$(mktemp -d "${TMPDIR:-/tmp}/halo-nested.XXXXXX")
RUNTIME=$(mktemp -d /tmp/halo-rt.XXXXXX)
chmod 700 "$RUNTIME"
cleanup() {
    # O portal de documentos do Shell aninhado pode deixar $RUNTIME/doc montado.
    fusermount3 -u "$RUNTIME/doc" 2>/dev/null || true
    rm -rf "$T" "$RUNTIME" 2>/dev/null || true
}
trap cleanup EXIT

DEST=$T/home/.local/share/gnome-shell/extensions/$UUID
mkdir -p "$DEST"
(cd "$ROOT/extension" && tar --exclude=./tests -cf - .) | (cd "$DEST" && tar -xf -)

cat > "$T/inner.sh" <<'INNER'
set -u
UUID=$1
DAEMON=$2
HALOD=$3
gsettings set org.gnome.shell disable-user-extensions false
gsettings set org.gnome.shell enabled-extensions "['$UUID']"
if [ "$DAEMON" = yes ]; then
    "$HALOD" --address "$DBUS_SESSION_BUS_ADDRESS" &
    HALOD_PID=$!
fi
gnome-shell --devkit --wayland --no-x11 &
SHELL_PID=$!
# Fechar a janela do devkit não encerra o Shell aninhado: espera o
# visualizador (filho do Shell) aparecer e sumir, e então encerra o Shell.
i=0
while ! pgrep -P "$SHELL_PID" -x mutter-devkit > /dev/null && [ "$i" -lt 60 ]; do
    kill -0 "$SHELL_PID" 2>/dev/null || break
    sleep 0.5
    i=$((i + 1))
done
while pgrep -P "$SHELL_PID" -x mutter-devkit > /dev/null; do
    sleep 1
done
kill "$SHELL_PID" 2>/dev/null
wait "$SHELL_PID" 2>/dev/null
[ "$DAEMON" = yes ] && kill "$HALOD_PID" 2>/dev/null
exit 0
INNER

echo "halo: abrindo o Shell aninhado (feche a janela para terminar); log em $T/shell.log"
env -i PATH=/usr/bin:/bin LANG="${LANG:-C.UTF-8}" \
    HOME="$T/home" \
    XDG_CONFIG_HOME="$T/home/.config" \
    XDG_DATA_HOME="$T/home/.local/share" \
    XDG_STATE_HOME="$T/home/.local/state" \
    XDG_CACHE_HOME="$T/home/.cache" \
    XDG_RUNTIME_DIR="$RUNTIME" \
    WAYLAND_DISPLAY="$HOST_DISPLAY" \
    PIPEWIRE_RUNTIME_DIR="$REAL_RUNTIME" \
    dbus-run-session -- sh "$T/inner.sh" "$UUID" "$daemon" "$HALOD" > "$T/shell.log" 2>&1 &
RUN_PID=$!
# Ctrl+C encerra o Shell aninhado, mas a conferência abaixo roda mesmo assim.
trap 'kill "$RUN_PID" 2>/dev/null' INT TERM
wait "$RUN_PID" 2>/dev/null || true
wait "$RUN_PID" 2>/dev/null || true
trap - INT TERM

if grep -q 'Failed to connect pipewire' "$T/shell.log"; then
    echo "halo: a janela do devkit não conseguiu falar com o PipeWire ($REAL_RUNTIME/pipewire-0)"
fi

errors=$(grep -E 'JS ERROR|Gjs-CRITICAL|halo@leoodaviid' "$T/shell.log" || true)
if [ -n "$errors" ]; then
    echo "halo: erros da extensão no log do Shell aninhado:"
    printf '%s\n' "$errors" | sed 's/^/  /'
else
    echo "halo: nenhum erro da extensão no log"
fi

status=0
if [ "$marker_before" = no ] && [ -e "$REAL_MARKER" ]; then
    echo "halo: ATENÇÃO: $REAL_MARKER apareceu; a sessão real desligaria as extensões no próximo início"
    status=1
else
    echo "halo: a sessão real não ganhou o marcador de crash"
fi
sessions_after=$(loginctl list-sessions --no-legend 2>/dev/null | wc -l)
if [ "$sessions_after" != "$sessions_before" ]; then
    echo "halo: ATENÇÃO: o número de sessões no loginctl mudou ($sessions_before -> $sessions_after)"
    status=1
else
    echo "halo: nenhuma sessão nova no loginctl"
fi
exit "$status"
