#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Teste de fumaça da extensão: carrega a extensão do repositório num GNOME
# Shell headless, com HOME, XDG_* e XDG_RUNTIME_DIR temporários e um D-Bus
# próprio (dbus-run-session). Confere que ela fica ACTIVE, que sobrevive a um
# disable/enable e que não deixa JS ERROR no log, sem o halod e com um halod
# de teste (target/debug/halod, com um sysfs falso) no barramento do teste.
# Confere também que a extensão não ativa o halod: o barramento do teste tem
# um arquivo de ativação que só registra se foi usado. Nada da sessão do
# usuário é tocado: nem o dconf, nem as extensões, nem o /run/user dele, nem
# os aparelhos (o sysfs é falso). Antes do Shell, um Led.Apply ao halod de
# teste precisa voltar NotFound: o sysfs falso não leva a nenhum nó real.
#
# Uso: sh scripts/dev/headless-smoke.sh   (HALOD= troca o binário do halod)

set -eu

UUID=halo@leoodaviid.github.io
ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
HALOD=${HALOD:-$ROOT/target/debug/halod}
[ -x "$HALOD" ] || { echo "halo: $HALOD não existe; rode cargo build -p halo-daemon" >&2; exit 1; }

# O marcador de crash do Shell da sessão real: um Shell de teste que use o
# /run/user do usuário o cria, e a sessão desliga todas as extensões no
# próximo início.
REAL_MARKER=/run/user/$(id -u)/gnome-shell-disable-extensions
marker_before=no
[ -e "$REAL_MARKER" ] && marker_before=yes

T=$(mktemp -d "${TMPDIR:-/tmp}/halo-smoke.XXXXXX")
RUNTIME=$(mktemp -d /tmp/halo-rt.XXXXXX)
chmod 700 "$RUNTIME"
cleanup() {
    # O portal de documentos do Shell de teste pode deixar $RUNTIME/doc montado.
    fusermount3 -u "$RUNTIME/doc" 2>/dev/null || true
    rm -rf "$T" "$RUNTIME" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 130' INT TERM

DEST=$T/home/.local/share/gnome-shell/extensions/$UUID
mkdir -p "$DEST"
(cd "$ROOT/extension" && tar --exclude=./tests -cf - .) | (cd "$DEST" && tar -xf -)
# O getSettings() do enable() precisa do schema compilado na pasta da extensão.
rm -f "$DEST/schemas/gschemas.compiled"
glib-compile-schemas --strict "$DEST/schemas" || exit 1

# sysfs falso com o Castor3 num hidraw que não existe em /dev (hidraw99).
sh "$ROOT/scripts/dev/fake-sysfs.sh" "$T/sys" || exit 1

# Ativação no barramento do teste: só registra que foi usada.
SERVICES=$T/home/.local/share/dbus-1/services
mkdir -p "$SERVICES"
printf '#!/bin/sh\ntouch %s\nexit 1\n' "$T/activated" > "$T/activate.sh"
chmod 755 "$T/activate.sh"
printf '[D-BUS Service]\nName=io.github.leoodaviid.Halo1\nExec=%s\n' "$T/activate.sh" \
    > "$SERVICES/io.github.leoodaviid.Halo1.service"

cat > "$T/inner.sh" <<'INNER'
set -u
UUID=$1
LOG=$2
T=$3
HALOD=$4
gsettings set org.gnome.shell disable-user-extensions false
gsettings set org.gnome.shell enabled-extensions "['$UUID']"

has_owner() {
    gdbus call --session -d org.freedesktop.DBus -o /org/freedesktop/DBus \
        -m org.freedesktop.DBus.NameHasOwner io.github.leoodaviid.Halo1 2>/dev/null |
        grep -q true
}

# Antes do Shell: um Led.Apply ao halod de teste precisa voltar NotFound. Prova
# que o sysfs falso não leva a nenhum nó real: o halod falha ao abrir
# /dev/hidraw99, antes do 99 DE 00, e nada chega a um aparelho.
status=0
"$HALOD" --address "$DBUS_SESSION_BUS_ADDRESS" --sys-root "$T/sys" > "$T/halod-probe.log" 2>&1 &
PROBE_PID=$!
i=0
while ! has_owner && [ "$i" -lt 40 ]; do sleep 0.25; i=$((i + 1)); done
reply=$(gdbus call --session -d io.github.leoodaviid.Halo1 -o /io/github/leoodaviid/Halo1/waterforce \
    -m io.github.leoodaviid.Halo1.Waterforce.Led.Apply cycle '@as []' 0 0 2>&1)
case "$reply" in
    *io.github.leoodaviid.Halo1.Error.NotFound*) echo "ok    Apply de teste volta NotFound (nenhum nó real)" ;;
    *) echo "FALHA Apply de teste não voltou NotFound: $reply"; status=1 ;;
esac
kill "$PROBE_PID" 2>/dev/null
wait "$PROBE_PID" 2>/dev/null
i=0
while has_owner && [ "$i" -lt 40 ]; do sleep 0.25; i=$((i + 1)); done

# Daqui em diante, qualquer Led.Apply no barramento vem da extensão, e sem
# gesto nenhum ela não pode mandar nenhum.
dbus-monitor --address "$DBUS_SESSION_BUS_ADDRESS" \
    "type='method_call',interface='io.github.leoodaviid.Halo1.Waterforce.Led',member='Apply'" \
    > "$T/applies.log" 2>/dev/null &
MONITOR_PID=$!

gnome-shell --headless --wayland --no-x11 --virtual-monitor 1280x800 > "$LOG" 2>&1 &
SHELL_PID=$!

state() {
    gnome-extensions info "$UUID" 2>/dev/null | sed -n 's/^ *State: *//p'
}
# Espera o estado $1 por até $2 segundos.
wait_state() {
    i=0
    while [ "$i" -lt "$(($2 * 4))" ]; do
        [ "$(state)" = "$1" ] && return 0
        sleep 0.25
        i=$((i + 1))
    done
    return 1
}

if wait_state ACTIVE 30; then echo "ok    extensão ACTIVE ao iniciar"
else echo "FALHA extensão não ficou ACTIVE ao iniciar (estado: $(state))"; status=1; fi

# Sem o halod: a extensão observa o nome, mas não o ativa.
sleep 10
if [ ! -e "$T/activated" ]; then echo "ok    sem o halod, a extensão não usou a ativação"
else echo "FALHA a extensão ativou o halod pelo D-Bus"; status=1; fi
if ! has_owner; then echo "ok    sem o halod, o nome continua sem dono"
else echo "FALHA o nome ganhou dono sem ninguém iniciar o halod"; status=1; fi

# O halod aparece (como depois de "Iniciar halod").
"$HALOD" --address "$DBUS_SESSION_BUS_ADDRESS" --sys-root "$T/sys" > "$T/halod.log" 2>&1 &
HALOD_PID=$!
i=0
while ! has_owner && [ "$i" -lt 40 ]; do sleep 0.25; i=$((i + 1)); done
sleep 2
if has_owner && [ "$(state)" = ACTIVE ]; then echo "ok    com o halod, a extensão continua ACTIVE"
else echo "FALHA com o halod (dono: $(has_owner && echo sim || echo não), estado: $(state))"; status=1; fi

# A fonte da barra muda pelo gsettings, com o schema da extensão, e a
# extensão segue ACTIVE (os erros do sinal aparecem no log do Shell).
SCHEMAS=$T/home/.local/share/gnome-shell/extensions/$UUID/schemas
for source in cpu coolant; do
    GSETTINGS_SCHEMA_DIR=$SCHEMAS gsettings set org.gnome.shell.extensions.halo temperature-source "$source"
    sleep 1
done
if [ "$(GSETTINGS_SCHEMA_DIR=$SCHEMAS gsettings get org.gnome.shell.extensions.halo temperature-source)" = "'coolant'" ] &&
    [ "$(state)" = ACTIVE ]; then echo "ok    troca da fonte para a CPU e de volta, extensão ACTIVE"
else echo "FALHA troca da fonte (estado: $(state))"; status=1; fi

gnome-extensions disable "$UUID"
if wait_state INACTIVE 10; then echo "ok    disable deixa INACTIVE"
else echo "FALHA disable não deixou INACTIVE (estado: $(state))"; status=1; fi

gnome-extensions enable "$UUID"
if wait_state ACTIVE 10; then echo "ok    enable volta a ACTIVE"
else echo "FALHA enable não voltou a ACTIVE (estado: $(state))"; status=1; fi

# O halod sai (como depois de um systemctl stop).
sleep 2
kill "$HALOD_PID" 2>/dev/null
wait "$HALOD_PID" 2>/dev/null
sleep 3
if [ "$(state)" = ACTIVE ] && ! has_owner && [ ! -e "$T/activated" ]; then
    echo "ok    o halod saiu e a extensão continua ACTIVE sem reativá-lo"
else echo "FALHA depois de o halod sair (estado: $(state))"; status=1; fi

sleep 1
kill "$SHELL_PID" 2>/dev/null
wait "$SHELL_PID" 2>/dev/null
kill "$MONITOR_PID" 2>/dev/null
applies=$(grep -c 'member=Apply' "$T/applies.log" 2>/dev/null || true)
if [ "${applies:-0}" -eq 0 ]; then echo "ok    a extensão não mandou nenhum Led.Apply sem gesto"
else echo "FALHA a extensão mandou $applies Led.Apply sem gesto"; status=1; fi
exit "$status"
INNER

set +e
env -i PATH=/usr/bin:/bin LANG="${LANG:-C.UTF-8}" \
    HOME="$T/home" \
    XDG_CONFIG_HOME="$T/home/.config" \
    XDG_DATA_HOME="$T/home/.local/share" \
    XDG_STATE_HOME="$T/home/.local/state" \
    XDG_CACHE_HOME="$T/home/.cache" \
    XDG_RUNTIME_DIR="$RUNTIME" \
    timeout 120 dbus-run-session -- sh "$T/inner.sh" "$UUID" "$T/shell.log" "$T" "$HALOD" 2>/dev/null
status=$?
set -e

# Os console.warn da extensão começam com "halo:" (proxy que não foi criado,
# Devices que falhou...): também contam como erro.
if grep -E 'JS ERROR|Gjs-CRITICAL|halo@leoodaviid|halo: ' "$T/shell.log" > "$T/errors" 2>/dev/null &&
    [ -s "$T/errors" ]; then
    echo "FALHA erros no log do Shell:"
    sed 's/^/      /' "$T/errors"
    status=1
else
    echo "ok    nenhum JS ERROR no log do Shell"
fi

if [ "$marker_before" = no ] && [ -e "$REAL_MARKER" ]; then
    echo "FALHA $REAL_MARKER apareceu durante o teste"
    status=1
else
    echo "ok    a sessão real não ganhou o marcador de crash"
fi

if [ "$status" -ne 0 ]; then
    echo "o teste de fumaça falhou"
    exit 1
fi
echo "todas as verificações passaram"
