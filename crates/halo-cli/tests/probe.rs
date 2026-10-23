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
    let dir = root.path().join("class/hidraw/hidraw96/device");
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

/// hwmon `waterforce` falso com o `temp1_input` informado.
fn sys_with_waterforce_hwmon(temp1_input: &str) -> tempfile::TempDir {
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
        ("temp1_input", temp1_input),
    ] {
        fs::write(dir.join(attr), value).unwrap();
    }
    sys
}

#[test]
fn waterforce_telemetria_de_sysfs_falso() {
    let sys = sys_with_waterforce_hwmon("29000");
    let out = run(&["telemetry", "waterforce"], sys.path());
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Líquido:  29,0 °C"), "{text}");
    assert!(out.stderr.is_empty());
}

#[test]
fn waterforce_telemetria_com_liquido_invalido() {
    let sys = sys_with_waterforce_hwmon("224000");
    let out = run(&["telemetry", "waterforce"], sys.path());
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Líquido:  inválido (o aparelho informou 224 °C)"),
        "{text}"
    );
    assert!(
        text.contains("Fan:      2204 RPM, duty 224 (88%)"),
        "{text}"
    );
    assert!(!text.contains("corte de energia"), "{text}");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("corte de energia"), "{err}");
    assert!(err.contains("docs/hardware.md"), "{err}");
}

#[test]
fn telemetria_recusa_intervalo_curto() {
    let sys = sys_with_other_device();
    let out = run(&["telemetry", "waterforce", "--watch", "1"], sys.path());
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("mínimo é 2 segundos"), "{err}");
}
