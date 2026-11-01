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

/// SPD do Fury Beast Refresh desta máquina, com o resto zerado.
fn spd_beast_refresh() -> Vec<u8> {
    let mut spd = vec![0u8; 512];
    spd[2] = 0x0C;
    spd[320] = 0x01;
    spd[321] = 0x98;
    spd[329..349].copy_from_slice(b"KF3200C16D4/16GX    ");
    spd[359] = 0x04;
    spd[360] = 0x23;
    spd
}

/// sysfs falso com o PIIX4 em `i2c-0`, o `i2c-dev` dele e o SPD dado, e um
/// `/dev` com o nó `i2c-0` como arquivo comum no modo dado.
fn ram_fake(spd: &[u8], node_mode: u32) -> (tempfile::TempDir, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let sys = tempfile::tempdir().unwrap();
    let adapter = sys.path().join("bus/i2c/devices/i2c-0");
    fs::create_dir_all(adapter.join("i2c-dev/i2c-0")).unwrap();
    fs::write(adapter.join("name"), "SMBus PIIX4 adapter port 0 at 0b00\n").unwrap();
    let dir = adapter.join("0-0052");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("name"), "ee1004\n").unwrap();
    fs::write(dir.join("eeprom"), spd).unwrap();
    let dev = sys.path().join("fake-dev");
    fs::create_dir_all(&dev).unwrap();
    let node = dev.join("i2c-0");
    fs::write(&node, "").unwrap();
    fs::set_permissions(&node, fs::Permissions::from_mode(node_mode)).unwrap();
    (sys, dev)
}

fn run_ram(args: &[&str], sys: &Path, dev: &Path) -> std::process::Output {
    halo()
        .args(args)
        .arg("--sys-root")
        .arg(sys)
        .arg("--dev-root")
        .arg(dev)
        .output()
        .unwrap()
}

#[test]
fn ram_simulacao_sem_sysfs() {
    let empty = tempfile::tempdir().unwrap();
    let out = run_ram(
        &["led", "ram", "off", "--dry-run"],
        empty.path(),
        empty.path(),
    );
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("0x5a  escreve 0x0B = 0x01"), "{text}");
    assert!(text.contains("0x5b  escreve 0x0B = 0x00"), "{text}");
    assert!(!text.contains("lê"), "{text}");
    assert!(text.contains("0x5b  escreve 0x08 = 0x53"), "{text}");
    assert!(text.contains("0x5a  escreve 0x20 = 0x00"), "{text}");
    assert!(text.contains("0x5b  escreve 0x08 = 0x44"), "{text}");
}

#[test]
fn ram_comando_invalido() {
    let empty = tempfile::tempdir().unwrap();
    let out = run_ram(&["led", "ram", "purple"], empty.path(), empty.path());
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn ram_simulacao_do_estatico() {
    let empty = tempfile::tempdir().unwrap();
    let out = run_ram(
        &[
            "led",
            "ram",
            "static",
            "--color",
            "12ab34",
            "--brightness",
            "50",
            "--dry-run",
        ],
        empty.path(),
        empty.path(),
    );
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("static 12ab34, brilho 50"), "{text}");
    assert!(!text.contains("lê"), "{text}");
    for a in ["0x5a", "0x5b"] {
        for reg in [
            "0x0B = 0x0",
            "0x20 = 0x32",
            "0x31 = 0x12",
            "0x32 = 0xAB",
            "0x33 = 0x34",
        ] {
            let line = format!("{a}  escreve {reg}");
            assert!(text.contains(&line), "{line}: {text}");
        }
    }
    // Duas molduras: dois 0x53 e dois 0x44 por pente.
    assert_eq!(text.matches("escreve 0x08 = 0x53").count(), 4, "{text}");
    assert_eq!(text.matches("escreve 0x08 = 0x44").count(), 4, "{text}");
    assert_eq!(text.matches("escreve").count(), 30, "{text}");
}

/// Entrada inválida sai com 2 sem abrir o nó: com SPD suportado e o nó em modo
/// `000`, abrir daria 4 (ou passaria do open, como root, e daria 5).
#[test]
fn ram_estatico_recusa_entrada_antes_de_abrir() {
    let (sys, dev) = ram_fake(&spd_beast_refresh(), 0o000);
    for args in [
        &[
            "led",
            "ram",
            "static",
            "--color",
            "0000ff",
            "--brightness",
            "101",
        ][..],
        &["led", "ram", "static", "--color", "00ff0"][..],
        &["led", "ram", "static"][..],
        &["led", "ram", "static", "--brightness", "50"][..],
        &["led", "ram", "off", "--color", "0000ff"][..],
        &["led", "ram", "on", "--brightness", "50"][..],
        &["led", "ram", "default", "--color", "0000ff", "--dry-run"][..],
    ] {
        let out = run_ram(args, sys.path(), &dev);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty(), "{args:?}");
    }
}

#[test]
fn ram_estatico_sem_pente_suportado_nao_abre_o_no() {
    let mut spd = spd_beast_refresh();
    spd[360] = 0x00;
    let (sys, dev) = ram_fake(&spd, 0o000);
    let out = run_ram(
        &["led", "ram", "static", "--color", "0000ff"],
        sys.path(),
        &dev,
    );
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn ram_sem_pente_suportado_nao_abre_o_no() {
    let mut spd = spd_beast_refresh();
    spd[360] = 0x00;
    // Nó em modo 000: abrir daria 4; o 3 prova que ele não foi aberto.
    let (sys, dev) = ram_fake(&spd, 0o000);
    let out = run_ram(&["led", "ram", "off"], sys.path(), &dev);
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn ram_sem_permissao_pede_sudo() {
    let (sys, dev) = ram_fake(&spd_beast_refresh(), 0o000);
    if fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(dev.join("i2c-0"))
        .is_ok()
    {
        eprintln!("aviso: o modo 000 não bloqueia a abertura (root?); teste pulado");
        return;
    }
    let out = run_ram(&["led", "ram", "off"], sys.path(), &dev);
    assert_eq!(out.status.code(), Some(4));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("sudo"), "{err}");
}

#[test]
fn ram_ajuda_lista_os_comandos() {
    let out = halo().args(["led", "ram", "--help"]).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for name in [
        "off",
        "on",
        "default",
        "static",
        "restore",
        "suspend",
        "--color",
        "--brightness",
    ] {
        assert!(text.contains(name), "{name}: {text}");
    }
    assert!(text.contains("0 a 100"), "{text}");
    assert!(text.contains("Não lê os pentes"), "{text}");
    assert!(!text.contains("--sys-root"), "{text}");
}

/// Acrescenta ao sysfs falso de `ram_fake` o SPD `instance` do Beast Refresh.
fn add_spd(sys: &Path, instance: &str) {
    let dir = sys.join("bus/i2c/devices/i2c-0").join(instance);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("name"), "ee1004\n").unwrap();
    fs::write(dir.join("eeprom"), spd_beast_refresh()).unwrap();
}

/// Diretório do estado em `0755`: o `tempdir` segue o umask, e um diretório
/// gravável pelo grupo seria recusado pelo restore.
fn state_dir() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

/// Grava `text` como estado em `dir`, com o modo dado.
fn write_state(dir: &Path, text: &str, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let file = dir.join("ram-led");
    fs::write(&file, text).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(mode)).unwrap();
}

fn run_ram_state(args: &[&str], sys: &Path, dev: &Path, state: &Path) -> std::process::Output {
    halo()
        .args(args)
        .arg("--state-dir")
        .arg(state)
        .arg("--sys-root")
        .arg(sys)
        .arg("--dev-root")
        .arg(dev)
        .output()
        .unwrap()
}

const WHITE_ON: &str =
    "version=1\neffect=static\ncolor=ffffff\nbrightness=80\npower=on\nspd=0-0052,0-0053\n";

#[test]
fn ram_estado_nao_e_gravado_sem_aplicar() {
    let (sys, dev) = ram_fake(&spd_beast_refresh(), 0o644);
    let state = state_dir();
    let out = run_ram_state(
        &["led", "ram", "static", "--color", "ffffff", "--dry-run"],
        sys.path(),
        &dev,
        state.path(),
    );
    assert_eq!(out.status.code(), Some(0));
    assert!(!state.path().join("ram-led").exists());
    // Nó que é um arquivo comum: o ioctl falha e nada é aplicado.
    let out = run_ram_state(
        &["led", "ram", "static", "--color", "ffffff"],
        sys.path(),
        &dev,
        state.path(),
    );
    assert_eq!(out.status.code(), Some(5));
    assert!(!state.path().join("ram-led").exists());
}

#[test]
fn ram_restore_e_suspend_sem_estado() {
    let (sys, dev) = ram_fake(&spd_beast_refresh(), 0o000);
    let state = state_dir();
    for action in ["restore", "suspend"] {
        let out = run_ram_state(&["led", "ram", action], sys.path(), &dev, state.path());
        assert_eq!(out.status.code(), Some(0), "{action}");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("nenhum estado salvo"), "{action}: {text}");
    }
}

#[test]
fn ram_restore_recusa_estado_gravavel_por_outros() {
    let (sys, dev) = ram_fake(&spd_beast_refresh(), 0o000);
    let state = state_dir();
    write_state(state.path(), WHITE_ON, 0o666);
    for action in ["restore", "suspend"] {
        let out = run_ram_state(&["led", "ram", action], sys.path(), &dev, state.path());
        assert_eq!(out.status.code(), Some(5), "{action}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("outros usuários"), "{action}: {err}");
    }
    write_state(
        state.path(),
        &WHITE_ON.replace("version=1", "version=2"),
        0o644,
    );
    let out = run_ram_state(&["led", "ram", "restore"], sys.path(), &dev, state.path());
    assert_eq!(out.status.code(), Some(5));
}

#[test]
fn ram_restore_simulacao() {
    let empty = tempfile::tempdir().unwrap();
    let state = state_dir();
    write_state(state.path(), WHITE_ON, 0o644);
    let out = run_ram_state(
        &["led", "ram", "restore", "--dry-run"],
        empty.path(),
        empty.path(),
        state.path(),
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("static ffffff, brilho 80, ligado"), "{text}");
    for a in ["0x5a", "0x5b"] {
        assert!(
            text.contains(&format!("{a}  escreve 0x31 = 0xFF")),
            "{text}"
        );
    }
    assert!(!text.contains("0x20 = 0x00"), "{text}");
    assert_eq!(text.matches("escreve").count(), 30, "{text}");

    write_state(
        state.path(),
        &WHITE_ON
            .replace("ffffff", "ff0000")
            .replace("power=on", "power=off"),
        0o644,
    );
    let out = run_ram_state(
        &["led", "ram", "restore", "--dry-run"],
        empty.path(),
        empty.path(),
        state.path(),
    );
    let text = String::from_utf8_lossy(&out.stdout);
    // As duas molduras do estático e, depois, a do off.
    assert_eq!(text.matches("escreve").count(), 30 + 8, "{text}");
    let last_color = text.rfind("0x31 = 0xFF").unwrap();
    let off = text.rfind("0x20 = 0x00").unwrap();
    assert!(off > last_color, "{text}");

    let out = run_ram_state(
        &["led", "ram", "suspend", "--dry-run"],
        empty.path(),
        empty.path(),
        state.path(),
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(text.matches("escreve").count(), 8, "{text}");
    assert!(text.contains("0x5a  escreve 0x20 = 0x00"), "{text}");
    // Nenhum dos dois grava o estado.
    let saved = fs::read_to_string(state.path().join("ram-led")).unwrap();
    assert!(
        saved.contains("power=off") && saved.contains("ff0000"),
        "{saved}"
    );
}

#[test]
fn ram_restore_espera_os_pentes_salvos() {
    let (sys, dev) = ram_fake(&spd_beast_refresh(), 0o644);
    let state = state_dir();
    write_state(state.path(), WHITE_ON, 0o644);
    let out = run_ram_state(
        &["led", "ram", "restore", "--wait-ms", "300"],
        sys.path(),
        &dev,
        state.path(),
    );
    assert_eq!(out.status.code(), Some(3));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("Faltam: 0-0053"), "{err}");

    // O 0-0053 aparece 200 ms depois do início: o restore passa da espera e
    // chega ao nó (um arquivo comum, onde o ioctl falha com 5).
    let start = std::time::Instant::now();
    let child = halo()
        .args(["led", "ram", "restore", "--wait-ms", "5000", "--state-dir"])
        .arg(state.path())
        .arg("--sys-root")
        .arg(sys.path())
        .arg("--dev-root")
        .arg(&dev)
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(200));
    add_spd(sys.path(), "0-0053");
    let out = child.wait_with_output().unwrap();
    assert!(start.elapsed() < std::time::Duration::from_secs(4));
    assert_eq!(out.status.code(), Some(5));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!err.contains("Faltam"), "{err}");
    assert!(!state.path().join("ram-led.tmp").exists());
}

#[test]
fn ram_restore_recusa_cor() {
    let (sys, dev) = ram_fake(&spd_beast_refresh(), 0o000);
    let state = state_dir();
    write_state(state.path(), WHITE_ON, 0o666);
    // Com o estado 0666, ler daria 5; o 2 prova que o estado não foi lido.
    for args in [
        &["led", "ram", "restore", "--color", "0000ff"][..],
        &["led", "ram", "suspend", "--brightness", "10"][..],
    ] {
        let out = run_ram_state(args, sys.path(), &dev, state.path());
        assert_eq!(out.status.code(), Some(2), "{args:?}");
    }
}
