#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Testa o packaging/install.sh num diretório temporário, com cargo e
# systemctl falsos no PATH. Nada é compilado, nenhuma unidade de verdade é
# carregada e nada é escrito fora do temporário.
#
# Uso: sh packaging/tests/install-test.sh

set -eu

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
SCRIPT=$ROOT/packaging/install.sh

T=$(mktemp -d "${TMPDIR:-/tmp}/halo-install-test.XXXXXX")
trap 'rm -rf "$T"' EXIT INT TERM

failures=0
ok() { printf 'ok    %s\n' "$1"; }
fail() { printf 'FALHA %s\n' "$1"; failures=$((failures + 1)); }
# check <nome> <comando...>: uma condição só; para juntar várias, use uma
# função.
check() {
    name=$1
    shift
    if "$@"; then ok "$name"; else fail "$name"; fi
}

# cargo falso: cria halo e halod com o conteúdo de FAKE_VERSION, ou falha
# com FAKE_CARGO_FAIL=1. systemctl falso: só registra os argumentos.
mkdir -p "$T/stubs"
cat > "$T/stubs/cargo" <<'EOF'
#!/bin/sh
echo "cargo $*" >> "$FAKE_LOG"
[ "${FAKE_CARGO_FAIL:-0}" = 1 ] && { echo "error: falha de mentira" >&2; exit 101; }
mkdir -p "$CARGO_TARGET_DIR/release"
for b in halo halod; do
    printf '#!/bin/sh\n# %s %s\n' "$b" "$FAKE_VERSION" > "$CARGO_TARGET_DIR/release/$b"
    chmod 755 "$CARGO_TARGET_DIR/release/$b"
done
EOF
cat > "$T/stubs/systemctl" <<'EOF'
#!/bin/sh
echo "systemctl $*" >> "$FAKE_LOG"
EOF
chmod 755 "$T/stubs/cargo" "$T/stubs/systemctl"

export HOME="$T/home"
export XDG_CONFIG_HOME="$T/home/.config"
export XDG_DATA_HOME="$T/home/.local/share"
export XDG_STATE_HOME="$T/home/.local/state"
export BINDIR="$T/home/.local/bin"
export CARGO_TARGET_DIR="$T/target"
export HALO_UDEV_RULES_DIR="$T/udev"
export FAKE_LOG="$T/calls.log"
export FAKE_VERSION=1
export PATH="$T/stubs:/usr/bin:/bin"
mkdir -p "$HOME" "$HALO_UDEV_RULES_DIR"

UNIT_FILE=$XDG_CONFIG_HOME/systemd/user/halod.service
DBUS_FILE=$XDG_DATA_HOME/dbus-1/services/io.github.leoodaviid.Halo1.service
PROFILE=$XDG_CONFIG_HOME/halo/profiles/noite.toml
ORIGINAL=$XDG_STATE_HOME/halo/waterforce-speed.original

# Perfis e snapshots que o script nunca pode mexer.
mkdir -p "$(dirname "$PROFILE")" "$(dirname "$ORIGINAL")"
echo '[waterforce.led]' > "$PROFILE"
echo 'model=0 fan=5 pump=4' > "$ORIGINAL"
echo 'noite' > "$XDG_STATE_HOME/halo/active-profile"
user_data_sum() {
    cat "$PROFILE" "$ORIGINAL" "$XDG_STATE_HOME/halo/active-profile" | cksum
}
DATA_BEFORE=$(user_data_sum)

run() {
    : > "$FAKE_LOG"
    set +e
    sh "$SCRIPT" "$@" > "$T/out" 2>&1
    status=$?
    set -e
}
out_has() { grep -qF -- "$1" "$T/out"; }
out_has_all() {
    for text in "$@"; do out_has "$text" || return 1; done
}
exist_all() {
    for f in "$@"; do [ -e "$f" ] || return 1; done
}
missing_all() {
    for f in "$@"; do [ ! -e "$f" ] || return 1; done
}
calls() { grep '^systemctl' "$FAKE_LOG" || true; }
calls_are() { [ "$(calls)" = "$1" ]; }

touch "$T/marker"
sleep 1

# Primeira instalação, sem a regra udev.
run
check "primeira instalação termina com sucesso" [ "$status" = 0 ]
check "halo instalado e executável" [ -x "$BINDIR/halo" ]
check "halod instalado e executável" [ -x "$BINDIR/halod" ]
check "unidade e ativação instaladas" exist_all "$UNIT_FILE" "$DBUS_FILE"
check "@BINDIR@ trocado na unidade" grep -qxF "ExecStart=$BINDIR/halod" "$UNIT_FILE"
check "@BINDIR@ trocado na ativação" grep -qxF "Exec=$BINDIR/halod" "$DBUS_FILE"
check "nenhum @BINDIR@ sobrando" sh -c "! grep -q @BINDIR@ '$UNIT_FILE' '$DBUS_FILE'"
check "compila com --release --locked" grep -q -- '--release --locked' "$FAKE_LOG"
check "daemon-reload, enable e restart, nessa ordem" calls_are "systemctl --user daemon-reload
systemctl --user enable halod.service
systemctl --user restart halod.service"
check "regra ausente: avisa e mostra os comandos" \
    out_has_all "a regra udev não está instalada" "sudo udevadm control --reload"
check "avisa que o BINDIR não está no PATH" out_has "não está no PATH"
check "nenhum .new sobrando" \
    sh -c "! find '$HOME' -name '*.new' | grep -q ."

# Reinstalação com uma versão nova, com a regra em dia e o BINDIR no PATH.
cp "$ROOT/packaging/udev/70-halo.rules" "$HALO_UDEV_RULES_DIR/"
export FAKE_VERSION=2
PATH="$PATH:$BINDIR" run
check "reinstalação termina com sucesso" [ "$status" = 0 ]
check "reinstalação troca os binários" grep -q 'halod 2' "$BINDIR/halod"
check "reinstalação reinicia o daemon" grep -qxF 'systemctl --user restart halod.service' "$FAKE_LOG"
check "regra em dia: nenhum comando com sudo" sh -c "! grep -q sudo '$T/out'"
check "BINDIR no PATH: sem aviso" sh -c "! grep -q 'não está no PATH' '$T/out'"

# Regra desatualizada.
echo '# mudada' >> "$HALO_UDEV_RULES_DIR/70-halo.rules"
run
check "regra diferente: avisa que está desatualizada" \
    out_has_all "está desatualizada" "sudo install -m 644"

# Compilação falha: nada instalado muda e o systemd não é chamado.
FAKE_VERSION=3 FAKE_CARGO_FAIL=1 run
check "compilação falha: código diferente de zero" [ "$status" != 0 ]
check "compilação falha: binário anterior mantido" grep -q 'halod 2' "$BINDIR/halod"
check "compilação falha: systemctl não chamado" calls_are ""

check "perfis e snapshots iguais depois das instalações" \
    [ "$(user_data_sum)" = "$DATA_BEFORE" ]

# Desinstalação.
run --uninstall
check "desinstalação termina com sucesso" [ "$status" = 0 ]
check "desinstalação para, desabilita e recarrega" calls_are "systemctl --user disable --now halod.service
systemctl --user daemon-reload"
check "binários, unidade e ativação removidos" \
    missing_all "$BINDIR/halo" "$BINDIR/halod" "$UNIT_FILE" "$DBUS_FILE"
check "perfis e snapshots mantidos, inclusive o original" \
    [ "$(user_data_sum)" = "$DATA_BEFORE" ]
check "desinstalação informa onde ficaram os dados" \
    out_has_all "perfis mantidos em $XDG_CONFIG_HOME/halo" "snapshots em $XDG_STATE_HOME/halo"

# Desinstalar de novo, sem nada instalado.
run --uninstall
check "desinstalar sem nada instalado: sucesso" [ "$status" = 0 ]
check "desinstalar sem nada instalado: avisa" out_has "nada a remover"
check "desinstalar sem nada instalado: systemctl não chamado" calls_are ""

# Ajuda e argumento inválido.
run --help
check "--help termina com sucesso" [ "$status" = 0 ]
check "--help cita o --uninstall" out_has "--uninstall"
run --foo
check "argumento inválido: código 2" [ "$status" = 2 ]

# Nada escrito no repositório (fora do temporário só o repositório é
# acessível; o HOME e todos os XDG apontam para o temporário).
check "nada escrito no repositório" sh -c "[ -z \"\$(find '$ROOT' -path '$ROOT/.git' -prune -o -path '$ROOT/target' -prune -o -newer '$T/marker' -print)\" ]"
check "nenhuma chamada a sudo ou udevadm" sh -c "! grep -qE '^(sudo|udevadm)' '$FAKE_LOG'"

if [ "$failures" -ne 0 ]; then
    printf '%d verificação(ões) falharam\n' "$failures"
    exit 1
fi
echo "todas as verificações passaram"
