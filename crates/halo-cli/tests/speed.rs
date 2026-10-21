// SPDX-License-Identifier: GPL-2.0-or-later

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

/// Roda o `halo` com `--sys-root` vazio e o estado em `state`.
fn run(args: &[&str], state: &Path) -> Output {
    let sys = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_halo"))
        .args(args)
        .arg("--sys-root")
        .arg(sys.path())
        .env("XDG_STATE_HOME", state)
        .output()
        .unwrap()
}

fn assert_empty(dir: &Path) {
    assert_eq!(fs::read_dir(dir).unwrap().count(), 0, "{}", dir.display());
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
