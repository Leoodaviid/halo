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
        &["probe", "ram", "--help"],
        &["probe", "ram-controller", "--help"],
        &["probe", "waterforce-lcd", "--help"],
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

#[test]
fn ajuda_da_ram_diz_que_so_le_o_sysfs() {
    let out = halo().args(["probe", "ram", "--help"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("ee1004"), "{text}");
    assert!(text.contains("não abre /dev/i2c"), "{text}");
}

/// SPD de 512 bytes com os bytes lidos em 2026-10-31 e o resto zerado.
fn spd_kingston() -> Vec<u8> {
    let mut spd = vec![0u8; 512];
    spd[2] = 0x0C;
    spd[320] = 0x01;
    spd[321] = 0x98;
    spd[329..349].copy_from_slice(b"KF3200C16D4/16GX    ");
    spd[359] = 0x04;
    spd[360] = 0x23;
    spd
}

/// sysfs falso com o adaptador PIIX4 e um `ee1004` por item de `eeproms`.
fn sys_with_ram(eeproms: &[(&str, &[u8])]) -> tempfile::TempDir {
    let sys = tempfile::tempdir().unwrap();
    let adapter = sys.path().join("bus/i2c/devices/i2c-0");
    fs::create_dir_all(&adapter).unwrap();
    fs::write(adapter.join("name"), "SMBus PIIX4 adapter port 0 at 0b00\n").unwrap();
    for (instance, bytes) in eeproms {
        let dir = adapter.join(instance);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("name"), "ee1004\n").unwrap();
        fs::write(dir.join("eeprom"), bytes).unwrap();
    }
    sys
}

#[test]
fn ram_dois_pentes() {
    let spd = spd_kingston();
    let sys = sys_with_ram(&[("0-0052", &spd), ("0-0053", &spd)]);
    let out = run(&["probe", "ram"], sys.path());
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.contains("0-0052 em SMBus PIIX4 adapter port 0 at 0b00"),
        "{text}"
    );
    assert!(
        text.contains("0-0053 em SMBus PIIX4 adapter port 0 at 0b00"),
        "{text}"
    );
    assert!(text.contains("SPD:          0x52 (slot 2)"), "{text}");
    assert!(text.contains("Tipo:         DDR4"), "{text}");
    assert!(text.contains("Fabricante:   Kingston [01 98]"), "{text}");
    assert!(text.contains("Part number:  KF3200C16D4/16GX\n"), "{text}");
    assert_eq!(text.matches("Fury Beast Refresh").count(), 2, "{text}");
    assert!(!text.contains("Bytes brutos"), "{text}");
}

#[test]
fn ram_sem_adaptador() {
    let sys = sys_with_other_device();
    let out = run(&["probe", "ram"], sys.path());
    assert_eq!(out.status.code(), Some(3));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("acpi_enforce_resources=lax"), "{err}");
    assert!(err.contains("docs/hardware.md"), "{err}");
}

#[test]
fn ram_adaptador_sem_ee1004() {
    let sys = sys_with_ram(&[]);
    let out = run(&["probe", "ram"], sys.path());
    assert_eq!(out.status.code(), Some(3));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("ee1004"), "{err}");
}

#[test]
fn ram_spd_curto_ao_lado_de_um_bom() {
    let spd = spd_kingston();
    let sys = sys_with_ram(&[("0-0052", &spd), ("0-0053", &spd[..256])]);
    let out = run(&["probe", "ram"], sys.path());
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{text}");
    let short = text.split("0-0053 em").nth(1).unwrap();
    assert!(short.contains("256 bytes"), "{text}");
    assert!(!short.contains("Part number"), "{text}");
}

#[test]
fn ram_so_spd_curto() {
    let spd = spd_kingston();
    let sys = sys_with_ram(&[("0-0053", &spd[..256])]);
    let out = run(&["probe", "ram"], sys.path());
    assert_eq!(out.status.code(), Some(5));
}

/// Tira a leitura do `eeprom`. Devolve falso quando o modo não bloqueia
/// (rodando como root).
fn deny(sys: &Path, instance: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let eeprom = sys
        .join("bus/i2c/devices/i2c-0")
        .join(instance)
        .join("eeprom");
    fs::set_permissions(&eeprom, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&eeprom).is_ok() {
        eprintln!("aviso: o modo 000 não bloqueia a leitura (root?); teste pulado");
        return false;
    }
    true
}

#[test]
fn ram_sem_permissao_ao_lado_de_um_bom() {
    let spd = spd_kingston();
    let sys = sys_with_ram(&[("0-0052", &spd), ("0-0053", &spd)]);
    if !deny(sys.path(), "0-0053") {
        return;
    }
    let out = run(&["probe", "ram"], sys.path());
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("sem permissão"), "{text}");
    assert!(text.contains("Fury Beast Refresh"), "{text}");
}

#[test]
fn ram_so_sem_permissao() {
    let spd = spd_kingston();
    let sys = sys_with_ram(&[("0-0053", &spd)]);
    if !deny(sys.path(), "0-0053") {
        return;
    }
    let out = run(&["probe", "ram"], sys.path());
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(4), "{text}");
    assert!(text.contains("sem permissão"), "{text}");
}

#[test]
fn ram_bytes_brutos() {
    let spd = spd_kingston();
    let sys = sys_with_ram(&[("0-0052", &spd)]);
    let out = run(&["probe", "ram", "--raw"], sys.path());
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    let rows: Vec<&str> = text
        .lines()
        .skip_while(|l| !l.contains("Bytes brutos:"))
        .skip(1)
        .collect();
    assert_eq!(rows.len(), 32, "{text}");
    assert!(rows[0].starts_with("    000: 00 00 0c 00"), "{text}");
    assert!(rows[31].starts_with("    1f0: "), "{text}");
    for row in &rows {
        assert_eq!(
            row.split_once(": ").unwrap().1.split(' ').count(),
            16,
            "{row}"
        );
    }
    assert!(text.contains("    140: 01 98"), "{text}");
}

/// sysfs falso com o adaptador PIIX4 em `i2c-0`, o `i2c-dev` dele e um `/dev`
/// ao lado. O nó `/dev/i2c-0` é um arquivo comum com o modo dado.
fn ram_with_node(
    eeproms: &[(&str, &[u8])],
    node_mode: Option<u32>,
) -> (tempfile::TempDir, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let sys = sys_with_ram(eeproms);
    fs::create_dir_all(sys.path().join("bus/i2c/devices/i2c-0/i2c-dev/i2c-0")).unwrap();
    let dev = sys.path().join("fake-dev");
    fs::create_dir_all(&dev).unwrap();
    if let Some(mode) = node_mode {
        let node = dev.join("i2c-0");
        fs::write(&node, "").unwrap();
        fs::set_permissions(&node, fs::Permissions::from_mode(mode)).unwrap();
    }
    (sys, dev)
}

fn run_dev(args: &[&str], sys: &Path, dev: &Path) -> std::process::Output {
    halo()
        .args(args)
        .arg("--sys-root")
        .arg(sys)
        .arg("--dev-root")
        .arg(dev)
        .output()
        .unwrap()
}

/// Verdadeiro quando o modo 000 bloqueia a abertura (não é root).
fn mode_blocks(dev: &Path) -> bool {
    let blocked = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(dev.join("i2c-0"))
        .is_err();
    if !blocked {
        eprintln!("aviso: o modo 000 não bloqueia a abertura (root?); teste pulado");
    }
    blocked
}

#[test]
fn controlador_sem_adaptador_nao_abre_o_no() {
    let sys = sys_with_other_device();
    let dev = sys.path().join("fake-dev");
    fs::create_dir_all(&dev).unwrap();
    fs::write(dev.join("i2c-0"), "").unwrap();
    let out = run_dev(&["probe", "ram-controller"], sys.path(), &dev);
    assert_eq!(out.status.code(), Some(3));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("acpi_enforce_resources=lax"), "{err}");
}

#[test]
fn controlador_sem_pente_suportado_nao_abre_o_no() {
    let mut spd = spd_kingston();
    spd[360] = 0x00;
    // Com o nó em modo 000, abrir daria código 4: o 3 prova que ele não foi aberto.
    let (sys, dev) = ram_with_node(&[("0-0052", &spd)], Some(0o000));
    let out = run_dev(&["probe", "ram-controller"], sys.path(), &dev);
    assert_eq!(out.status.code(), Some(3));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("nenhum pente Kingston DDR4 suportado"),
        "{err}"
    );
}

#[test]
fn controlador_recusa_pente_da_api_v1() {
    let mut spd = spd_kingston();
    spd[359] = 0x01; // Fury Beast, API v1
    let (sys, dev) = ram_with_node(&[("0-0052", &spd)], Some(0o000));
    let out = run_dev(&["probe", "ram-controller"], sys.path(), &dev);
    assert_eq!(out.status.code(), Some(3));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("Fury Beast, API v1"), "{err}");
}

#[test]
fn controlador_sem_i2c_dev() {
    let spd = spd_kingston();
    let sys = sys_with_ram(&[("0-0052", &spd)]);
    let dev = sys.path().join("fake-dev");
    fs::create_dir_all(&dev).unwrap();
    let out = run_dev(&["probe", "ram-controller"], sys.path(), &dev);
    assert_eq!(out.status.code(), Some(3));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("i2c-dev"), "{err}");
}

#[test]
fn controlador_sem_permissao_pede_sudo() {
    let spd = spd_kingston();
    let (sys, dev) = ram_with_node(&[("0-0052", &spd)], Some(0o000));
    if !mode_blocks(&dev) {
        return;
    }
    let out = run_dev(&["probe", "ram-controller"], sys.path(), &dev);
    assert_eq!(out.status.code(), Some(4));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("sudo"), "{err}");
    assert!(!err.contains("grupo") && !err.contains("usermod"), "{err}");
}

#[test]
fn controlador_em_arquivo_comum_nao_e_ausencia() {
    let spd = spd_kingston();
    let (sys, dev) = ram_with_node(&[("0-0052", &spd)], Some(0o600));
    let out = run_dev(&["probe", "ram-controller"], sys.path(), &dev);
    assert_eq!(out.status.code(), Some(5));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("erro ao abrir"), "{err}");
    assert!(!err.contains("nada respondeu"), "{err}");
}

#[test]
fn ajuda_do_controlador_avisa_contra_leituras_em_sequencia() {
    let out = halo()
        .args(["probe", "ram-controller", "--help"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Não rode em sequência"), "{text}");
    assert!(text.contains("índice"), "{text}");
}

#[test]
fn waterforce_lcd_nao_encontrado() {
    let sys = sys_with_other_device();
    let out = run(&["probe", "waterforce-lcd"], sys.path());
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stderr).contains("1044:7a4d"));
}

#[test]
fn waterforce_lcd_ajuda_diz_que_so_le() {
    let out = halo()
        .args(["probe", "waterforce-lcd", "--help"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Só lê"), "{text}");
    assert!(text.contains("99 E8"), "{text}");
    assert!(text.contains("Nunca envia comandos de escrita"), "{text}");
    // O probe antigo continua com os sete pedidos.
    let out = halo()
        .args(["probe", "waterforce", "--help"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("D9 01, D9 02, C7 e E8"), "{text}");
}
