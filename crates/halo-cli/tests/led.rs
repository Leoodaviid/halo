// SPDX-License-Identifier: GPL-2.0-or-later

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;

fn halo() -> Command {
    Command::new(env!("CARGO_BIN_EXE_halo"))
}

fn run(args: &[&str], sys: &Path) -> std::process::Output {
    halo()
        .args(args)
        .arg("--sys-root")
        .arg(sys)
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
    // Usage page 0x0000, usage 0x0002, coleção vazia.
    fs::write(
        dir.join("report_descriptor"),
        [0x05, 0x00, 0x09, 0x02, 0xA1, 0x01, 0xC0],
    )
    .unwrap();
    root
}

#[test]
fn ajuda() {
    let out = halo()
        .args(["led", "waterforce", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("tri-color"), "{text}");
    assert!(!text.contains("--sys-root"), "{text}");
}

#[test]
fn dry_run_sem_sysfs() {
    let missing = Path::new("/caminho/que/nao/existe");
    let out = run(
        &[
            "led",
            "waterforce",
            "static",
            "--color",
            "ff6600",
            "--dry-run",
        ],
        missing,
    );
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "99 C9 01 64 0A 02 01\n99 CD FF 66 00\n99 B6\n"
    );
}

#[test]
fn nao_encontrado() {
    let sys = tempfile::tempdir().unwrap();
    let out = run(&["led", "waterforce", "cycle"], sys.path());
    assert_eq!(out.status.code(), Some(3));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("Waterforce não encontrado"), "{err}");
}

#[test]
fn sem_permissao() {
    // O sysfs falso aponta para /dev/kmsg (crw-r--r--), que um usuário comum
    // não abre para escrita. Como root ele abriria, então o teste não roda.
    if fs::metadata("/proc/self").unwrap().uid() == 0 {
        eprintln!("teste pulado: rodando como root");
        return;
    }
    let sys = sys_with_castor3("kmsg");
    let out = run(&["led", "waterforce", "cycle"], sys.path());
    assert_eq!(out.status.code(), Some(4));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("sem permissão para abrir /dev/kmsg"), "{err}");
    assert!(err.contains("packaging/udev/70-halo.rules"), "{err}");
}

#[test]
fn entradas_invalidas_saem_com_2() {
    let missing = Path::new("/caminho/que/nao/existe");
    for (args, msg) in [
        (&["pulse", "--speed", "6"][..], "de 1 a 5"),
        (&["pulse", "--brightness", "0"], "de 1 a 10"),
        (&["cycle", "--color", "ff0000"], "não aceita cor"),
        (
            &["static", "--color", "ff0000", "--color", "00ff00"],
            "uma cor só",
        ),
        (&["static", "--color", "laranja"], "cor inválida"),
        (&["piscar"], "invalid value"),
    ] {
        let mut full = vec!["led", "waterforce"];
        full.extend(args);
        let out = run(&full, missing);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains(msg), "{args:?}: {err}");
    }

    let mut nine = vec!["led", "waterforce", "aurora"];
    for _ in 0..9 {
        nine.extend(["--color", "ff0000"]);
    }
    let out = run(&nine, missing);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no máximo 8"));
}
