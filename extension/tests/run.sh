#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Roda os testes da extensão com o gjs, um arquivo por vez.
#
# Uso: sh extension/tests/run.sh

set -eu

DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)

failures=0
for t in "$DIR"/*.test.js; do
    [ -e "$t" ] || continue
    if ! gjs -m "$t"; then
        failures=$((failures + 1))
    fi
done

if [ "$failures" -ne 0 ]; then
    printf '%s arquivo(s) de teste falharam\n' "$failures" >&2
    exit 1
fi
echo 'todas as verificações passaram'
