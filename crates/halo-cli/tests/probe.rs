// SPDX-License-Identifier: GPL-2.0-or-later

use std::fs;
use std::process::Command;

fn halo() -> Command {
    Command::new(env!("CARGO_BIN_EXE_halo"))
}

#[test]
fn ajuda_do_probe() {
    let out = halo().args(["probe", "it5702", "--help"]).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--raw"));
    assert!(!text.contains("--sysfs-root"));
}

#[test]
fn nao_encontrado_sem_enviar_nada() {
    // Árvore sysfs falsa só com um dispositivo de outro VID:PID.
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("hidraw0/device");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("uevent"), "HID_ID=0003:0000046D:0000C08B\n").unwrap();
    fs::write(
        dir.join("report_descriptor"),
        [0x06, 0x89, 0xFF, 0xA1, 0x01, 0xC0],
    )
    .unwrap();

    let out = halo()
        .args(["probe", "it5702", "--sysfs-root"])
        .arg(root.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("controlador IT5702 não encontrado"), "{err}");
}
