// SPDX-License-Identifier: GPL-2.0-or-later

use std::fs;
use std::path::Path;
use std::process::Command;

fn halo() -> Command {
    Command::new(env!("CARGO_BIN_EXE_halo"))
}

/// Árvore sysfs falsa com um hidraw de outro VID:PID.
fn sys_with_other_device() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("class/hidraw/hidraw0/device");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("uevent"), "HID_ID=0003:0000046D:0000C08B\n").unwrap();
    fs::write(
        dir.join("report_descriptor"),
        [0x06, 0x89, 0xFF, 0xA1, 0x01, 0xC0],
    )
    .unwrap();
    root
}

fn run(args: &[&str], sys: &Path) -> std::process::Output {
    halo()
        .args(args)
        .arg("--sys-root")
        .arg(sys)
        .output()
        .unwrap()
}

#[test]
fn ajuda_dos_comandos() {
    for args in [
        &["probe", "it5702", "--help"][..],
        &["probe", "waterforce", "--help"],
        &["telemetry", "waterforce", "--help"],
    ] {
        let out = halo().args(args).output().unwrap();
        assert!(out.status.success(), "{args:?}");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(!text.contains("--sys-root"), "{args:?}");
    }
}

#[test]
fn ajuda_do_probe_lista_os_pedidos() {
    let out = halo().args(["probe", "--help"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("D6, DE 00, DD"), "{text}");
}

#[test]
fn it5702_nao_encontrado() {
    let sys = sys_with_other_device();
    let out = run(&["probe", "it5702"], sys.path());
    assert_eq!(out.status.code(), Some(3));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("controlador IT5702 não encontrado"), "{err}");
}

#[test]
fn waterforce_probe_nao_encontrado() {
    let sys = sys_with_other_device();
    let out = run(&["probe", "waterforce"], sys.path());
    assert_eq!(out.status.code(), Some(3));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("Waterforce não encontrado"), "{err}");
}

#[test]
fn waterforce_telemetria_de_sysfs_falso() {
    let sys = tempfile::tempdir().unwrap();
    let dir = sys.path().join("class/hwmon/hwmon6");
    fs::create_dir_all(dir.join("device")).unwrap();
    fs::write(dir.join("name"), "waterforce\n").unwrap();
    fs::write(dir.join("device/uevent"), "HID_ID=0003:00001044:00007A4D\n").unwrap();
    for (attr, value) in [
        ("fan1_input", "2204"),
        ("fan2_input", "3030"),
        ("pwm1", "224"),
        ("pwm2", "255"),
        ("temp1_input", "29000"),
    ] {
        fs::write(dir.join(attr), value).unwrap();
    }

    let out = run(&["telemetry", "waterforce"], sys.path());
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Líquido:  29,0 °C"), "{text}");
}

#[test]
fn telemetria_recusa_intervalo_curto() {
    let sys = sys_with_other_device();
    let out = run(&["telemetry", "waterforce", "--watch", "1"], sys.path());
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("mínimo é 2 segundos"), "{err}");
}
