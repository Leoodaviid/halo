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
# Uso: sh scripts/dev/nested-shell.sh [--daemon] [--profile]
#   --daemon   sobe um target/debug/halod no barramento do teste, com o sysfs
#              falso de scripts/dev/fake-sysfs.sh (o Castor3 em hidraw99, que
#              não existe): o carrossel mostra o Waterforce, e todo Led.Apply
#              falha com NotFound antes de chegar a um aparelho. Nenhum modo
#              lê o /sys real. Sem --daemon, o painel mostra o daemon fora do ar.
#   --profile  grava no XDG temporário um perfil "teste" com o LED tri-color e
#              o marca como ativo (para religar sem estado conhecido).
# Os Led.Apply do barramento do teste são registrados com o dbus-monitor e
# mostrados ao fechar a janela.

set -eu

UUID=halo@leoodaviid.github.io
ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
HALOD=${HALOD:-$ROOT/target/debug/halod}

daemon=no
profile=no
for arg in "$@"; do
    case "$arg" in
        --daemon) daemon=yes ;;
        --profile) profile=yes ;;
        *) echo "uso: sh scripts/dev/nested-shell.sh [--daemon] [--profile]" >&2; exit 2 ;;
    esac
done

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

# sysfs falso com o Castor3 num hidraw que não existe em /dev (hidraw99).
if [ "$daemon" = yes ]; then
    sh "$ROOT/scripts/dev/fake-sysfs.sh" "$T/sys" || exit 1
fi
if [ "$profile" = yes ]; then
    mkdir -p "$T/home/.config/halo/profiles" "$T/home/.local/state/halo"
    printf '[waterforce.led]\neffect = "tri-color"\ncolors = ["ff0000", "00ff00", "0000ff"]\nbrightness = 6\nspeed = 3\n' \
        > "$T/home/.config/halo/profiles/teste.toml"
    echo teste > "$T/home/.local/state/halo/active-profile"
fi

cat > "$T/inner.sh" <<'INNER'
set -u
UUID=$1
DAEMON=$2
HALOD=$3
T=$4
echo "$DBUS_SESSION_BUS_ADDRESS" > "$T/bus-address"
gsettings set org.gnome.shell disable-user-extensions false
gsettings set org.gnome.shell enabled-extensions "['$UUID']"
# Só observa: registra os Led.Apply deste barramento (o do dbus-run-session).
dbus-monitor --address "$DBUS_SESSION_BUS_ADDRESS" \
    "type='method_call',interface='io.github.leoodaviid.Halo1.Waterforce.Led',member='Apply'" \
    > "$T/applies.log" 2>/dev/null &
MONITOR_PID=$!
if [ "$DAEMON" = yes ]; then
    "$HALOD" --address "$DBUS_SESSION_BUS_ADDRESS" --sys-root "$T/sys" &
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
kill "$MONITOR_PID" 2>/dev/null
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
    dbus-run-session -- sh "$T/inner.sh" "$UUID" "$daemon" "$HALOD" "$T" > "$T/shell.log" 2>&1 &
RUN_PID=$!
i=0
while [ ! -s "$T/bus-address" ] && [ "$i" -lt 20 ]; do sleep 0.5; i=$((i + 1)); done
if [ -s "$T/bus-address" ]; then
    echo "halo: para contar os Led.Apply em outro terminal:"
    echo "  sh scripts/dev/watch-led-applies.sh --address '$(cat "$T/bus-address")'"
fi
# Ctrl+C encerra o Shell aninhado, mas a conferência abaixo roda mesmo assim.
trap 'kill "$RUN_PID" 2>/dev/null' INT TERM
wait "$RUN_PID" 2>/dev/null || true
wait "$RUN_PID" 2>/dev/null || true
trap - INT TERM

# Cada Apply: a linha do method call e os argumentos (efeito, cores, brilho,
# velocidade) numa linha só.
applies=$(awk '/^method call/ { if (line) print line; line = "Apply:"; next }
    line && /^ *(string|byte|array|\])/ { gsub(/^ +/, ""); line = line " " $0 }
    END { if (line) print line }' "$T/applies.log" 2>/dev/null || true)
if [ -n "$applies" ]; then
    echo "halo: Led.Apply no barramento do teste ($(printf '%s\n' "$applies" | wc -l)):"
    printf '%s\n' "$applies" | sed 's/^/  /'
else
    echo "halo: nenhum Led.Apply no barramento do teste"
fi

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
