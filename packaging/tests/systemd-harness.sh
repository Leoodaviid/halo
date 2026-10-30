#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Testa no systemd do usuário o ciclo de vida da packaging/systemd/halod.service
# (ExecStopPost, watchdog, Restart e limite de inícios) e da guarda
# packaging/systemd/halod-session.service (a barreira do logout), com um
# halod falso em Python que não abre nenhum aparelho: ele só pega um nome
# D-Bus próprio, responde ao Session.Begin()/End() nesse nome, manda
# WATCHDOG=1 e registra o que acontece. O helper falso registra cada
# execução, do ExecStopPost (com o SERVICE_RESULT do systemd) ou da guarda.
# O `--session` falso fala só com o nome do harness, nunca com o halod real.
#
# As unidades de teste são geradas das reais (mesmos Type, watchdog,
# TimeoutStopSec, Restart, StartLimit, PartOf e After), com outros nomes,
# outro BusName e um alvo de teste no lugar do graphical-session.target.
# Ficam em $XDG_RUNTIME_DIR/systemd/user e são removidas no fim. Usa o
# gerenciador do usuário em execução (daemon-reload, start, stop, kill), sem
# tocar no halod.service nem na sessão gráfica. Leva uns 4 minutos.
#
# Uso: sh packaging/tests/systemd-harness.sh
# HARNESS_DROP_AFTER=1 gera a guarda sem o After= do alvo, para conferir que
# os testes da barreira falham sem ele. HARNESS_TYPE_DBUS=1 gera a unidade com
# Type=dbus, para conferir que o caso "nome perdido" falha com ele.
#
# O barramento da sessão real não pode ser reiniciado sem derrubar a sessão.
# O caso "nome perdido" cobre o que importa da queda no logout: quando o nome
# do BusName fica sem dono com a unidade rodando, o systemd reage do mesmo
# jeito que ao rever o nome com o barramento de volta.

set -eu

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
[ -n "${XDG_RUNTIME_DIR:-}" ] || { echo "sem XDG_RUNTIME_DIR" >&2; exit 2; }

ID=halo-harness-$$
UNIT=$ID.service
GUARD=$ID-session.service
TARGET=$ID.target
UNIT_DIR=$XDG_RUNTIME_DIR/systemd/user
T=$(mktemp -d "${TMPDIR:-/tmp}/halo-harness.XXXXXX")
LOG=$T/log
MODE=$T/mode

cleanup() {
    systemctl --user stop "$GUARD" "$UNIT" "$TARGET" > /dev/null 2>&1 || true
    systemctl --user reset-failed "$UNIT" "$GUARD" > /dev/null 2>&1 || true
    rm -f "$UNIT_DIR/$UNIT" "$UNIT_DIR/$GUARD" "$UNIT_DIR/$TARGET"
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
name = os.environ["HARNESS_NAME"]

def log(text):
    with open(os.path.join(d, "log"), "a") as f:
        f.write(f"{time.monotonic():.3f} {text}\n")

mode = open(os.path.join(d, "mode")).read().strip()

def helper_wait():
    # Na falha no início, a espera real de 8 s do helper com o X 240
    # presente: é ela que deixaria o limite padrão de inícios sem efeito.
    # Com o End() travado, o orçamento inteiro de 18 s (flock e os 8 s).
    time.sleep({"startfail": 8, "hangend": 18}.get(mode, 1))

if "--waterforce-cpu-safe" in sys.argv:
    log(f"helper {os.environ.get('SERVICE_RESULT', '?')} {os.environ.get('EXIT_CODE', '?')}")
    helper_wait()
    sys.exit(0)

import gi
gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib

IFACE = "io.github.leoodaviid.Halo1.Session"
PATH = "/io/github/leoodaviid/Halo1"

if "--session" in sys.argv:
    # Como o halod --session: End() sem ativação, com prazo de 10 s; sem
    # resposta, o helper e a falha. Begin() que falha não falha a guarda.
    step = sys.argv[sys.argv.index("--session") + 1]
    method = {"begin": "Begin", "end": "End"}[step]
    flags = Gio.DBusCallFlags.NO_AUTO_START if step == "end" else Gio.DBusCallFlags.NONE
    bus = Gio.bus_get_sync(Gio.BusType.SESSION)
    log(f"session {step} call")
    try:
        bus.call_sync(name, PATH, IFACE, method, None, None, flags, 10000, None)
    except GLib.Error as e:
        log(f"session {step} failed {e.message}")
        if step == "end":
            log("guard-helper start")
            helper_wait()
            log("guard-helper done")
            sys.exit(1)
        sys.exit(0)
    log(f"session {step} ok")
    sys.exit(0)

if mode == "startfail":
    log("main startfail")
    sys.exit(1)

NODE = Gio.DBusNodeInfo.new_for_xml(
    f'<node><interface name="{IFACE}"><method name="Begin"/><method name="End"/></interface></node>'
)

def on_call(conn, sender, path, iface, method, params, invocation):
    log(f"main {method.lower()} received")
    if method == "End" and mode == "slowend":
        def reply():
            log("main end reply")
            invocation.return_value(None)
            return False
        GLib.timeout_add(3000, reply)
        return
    if method == "End" and mode == "hangend":
        return
    log(f"main {method.lower()} reply")
    invocation.return_value(None)

bus = Gio.bus_get_sync(Gio.BusType.SESSION)
bus.register_object(PATH, NODE.interfaces[0], on_call, None, None)
bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
              "RequestName", GLib.Variant("(su)", (name, 4)),
              None, Gio.DBusCallFlags.NONE, -1, None)
log(f"main {os.getpid()} {mode}")

def notify(msg):
    addr = os.environ.get("NOTIFY_SOCKET", "")
    if addr.startswith("@"):
        addr = "\0" + addr[1:]
    s = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
    s.sendto(msg.encode(), addr)

if mode != "noready":
    notify("READY=1")

def name_call(method, args):
    bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
                  method, GLib.Variant(*args), None, Gio.DBusCallFlags.NONE, -1, None)

def drop_name():
    name_call("ReleaseName", ("(s)", (name,)))
    log("main name released")
    GLib.timeout_add_seconds(3, take_name)
    return False

def take_name():
    name_call("RequestName", ("(su)", (name, 4)))
    log("main name back")
    return False

if mode == "dropname":
    GLib.timeout_add_seconds(2, drop_name)

loop = GLib.MainLoop()

def term():
    log("main sigterm")
    loop.quit()
    return False

started = time.monotonic()

def tick():
    if mode == "panic" and time.monotonic() - started > 3:
        log("main panic")
        os._exit(101)
    if mode != "hang":
        notify("WATCHDOG=1")
    return True

GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, signal.SIGTERM, term)
tick()
GLib.timeout_add_seconds(2, tick)
loop.run()
EOF
chmod 755 "$T/bin/halod"

BUS=io.github.leoodaviid.HaloHarness.p$$
sed -e "s|@BINDIR@|$T/bin|g" \
    -e "s|^BusName=.*|BusName=$BUS|" \
    -e "s|graphical-session.target|$TARGET|g" \
    -e "s|^\[Service\]|[Service]\nEnvironment=HARNESS_DIR=$T HARNESS_NAME=$BUS|" \
    "$ROOT/packaging/systemd/halod.service" > "$UNIT_DIR/$UNIT"
sed -e "s|@BINDIR@|$T/bin|g" \
    -e "s|graphical-session.target|$TARGET|g" \
    -e "s|^\[Service\]|[Service]\nEnvironment=HARNESS_DIR=$T HARNESS_NAME=$BUS|" \
    "$ROOT/packaging/systemd/halod-session.service" > "$UNIT_DIR/$GUARD"
[ "${HARNESS_DROP_AFTER:-}" != 1 ] || sed -i '/^After=/d' "$UNIT_DIR/$GUARD"
[ "${HARNESS_TYPE_DBUS:-}" != 1 ] || sed -i 's/^Type=notify$/Type=dbus/' "$UNIT_DIR/$UNIT"
printf '[Unit]\nDescription=alvo do harness do halo\n' > "$UNIT_DIR/$TARGET"
systemctl --user daemon-reload

prop() { systemctl --user show -p "$1" --value "$UNIT"; }
gprop() { systemctl --user show -p "$1" --value "$GUARD"; }
# Instante (CLOCK_MONOTONIC, em s) do último "Stopped" da unidade no journal,
# depois de <desde>. A unidade inativa é descarregada, e as propriedades dela
# deixam de valer; o journal fica. Espera até 5 s pela escrita.
stopped_at() {
    i=0
    while [ "$i" -le 25 ]; do
        at=$(journalctl --user -o json --no-pager -b USER_UNIT="$1" JOB_TYPE=stop JOB_RESULT=done |
            python3 -I -c 'import json, sys
last = [int(json.loads(l)["__MONOTONIC_TIMESTAMP"]) for l in sys.stdin]
print(f"{last[-1] / 1e6:.3f}" if last else "")')
        if [ -n "$at" ] && awk -v a="$at" -v b="$2" 'BEGIN { exit !(a + 0 > b + 0) }'; then
            echo "$at"
            return
        fi
        i=$((i + 1))
        sleep 0.2
    done
}
# Instante do último registro com o texto dado.
logged_at() { grep "$1" "$LOG" | tail -n 1 | cut -d' ' -f1; }
before() { awk -v a="$1" -v b="$2" 'BEGIN { exit !(a != "" && b != "" && a + 0 <= b + 0) }'; }
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
    systemctl --user stop "$GUARD" "$UNIT" "$TARGET" > /dev/null 2>&1 || true
    systemctl --user reset-failed "$UNIT" "$GUARD" > /dev/null 2>&1 || true
    : > "$LOG"
    printf '%s\n' "$1" > "$MODE"
}

check "unidade gerada passa no systemd-analyze verify" \
    systemd-analyze --user verify "$UNIT_DIR/$UNIT"
check "guarda gerada passa no systemd-analyze verify" \
    systemd-analyze --user verify "$UNIT_DIR/$GUARD"

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

# Type=notify: sem READY=1, a unidade não fica ativa, mesmo com o nome.
fresh noready
systemctl --user start --no-block "$UNIT"
wait_for 10 mains_at_least 1
sleep 2
check "sem READY=1: unidade continua em activating" [ "$(prop ActiveState)" = activating ]
systemctl --user stop "$UNIT"

# Nome perdido com a unidade rodando (como o barramento que volta antes de o
# halod reconectar): com Type=notify, nada muda; com Type=dbus, o systemd
# pararia a unidade como sucesso, sem Restart.
fresh dropname
systemctl --user start "$UNIT"
wait_for 10 mains_at_least 1
check "nome perdido: unidade ativa logo depois do início" active
pid=$(prop MainPID)
wait_for 15 sh -c "grep -q 'main name back' '$LOG'" || true
sleep 1
check "nome perdido: nome solto e de volta" sh -c "grep -q 'main name released' '$LOG' && grep -q 'main name back' '$LOG'"
check "nome perdido: unidade continua ativa" active
check "nome perdido: mesmo processo" [ "$(prop MainPID)" = "$pid" ]
check "nome perdido: sem SIGTERM nem helper" sh -c "! grep -qE 'main sigterm| helper ' '$LOG'"
check "nome perdido: NRestarts = 0" [ "$(prop NRestarts)" = 0 ]
systemctl --user stop "$UNIT"

# Fim do alvo (o logout): o daemon continua; a guarda chama o End() e o
# alvo só para depois da resposta, que o daemon falso atrasa 3 s.
fresh slowend
systemctl --user start "$UNIT"
wait_for 10 mains_at_least 1
systemctl --user start "$TARGET" "$GUARD"
check "login: guarda ativa" [ "$(gprop ActiveState)" = active ]
check "login: Begin() respondido" sh -c "grep -q 'main begin reply' '$LOG' && grep -q 'session begin ok' '$LOG'"
systemctl --user stop "$TARGET"
reply=$(logged_at 'main end reply')
check "logout: End() respondido em 3 s" [ -n "$reply" ]
called=$(logged_at 'session end call')
guard_stop=$(stopped_at "$GUARD" "$called")
target_stop=$(stopped_at "$TARGET" "$called")
check "logout: alvo parado só depois da resposta" before "$reply" "$target_stop"
check "logout: guarda parada antes do alvo" before "$guard_stop" "$target_stop"
check "logout: guarda terminou com sucesso" [ "$(gprop Result)" = success ]
check "logout: daemon continua ativo" active
check "logout: nenhum helper" [ "$(count 'helper ')" = 0 ]
systemctl --user stop "$UNIT"
check "logout: stop do daemon ainda passa pelo helper" helpers_at_least 1

# Logout com o daemon fora do barramento: a guarda roda o helper e falha.
fresh normal
systemctl --user start "$TARGET" "$GUARD"
check "sem daemon: Begin() que falha não falha a guarda" [ "$(gprop ActiveState)" = active ]
systemctl --user stop "$TARGET" || true
check "sem daemon: helper da guarda" sh -c "grep -q 'guard-helper done' '$LOG'"
check "sem daemon: helper depois da falha do End()" before "$(logged_at 'session end failed')" "$(logged_at 'guard-helper start')"
check "sem daemon: guarda em failed" [ "$(gprop Result)" = exit-code ]

# End() sem resposta e o helper com o orçamento inteiro (18 s): a chamada
# desiste em 10 s e o helper termina antes de o systemd matar a guarda.
fresh hangend
systemctl --user start "$UNIT"
wait_for 10 mains_at_least 1
systemctl --user start "$TARGET" "$GUARD"
systemctl --user stop "$TARGET" || true
called=$(logged_at 'session end call')
failed=$(logged_at 'session end failed')
check "End() travado: chamada desiste em ~10 s" awk -v a="$called" -v b="$failed" 'BEGIN { d = b - a; exit !(d >= 9.5 && d < 12) }'
check "End() travado: helper registra o fim" sh -c "grep -q 'guard-helper done' '$LOG'"
check "End() travado: guarda não morta pelo prazo" [ "$(gprop Result)" = exit-code ]
check "End() travado: alvo parado depois do helper" before "$(logged_at 'guard-helper done')" "$(stopped_at "$TARGET" "$called")"
systemctl --user stop "$UNIT"

if [ "$failures" -ne 0 ]; then
    printf '%d verificação(ões) falharam\n' "$failures"
    exit 1
fi
echo "todas as verificações passaram"
