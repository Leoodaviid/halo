#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Monitor só de leitura dos Led.Apply do halod, para contar as gravações no
# Waterforce. Cada Apply que dá certo termina com um único 99 B6
# (LedSetting::packets() no halo-core) e muda o LastApplied, que sai num
# PropertiesChanged; um Apply com erro não mandou nenhum B6, salvo um erro
# Device no meio da sequência, que fica marcado como dúvida.
#
# Usa o dbus-monitor (BecomeMonitor) e não chama nenhum método do halod, nem
# exige root. Ctrl+C mostra os totais.
#
# Uso: sh scripts/dev/watch-led-applies.sh [--address <endereço>]
#   --address  um barramento de teste (o DBUS_SESSION_BUS_ADDRESS do
#              dbus-run-session); sem ele, o D-Bus da sessão.

set -eu

bus=--session
case "${1:-}" in
    '') ;;
    --address)
        [ $# -eq 2 ] || { echo "uso: sh scripts/dev/watch-led-applies.sh [--address <endereço>]" >&2; exit 2; }
        bus="--address $2" ;;
    *) echo "uso: sh scripts/dev/watch-led-applies.sh [--address <endereço>]" >&2; exit 2 ;;
esac

command -v dbus-monitor > /dev/null || { echo "halo: falta o dbus-monitor" >&2; exit 1; }

LED=io.github.leoodaviid.Halo1.Waterforce.Led
echo "halo: observando os Led.Apply (Ctrl+C mostra os totais)"

# O dbus-monitor escreve num FIFO lido pelo awk: o Ctrl+C encerra só o
# monitor, e o awk termina a entrada e mostra os totais.
DIR=$(mktemp -d "${TMPDIR:-/tmp}/halo-watch.XXXXXX")
trap 'rm -rf "$DIR"' EXIT
mkfifo "$DIR/bus"

# shellcheck disable=SC2086 # $bus são duas palavras de propósito
dbus-monitor $bus \
    "type='method_call',interface='$LED',member='Apply'" \
    "type='signal',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged',path='/io/github/leoodaviid/Halo1/waterforce',arg0='$LED'" \
    "type='error'" > "$DIR/bus" 2>/dev/null &
MONITOR_PID=$!

awk '
    function flush() {
        if (kind == "apply") {
            applies++
            printf "%s pedido: %s\n", stamp(), args; fflush()
        } else if (kind == "error") {
            errors++
            if (name ~ /\.Device$/) doubt++
            printf "%s erro:   %s %s%s\n", stamp(), name, msg, (name ~ /\.Device$/ ? " (B6 incerto)" : ""); fflush()
        }
        kind = ""; args = ""; msg = ""
    }
    function stamp(  cmd, t) {
        cmd = "date +%T"; cmd | getline t; close(cmd); return t
    }
    BEGIN { applies = 0; b6 = 0; errors = 0; doubt = 0 }
    /^method call / { flush(); kind = "apply"; next }
    /^signal / && /member=PropertiesChanged/ {
        flush(); b6++
        printf "%s aplicado: um 99 B6\n", stamp(); fflush(); next
    }
    /^error / {
        flush()
        if (match($0, /error_name=[^ ]+/) && substr($0, RSTART + 11, RLENGTH - 11) ~ /^io\.github\.leoodaviid\.Halo1\.Error\./) {
            kind = "error"; name = substr($0, RSTART + 11, RLENGTH - 11)
            sub(/^io\.github\.leoodaviid\.Halo1\.Error\./, "", name)
        }
        next
    }
    /^(method return|signal) / { flush(); next }
    kind == "apply" && /^ *(string|byte|array|\])/ { line = $0; gsub(/^ +/, "", line); args = args " " line; next }
    kind == "error" && /^ *string/ { line = $0; sub(/^ *string /, "", line); msg = msg line; next }
    { }
    END {
        flush()
        printf "halo: %d Apply, %d com sucesso (%d B6), %d erro(s)%s\n", applies, b6, b6, errors,
            (doubt ? sprintf(", %d com B6 incerto", doubt) : "")
    }
' < "$DIR/bus" &
AWK_PID=$!
# O wait é interrompido pelo sinal; o trap para o monitor, e o segundo wait
# espera o awk mostrar os totais.
trap 'kill "$MONITOR_PID" 2>/dev/null' INT TERM
wait "$AWK_PID" || wait "$AWK_PID"
