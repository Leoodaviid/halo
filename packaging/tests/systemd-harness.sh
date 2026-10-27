#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Testa no systemd do usuário o ciclo de vida da packaging/systemd/halod.service
# (ExecStopPost, watchdog, Restart e limite de inícios), com um halod falso
# em Python que não abre nenhum aparelho: ele só pega um nome D-Bus próprio,
# manda WATCHDOG=1 e registra o que acontece. O helper falso registra cada
# execução do ExecStopPost com o SERVICE_RESULT do systemd.
#
# A unidade de teste é gerada da unidade real (mesmos Type, watchdog,
# TimeoutStopSec, Restart e StartLimit), com outro nome, outro BusName e um
# alvo de teste no lugar do graphical-session.target. Fica em
# $XDG_RUNTIME_DIR/systemd/user e é removida no fim. Usa o gerenciador do
# usuário em execução (daemon-reload, start, stop, kill), sem tocar no
# halod.service nem na sessão gráfica. Leva uns 3 minutos.
#
# Uso: sh packaging/tests/systemd-harness.sh

set -eu

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
[ -n "${XDG_RUNTIME_DIR:-}" ] || { echo "sem XDG_RUNTIME_DIR" >&2; exit 2; }

ID=halo-harness-$$
UNIT=$ID.service
TARGET=$ID.target
UNIT_DIR=$XDG_RUNTIME_DIR/systemd/user
T=$(mktemp -d "${TMPDIR:-/tmp}/halo-harness.XXXXXX")
LOG=$T/log
MODE=$T/mode

cleanup() {
    systemctl --user stop "$UNIT" "$TARGET" > /dev/null 2>&1 || true
    systemctl --user reset-failed "$UNIT" > /dev/null 2>&1 || true
    rm -f "$UNIT_DIR/$UNIT" "$UNIT_DIR/$TARGET"
    systemctl --user daemon-reload > /dev/null 2>&1 || true
    rm -rf "$T"
}
trap cleanup EXIT INT TERM

failures=0
ok() { printf 'ok    %s\n' "$1"; }
fail() { printf 'FALHA %s\n' "$1"; failures=$((failures + 1)); }
check() {
    name=$1
    shift
    if "$@"; then ok "$name"; else fail "$name"; fi
}

mkdir -p "$T/bin" "$UNIT_DIR"
cat > "$T/bin/halod" <<'EOF'
#!/usr/bin/python3 -I
import os, signal, socket, sys, time

d = os.environ["HARNESS_DIR"]

def log(text):
    with open(os.path.join(d, "log"), "a") as f:
        f.write(f"{time.monotonic():.3f} {text}\n")

mode = open(os.path.join(d, "mode")).read().strip()

if "--waterforce-cpu-safe" in sys.argv:
    log(f"helper {os.environ.get('SERVICE_RESULT', '?')} {os.environ.get('EXIT_CODE', '?')}")
    # Na falha no início, a espera real de 8 s do helper com o X 240
    # presente: é ela que deixaria o limite padrão de inícios sem efeito.
    time.sleep(8 if mode == "startfail" else 1)
    sys.exit(0)

if mode == "startfail":
    log("main startfail")
    sys.exit(1)

import gi
gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib

bus = Gio.bus_get_sync(Gio.BusType.SESSION)
bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
              "RequestName", GLib.Variant("(su)", (os.environ["HARNESS_NAME"], 4)),
              None, Gio.DBusCallFlags.NONE, -1, None)
log(f"main {os.getpid()} {mode}")

def notify(msg):
    addr = os.environ.get("NOTIFY_SOCKET", "")
    if addr.startswith("@"):
        addr = "\0" + addr[1:]
    s = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
    s.sendto(msg.encode(), addr)

def term(*_):
    log("main sigterm")
    sys.exit(0)

signal.signal(signal.SIGTERM, term)
started = time.monotonic()
while True:
    if mode == "panic" and time.monotonic() - started > 3:
        log("main panic")
        os._exit(101)
    if mode != "hang":
        notify("WATCHDOG=1")
    time.sleep(2)
EOF
chmod 755 "$T/bin/halod"

BUS=io.github.leoodaviid.HaloHarness.p$$
sed -e "s|@BINDIR@|$T/bin|g" \
    -e "s|^BusName=.*|BusName=$BUS|" \
    -e "s|graphical-session.target|$TARGET|g" \
    -e "s|^\[Service\]|[Service]\nEnvironment=HARNESS_DIR=$T HARNESS_NAME=$BUS|" \
    "$ROOT/packaging/systemd/halod.service" > "$UNIT_DIR/$UNIT"
printf '[Unit]\nDescription=alvo do harness do halo\n' > "$UNIT_DIR/$TARGET"
systemctl --user daemon-reload

prop() { systemctl --user show -p "$1" --value "$UNIT"; }
count() { grep -c "$1" "$LOG" 2>/dev/null || true; }
# wait_for <segundos> <comando...>
wait_for() {
    limit=$1
    shift
    i=0
    while ! "$@"; do
        i=$((i + 1))
        [ "$i" -le $((limit * 5)) ] || return 1
        sleep 0.2
    done
}
active() { [ "$(prop ActiveState)" = active ]; }
helpers_at_least() { [ "$(count ' helper ')" -ge "$1" ]; }
mains_at_least() { [ "$(count ' main [0-9]')" -ge "$1" ]; }
helper_after_main() {
    # O último helper veio depois do último registro do principal.
    last_main=$(grep ' main ' "$LOG" | tail -n 1 | cut -d' ' -f1)
    last_helper=$(grep ' helper ' "$LOG" | tail -n 1 | cut -d' ' -f1)
    awk -v a="$last_main" -v b="$last_helper" 'BEGIN { exit !(b > a) }'
}
fresh() {
    systemctl --user stop "$UNIT" "$TARGET" > /dev/null 2>&1 || true
    systemctl --user reset-failed "$UNIT" > /dev/null 2>&1 || true
    : > "$LOG"
    printf '%s\n' "$1" > "$MODE"
}

check "unidade gerada passa no systemd-analyze verify" \
    systemd-analyze --user verify "$UNIT_DIR/$UNIT"

# Stop explícito: helper roda, sem restart.
fresh normal
systemctl --user start "$UNIT"
check "stop: principal sobe com o nome D-Bus" wait_for 10 mains_at_least 1
systemctl --user stop "$UNIT"
check "stop: helper executado" helpers_at_least 1
check "stop: SIGTERM antes do helper" sh -c "grep -q 'main sigterm' '$LOG' && grep ' helper ' '$LOG' | grep -q success"
sleep 3
check "stop: sem restart" [ "$(count ' main [0-9]')" = 1 ]

# SIGKILL só no principal (como OOM ou kill do PID): helper e depois um
# principal novo. Sem --kill-whom=main, o SIGKILL vai ao cgroup inteiro e
# pode matar também o helper que o systemd acabou de iniciar.
fresh normal
systemctl --user start "$UNIT"
wait_for 10 mains_at_least 1
systemctl --user kill --kill-whom=main -s KILL "$UNIT"
check "SIGKILL: helper executado" wait_for 10 helpers_at_least 1
check "SIGKILL: helper com SERVICE_RESULT=signal" sh -c "grep ' helper ' '$LOG' | grep -q 'signal'"
check "SIGKILL: principal novo depois do helper" wait_for 15 mains_at_least 2
check "SIGKILL: NRestarts = 1" [ "$(prop NRestarts)" = 1 ]

# Panic (saída 101): helper e restart.
fresh panic
systemctl --user start "$UNIT"
check "panic: helper executado" wait_for 15 helpers_at_least 1
check "panic: helper com SERVICE_RESULT=exit-code" sh -c "grep ' helper ' '$LOG' | grep -q 'exit-code'"
check "panic: principal novo" wait_for 15 mains_at_least 2

# SIGSTOP no principal: o watchdog (20 s) mata com SIGKILL e o helper roda.
fresh normal
systemctl --user start "$UNIT"
wait_for 10 mains_at_least 1
systemctl --user kill --kill-whom=main -s STOP "$UNIT"
check "STOP: helper executado pelo watchdog" wait_for 35 helpers_at_least 1
check "STOP: helper com SERVICE_RESULT=watchdog" sh -c "grep ' helper ' '$LOG' | grep -q watchdog"
check "STOP: principal novo" wait_for 15 mains_at_least 2

# Principal sem progresso (pings param, processo vivo): watchdog.
fresh hang
systemctl --user start "$UNIT"
check "sem pings: helper pelo watchdog" wait_for 35 helpers_at_least 1
check "sem pings: SERVICE_RESULT=watchdog" sh -c "grep ' helper ' '$LOG' | grep -q watchdog"
systemctl --user stop "$UNIT"

# Falha no início, seguidas, com o helper esperando 8 s: o helper roda em
# cada uma e a unidade desiste pelo StartLimit da unidade.
fresh startfail
systemctl --user start "$UNIT" > /dev/null 2>&1 || true
# O Result fica exit-code; desistir é parar em failed e não tentar mais.
check "falha no início: unidade para em failed" \
    wait_for 120 sh -c "[ \"\$(systemctl --user show -p ActiveState --value '$UNIT')\" = failed ]"
attempts=$(count 'main startfail')
sleep 5
check "falha no início: nenhuma tentativa depois de desistir" [ "$(count 'main startfail')" = "$attempts" ]
check "falha no início: helper em cada tentativa" [ "$(count ' helper ')" = "$(count 'main startfail')" ]
check "falha no início: helper depois da última tentativa" helper_after_main
check "falha no início: 5 tentativas" [ "$(count 'main startfail')" = 5 ]

# Parada pelo PartOf: helper roda, sem restart.
fresh normal
systemctl --user start "$TARGET" "$UNIT"
wait_for 10 mains_at_least 1
systemctl --user stop "$TARGET"
check "PartOf: unidade parada" wait_for 25 sh -c "[ \"\$(systemctl --user show -p ActiveState --value '$UNIT')\" = inactive ]"
check "PartOf: helper executado" helpers_at_least 1
sleep 3
check "PartOf: sem restart" [ "$(count ' main [0-9]')" = 1 ]

if [ "$failures" -ne 0 ]; then
    printf '%d verificação(ões) falharam\n' "$failures"
    exit 1
fi
echo "todas as verificações passaram"
