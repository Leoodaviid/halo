#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Testa o packaging/install.sh num diretório temporário, com cargo,
# systemctl e gsettings falsos no PATH e uma extensão falsa. Nada é
# compilado, nenhuma unidade de verdade é carregada, as configurações do
# GNOME não são tocadas e nada é escrito fora do temporário.
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
# com FAKE_CARGO_FAIL=1. O halod falso responde ao --help com o
# --waterforce-cpu-safe, salvo com FAKE_NO_SAFE=1. systemctl falso: registra
# os argumentos e, num stop, qual halod estava instalado (o ExecStopPost
# precisa dele).
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
if [ "${FAKE_NO_SAFE:-0}" != 1 ]; then
    echo 'echo "      --waterforce-cpu-safe"' >> "$CARGO_TARGET_DIR/release/halod"
fi
EOF
cat > "$T/stubs/systemctl" <<'EOF'
#!/bin/sh
echo "systemctl $*" >> "$FAKE_LOG"
case "$*" in
    *stop* | *--now*)
        if [ -x "$BINDIR/halod" ]; then
            echo "helper: $(sed -n 2p "$BINDIR/halod")" >> "$FAKE_LOG"
        else
            echo "helper: ausente" >> "$FAKE_LOG"
        fi
        ;;
esac
EOF
# gsettings falso: guarda enabled-extensions e disable-user-extensions em
# arquivos de $FAKE_GSETTINGS_DIR (padrões '@as []' e false) e falha com
# FAKE_GSETTINGS_FAIL=1.
cat > "$T/stubs/gsettings" <<'EOF'
#!/bin/sh
echo "gsettings $*" >> "$FAKE_LOG"
[ "${FAKE_GSETTINGS_FAIL:-0}" = 1 ] && { echo "gsettings: falha de mentira" >&2; exit 1; }
[ "${2:-}" = org.gnome.shell ] || { echo "gsettings falso: $*" >&2; exit 1; }
file=$FAKE_GSETTINGS_DIR/${3:-}
case "${1:-} ${3:-}" in
    'get enabled-extensions') cat "$file" 2>/dev/null || echo '@as []' ;;
    'get disable-user-extensions') cat "$file" 2>/dev/null || echo false ;;
    'set enabled-extensions') printf '%s\n' "$4" > "$file" ;;
    *) echo "gsettings falso: $*" >&2; exit 1 ;;
esac
EOF
chmod 755 "$T/stubs/cargo" "$T/stubs/systemctl" "$T/stubs/gsettings"

# Extensão falsa, com o que o script não pode instalar: tests/ e arquivos de
# editor.
EXT_SRC=$T/extsrc
mkdir -p "$EXT_SRC/tests" "$EXT_SRC/lib"
echo '{"uuid": "halo@leoodaviid.github.io"}' > "$EXT_SRC/metadata.json"
echo '// versão 1' > "$EXT_SRC/extension.js"
echo '// lib' > "$EXT_SRC/lib/view.js"
echo '// teste' > "$EXT_SRC/tests/view.test.js"
echo 'swap' > "$EXT_SRC/.extension.js.swp"
echo 'backup' > "$EXT_SRC/extension.js~"

export HOME="$T/home"
export XDG_CONFIG_HOME="$T/home/.config"
export XDG_DATA_HOME="$T/home/.local/share"
export XDG_STATE_HOME="$T/home/.local/state"
export BINDIR="$T/home/.local/bin"
export CARGO_TARGET_DIR="$T/target"
export HALO_UDEV_RULES_DIR="$T/udev"
export HALO_EXTENSION_SRC="$EXT_SRC"
export FAKE_GSETTINGS_DIR="$T/gsettings"
export FAKE_LOG="$T/calls.log"
export FAKE_VERSION=1
export PATH="$T/stubs:/usr/bin:/bin"
mkdir -p "$HOME" "$HALO_UDEV_RULES_DIR" "$FAKE_GSETTINGS_DIR"

UNIT_FILE=$XDG_CONFIG_HOME/systemd/user/halod.service
DBUS_FILE=$XDG_DATA_HOME/dbus-1/services/io.github.leoodaviid.Halo1.service
PROFILE=$XDG_CONFIG_HOME/halo/profiles/noite.toml
ORIGINAL=$XDG_STATE_HOME/halo/waterforce-speed.original
UUID=halo@leoodaviid.github.io
EXT_DIR=$XDG_DATA_HOME/gnome-shell/extensions/$UUID

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

# Lista de extensões habilitadas guardada pelo gsettings falso.
set_enabled() { printf '%s\n' "$1" > "$FAKE_GSETTINGS_DIR/enabled-extensions"; }
enabled_is() { [ "$(cat "$FAKE_GSETTINGS_DIR/enabled-extensions")" = "$1" ]; }
# A extensão instalada tem os arquivos dela e nenhum dos que não vão.
ext_installed() {
    exist_all "$EXT_DIR/metadata.json" "$EXT_DIR/extension.js" "$EXT_DIR/lib/view.js" &&
        missing_all "$EXT_DIR/tests" "$EXT_DIR/.extension.js.swp" "$EXT_DIR/extension.js~"
}
RELOGIN="saia e entre na sessão"

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
check "primeira instalação: daemon-reload, enable e start, sem stop" calls_are "systemctl --user daemon-reload
systemctl --user enable halod.service
systemctl --user start halod.service"
check "unidade com o helper no ExecStopPost" \
    grep -qxF "ExecStopPost=$BINDIR/halod --waterforce-cpu-safe" "$UNIT_FILE"
check "unidade com o watchdog e o limite de inícios" \
    sh -c "grep -qxF 'WatchdogSec=20s' '$UNIT_FILE' && grep -qxF 'WatchdogSignal=SIGKILL' '$UNIT_FILE' && grep -qxF 'StartLimitIntervalSec=180s' '$UNIT_FILE'"
check "regra ausente: avisa e mostra os comandos" \
    out_has_all "a regra udev não está instalada" "sudo udevadm control --reload"
check "avisa que o BINDIR não está no PATH" out_has "não está no PATH"
check "nenhum .new sobrando" \
    sh -c "! find '$HOME' -name '*.new' | grep -q ."
check "extensão instalada sem tests/ nem arquivos de editor" ext_installed
check "extensão habilitada pelo gsettings" enabled_is "['$UUID']"
check "extensão nova: pede para sair e entrar na sessão" out_has "$RELOGIN"
check "extensões do usuário ligadas: sem aviso" sh -c "! grep -q 'extensões do usuário estão desligadas' '$T/out'"

# Reinstalação com uma versão nova, com a regra em dia e o BINDIR no PATH.
cp "$ROOT/packaging/udev/70-halo.rules" "$HALO_UDEV_RULES_DIR/"
export FAKE_VERSION=2
PATH="$PATH:$BINDIR" run
check "reinstalação termina com sucesso" [ "$status" = 0 ]
check "reinstalação troca os binários" grep -q 'halod 2' "$BINDIR/halod"
check "reinstalação para antes de trocar e inicia depois" calls_are "systemctl --user stop halod.service
systemctl --user daemon-reload
systemctl --user enable halod.service
systemctl --user start halod.service"
check "o stop roda com o halod antigo ainda instalado" grep -qxF 'helper: # halod 1' "$FAKE_LOG"
check "o halod instalado tem o modo de proteção" sh -c "'$BINDIR/halod' --help | grep -q -- --waterforce-cpu-safe"
check "regra em dia: nenhum comando com sudo" sh -c "! grep -q sudo '$T/out'"
check "BINDIR no PATH: sem aviso" sh -c "! grep -q 'não está no PATH' '$T/out'"
check "extensão igual: não pede para sair e entrar na sessão" sh -c "! grep -qF '$RELOGIN' '$T/out'"
check "extensão igual: nenhum .new sobrando" missing_all "$EXT_DIR.new"
check "extensão já na lista: o UUID aparece uma vez só" enabled_is "['$UUID']"

# Extensão já habilitada entre outras: a lista não muda.
set_enabled "['user-theme@x', '$UUID', 'blur@y']"
run
check "extensão já na lista entre outras: lista igual" enabled_is "['user-theme@x', '$UUID', 'blur@y']"

# Extensão mudada no repositório.
echo '// versão 2' > "$EXT_SRC/extension.js"
run
check "extensão mudada: pede para sair e entrar na sessão" out_has "$RELOGIN"
check "extensão mudada: arquivo novo instalado" grep -qxF '// versão 2' "$EXT_DIR/extension.js"
check "extensão mudada: continua sem tests/" ext_installed

# Sem sessão gráfica: o gsettings falha, a instalação termina.
set_enabled '@as []'
FAKE_GSETTINGS_FAIL=1 run
check "gsettings falha: instalação termina com sucesso" [ "$status" = 0 ]
check "gsettings falha: mostra como habilitar depois" out_has "gnome-extensions enable $UUID"
check "gsettings falha: lista não mudou" enabled_is '@as []'

# Lista com outras extensões: ficam, e o UUID entra no fim.
set_enabled "['user-theme@x', 'blur@y']"
run
check "lista com outras: outras mantidas e UUID no fim" enabled_is "['user-theme@x', 'blur@y', '$UUID']"

# Formato inesperado: avisa e não grava.
set_enabled "['user-theme@x'"
run
check "formato inesperado: instalação termina com sucesso" [ "$status" = 0 ]
check "formato inesperado: avisa" out_has "formato inesperado"
check "formato inesperado: lista não mudou" enabled_is "['user-theme@x'"
check "formato inesperado: nenhum gsettings set" sh -c "! grep -q '^gsettings set' '$FAKE_LOG'"

# Extensões do usuário desligadas no GNOME.
echo true > "$FAKE_GSETTINGS_DIR/disable-user-extensions"
set_enabled "['$UUID']"
run
check "extensões do usuário desligadas: avisa" out_has "extensões do usuário estão desligadas"
echo false > "$FAKE_GSETTINGS_DIR/disable-user-extensions"

# Regra desatualizada.
echo '# mudada' >> "$HALO_UDEV_RULES_DIR/70-halo.rules"
run
check "regra diferente: avisa que está desatualizada" \
    out_has_all "está desatualizada" "sudo install -m 644"

# halod sem o modo de proteção: nada instalado muda, nem o daemon para.
FAKE_VERSION=4 FAKE_NO_SAFE=1 run
check "halod sem o helper: código diferente de zero" [ "$status" != 0 ]
check "halod sem o helper: avisa" out_has "não tem --waterforce-cpu-safe"
check "halod sem o helper: binário anterior mantido" grep -q 'halod 2' "$BINDIR/halod"
check "halod sem o helper: systemctl não chamado" calls_are ""

# Compilação falha: nada instalado muda e o systemd não é chamado.
FAKE_VERSION=3 FAKE_CARGO_FAIL=1 run
check "compilação falha: código diferente de zero" [ "$status" != 0 ]
check "compilação falha: binário anterior mantido" grep -q 'halod 2' "$BINDIR/halod"
check "compilação falha: systemctl não chamado" calls_are ""
check "compilação falha: extensão não trocada" grep -qxF '// versão 2' "$EXT_DIR/extension.js"

# Extensão sem metadata.json: para antes de compilar.
mv "$EXT_SRC/metadata.json" "$T/metadata.json"
run
check "extensão sem metadata.json: código diferente de zero" [ "$status" != 0 ]
check "extensão sem metadata.json: avisa" out_has "sem metadata.json"
check "extensão sem metadata.json: nada compilado" sh -c "! grep -q '^cargo' '$FAKE_LOG'"
mv "$T/metadata.json" "$EXT_SRC/metadata.json"

check "perfis e snapshots iguais depois das instalações" \
    [ "$(user_data_sum)" = "$DATA_BEFORE" ]

# Desinstalação, com outras extensões na lista.
set_enabled "['user-theme@x', '$UUID', 'blur@y']"
run --uninstall
check "desinstalação termina com sucesso" [ "$status" = 0 ]
check "desinstalação para, desabilita e recarrega" calls_are "systemctl --user disable --now halod.service
systemctl --user daemon-reload"
check "desinstalação para com o halod ainda instalado" grep -qxF 'helper: # halod 2' "$FAKE_LOG"
check "binários, unidade e ativação removidos" \
    missing_all "$BINDIR/halo" "$BINDIR/halod" "$UNIT_FILE" "$DBUS_FILE"
check "perfis e snapshots mantidos, inclusive o original" \
    [ "$(user_data_sum)" = "$DATA_BEFORE" ]
check "desinstalação informa onde ficaram os dados" \
    out_has_all "perfis mantidos em $XDG_CONFIG_HOME/halo" "snapshots em $XDG_STATE_HOME/halo"
check "desinstalação remove a pasta da extensão" missing_all "$EXT_DIR"
check "desinstalação tira o UUID e mantém as outras" enabled_is "['user-theme@x', 'blur@y']"

# Desinstalar de novo, sem nada instalado.
run --uninstall
check "desinstalar sem nada instalado: sucesso" [ "$status" = 0 ]
check "desinstalar sem nada instalado: avisa" out_has "nada a remover"
check "desinstalar sem nada instalado: systemctl não chamado" calls_are ""
check "desinstalar sem nada instalado: lista não mudou" enabled_is "['user-theme@x', 'blur@y']"

# Só o UUID na lista: a lista fica vazia.
set_enabled "['$UUID']"
run --uninstall
check "lista só com o halo: vira @as []" enabled_is '@as []'
check "lista só com o halo: não diz que não havia nada" sh -c "! grep -q 'nada a remover' '$T/out'"
check "só a extensão: systemctl não chamado" calls_are ""

# Desinstalar com a extensão instalada e o gsettings falhando.
mkdir -p "$EXT_DIR"
cp "$EXT_SRC/metadata.json" "$EXT_DIR/"
FAKE_GSETTINGS_FAIL=1 run --uninstall
check "desinstalar com gsettings falhando: sucesso" [ "$status" = 0 ]
check "desinstalar com gsettings falhando: avisa" out_has "gnome-extensions disable $UUID"
check "desinstalar com gsettings falhando: pasta removida" missing_all "$EXT_DIR"

# Ajuda e argumento inválido.
run --help
check "--help termina com sucesso" [ "$status" = 0 ]
check "--help cita o --uninstall" out_has "--uninstall"
check "--help cita a extensão" out_has "gnome-shell/extensions/$UUID"
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
