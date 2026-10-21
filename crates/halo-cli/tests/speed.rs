// SPDX-License-Identifier: GPL-2.0-or-later

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Output};

/// Roda o `halo` com `--sys-root` vazio e o estado em `state`.
fn run(args: &[&str], state: &Path) -> Output {
    let sys = tempfile::tempdir().unwrap();
    run_in(args, sys.path(), state)
}

fn run_in(args: &[&str], sys: &Path, state: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_halo"))
        .args(args)
        .arg("--sys-root")
        .arg(sys)
        .env("XDG_STATE_HOME", state)
        .output()
        .unwrap()
}

/// Árvore sysfs falsa com um hidraw `name` que se apresenta como o Castor3.
fn sys_with_castor3(name: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("class/hidraw").join(name).join("device");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("uevent"),
        "HID_ID=0003:00001044:00007A4D\nHID_NAME=ITE Tech. Castor3\n",
    )
    .unwrap();
    fs::write(
        dir.join("report_descriptor"),
        [0x05, 0x00, 0x09, 0x02, 0xA1, 0x01, 0xC0],
    )
    .unwrap();
    root
}

fn assert_empty(dir: &Path) {
    assert_eq!(fs::read_dir(dir).unwrap().count(), 0, "{}", dir.display());
}

fn curve_args<'a>(side: &'a str, rpm: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["speed", "waterforce", "curve", side, rpm];
    args.extend(extra);
    args
}

#[test]
fn simulacao() {
    let state = tempfile::tempdir().unwrap();
    let out = run(
        &[
            "speed",
            "waterforce",
            "set",
            "--fan",
            "quiet",
            "--pump",
            "turbo",
            "--dry-run",
        ],
        state.path(),
    );
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "99 E5 01 06 00\n99 E5 02 04 00\n99 B6\n"
    );
    assert_empty(state.path());

    let out = run(
        &[
            "speed",
            "waterforce",
            "set",
            "--fan",
            "quiet",
            "--pump",
            "turbo",
            "--dry-run",
            "--no-save",
        ],
        state.path(),
    );
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "99 E5 01 06 00\n99 E5 02 04 00\n"
    );
    assert_empty(state.path());
}

#[test]
fn simulacao_com_um_lado_so() {
    let state = tempfile::tempdir().unwrap();
    let out = run(
        &["speed", "waterforce", "set", "--fan", "quiet", "--dry-run"],
        state.path(),
    );
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("precisa de --fan e --pump"), "{err}");
    assert_empty(state.path());
}

#[test]
fn recusas_antes_de_abrir() {
    let state = tempfile::tempdir().unwrap();
    for (args, msg) in [
        (
            &["--fan", "zero-rpm"][..],
            "[possible values: default, quiet, balanced, performance, turbo]",
        ),
        (&["--fan", "customized"], "invalid value"),
        (&["--pump", "quiet"], "[possible values: balanced, turbo]"),
        (&[], "--fan <FAN>|--pump <PUMP>"),
        (&["--no-save"], "--fan <FAN>|--pump <PUMP>"),
    ] {
        let mut full = vec!["speed", "waterforce", "set"];
        full.extend(args);
        let out = run(&full, state.path());
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains(msg), "{args:?}: {err}");
        assert!(!err.contains("não encontrado"), "{args:?}: {err}");
    }
    assert_empty(state.path());
}

#[test]
fn restaurar_sem_snapshot() {
    let state = tempfile::tempdir().unwrap();
    for args in [&["restore"][..], &["restore", "--original"]] {
        let mut full = vec!["speed", "waterforce"];
        full.extend(args);
        let out = run(&full, state.path());
        assert_eq!(out.status.code(), Some(5), "{args:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("não há estado para restaurar"), "{err}");
        assert!(!err.contains("não encontrado"), "{err}");
    }
    assert_empty(state.path());
}

#[test]
fn snapshot_corrompido() {
    let state = tempfile::tempdir().unwrap();
    let dir = state.path().join("halo");
    fs::create_dir(&dir).unwrap();
    let file = dir.join("waterforce-speed.previous");
    fs::write(&file, "version=1\ntaken_unix=1\nmodel=0\nfan_mo").unwrap();
    let out = run(&["speed", "waterforce", "restore"], state.path());
    assert_eq!(out.status.code(), Some(5));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains(&file.display().to_string()), "{err}");
    assert!(!err.contains("não encontrado"), "{err}");
}

#[test]
fn aparelho_ausente() {
    let state = tempfile::tempdir().unwrap();
    let out = run(&["speed", "waterforce", "status"], state.path());
    assert_eq!(out.status.code(), Some(3));
    let out = run(
        &["speed", "waterforce", "set", "--fan", "quiet"],
        state.path(),
    );
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stderr).contains("Waterforce não encontrado"));
    // Sem aparelho, nada foi lido, então nada foi gravado.
    assert_empty(state.path());
}

#[test]
fn simulacao_da_curva_sem_sysfs() {
    let missing = Path::new("/caminho/que/nao/existe");
    let state = tempfile::tempdir().unwrap();
    let out = run_in(
        &curve_args("fan", "1500", &["--dry-run"]),
        missing,
        state.path(),
    );
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "99 E5 01 01 00\n\
         99 E5 02 ?? 00   (?? = modo atual da bomba, lido no DD na gravação)\n\
         99 E6 00 01 00 05 DC 1E 05 DC 32 05 DC 50 05 DC\n\
         99 B6\n"
    );
    assert_empty(state.path());

    let out = run_in(
        &curve_args("fan", "1500", &["--dry-run", "--no-save"]),
        missing,
        state.path(),
    );
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.ends_with("99 E6 00 01 00 05 DC 1E 05 DC 32 05 DC 50 05 DC\n"),
        "{stdout}"
    );
    assert!(!stdout.contains("B6"), "{stdout}");
    assert_empty(state.path());
}

#[test]
fn curva_invalida_sem_sysfs() {
    let missing = Path::new("/caminho/que/nao/existe");
    let state = tempfile::tempdir().unwrap();
    for (side, rpm, msg) in [
        ("fan", "500", "de 900 a 2500 RPM"),
        ("fan", "2600", "de 900 a 2500 RPM"),
        ("pump", "1500", "Balanced"),
        ("pump", "3150", "Turbo"),
        ("fan", "mil", "invalid value 'mil'"),
        ("fan", "70000", "invalid value '70000'"),
    ] {
        for extra in [&[][..], &["--dry-run"]] {
            let out = run_in(&curve_args(side, rpm, extra), missing, state.path());
            assert_eq!(out.status.code(), Some(2), "{side} {rpm} {extra:?}");
            let err = String::from_utf8_lossy(&out.stderr);
            assert!(err.contains(msg), "{side} {rpm}: {err}");
            assert!(!err.contains("não encontrado"), "{side} {rpm}: {err}");
            assert!(out.stdout.is_empty(), "{side} {rpm}");
        }
    }
    for args in [
        &["speed", "waterforce", "curve", "lcd", "1500"][..],
        &["speed", "waterforce", "curve", "fan", "0:1500", "30:1500"],
    ] {
        let out = run_in(args, missing, state.path());
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}");
    }
    assert_empty(state.path());
}

#[test]
fn curva_sem_aparelho() {
    let state = tempfile::tempdir().unwrap();
    let out = run(&curve_args("fan", "1500", &[]), state.path());
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("Waterforce não encontrado"));
    assert_empty(state.path());
}

#[test]
fn curva_sem_permissao() {
    // O sysfs falso aponta para /dev/kmsg, que um usuário comum não abre para
    // escrita. Como root ele abriria, então o teste não roda.
    if fs::metadata("/proc/self").unwrap().uid() == 0 {
        eprintln!("teste pulado: rodando como root");
        return;
    }
    let sys = sys_with_castor3("kmsg");
    let state = tempfile::tempdir().unwrap();
    let out = run_in(
        &curve_args("fan", "1500", &["--no-save"]),
        sys.path(),
        state.path(),
    );
    assert_eq!(out.status.code(), Some(4), "{out:?}");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("--no-save é experimental"), "{err}");
    assert!(err.contains("sem permissão para abrir /dev/kmsg"), "{err}");
    assert!(err.contains("packaging/udev/70-halo.rules"), "{err}");
    assert_empty(state.path());
}
