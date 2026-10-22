// SPDX-License-Identifier: GPL-2.0-or-later

//! Testes do binário `halod` num `dbus-daemon` privado, com sysfs e
//! diretórios falsos. Nada aqui abre um aparelho real.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;

const BUS_NAME: &str = "io.github.leoodaviid.Halo1";
const ROOT: &str = "/io/github/leoodaviid/Halo1";
const WATERFORCE: &str = "/io/github/leoodaviid/Halo1/waterforce";

struct Bus {
    child: Child,
    address: String,
    _dir: tempfile::TempDir,
}

impl Bus {
    fn start() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("halo-bus")
            .tempdir_in("/tmp")
            .unwrap();
        let config = dir.path().join("bus.conf");
        std::fs::write(
            &config,
            format!(
                r#"<busconfig><type>session</type><listen>unix:dir={}</listen>
<policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>
"#,
                dir.path().display()
            ),
        )
        .unwrap();
        let mut child = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        Self {
            child,
            address: address.trim().to_owned(),
            _dir: dir,
        }
    }

    fn client(&self) -> Connection {
        Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .unwrap()
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Daemon {
    child: Option<Child>,
}

impl Daemon {
    /// Encerra e devolve a saída, com o stderr.
    fn stop(mut self) -> std::process::Output {
        let mut child = self.child.take().unwrap();
        let _ = child.kill();
        child.wait_with_output().unwrap()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn halod(bus: &Bus, home: &Path, sys: &Path, extra: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_halod"));
    cmd.arg("--address")
        .arg(&bus.address)
        .arg("--sys-root")
        .arg(sys)
        .args(extra)
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_STATE_HOME")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    cmd
}

fn has_owner(client: &Connection) -> bool {
    let reply = client.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "NameHasOwner",
        &(BUS_NAME,),
    );
    reply.unwrap().body().deserialize::<bool>().unwrap()
}

/// Inicia o `halod` e espera ele pegar o nome.
fn start(bus: &Bus, home: &Path, sys: &Path, extra: &[&str]) -> Daemon {
    let daemon = Daemon {
        child: Some(halod(bus, home, sys, extra).spawn().unwrap()),
    };
    let client = bus.client();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !has_owner(&client) {
        assert!(Instant::now() < deadline, "o halod não pegou o nome");
        std::thread::sleep(Duration::from_millis(20));
    }
    daemon
}

fn version(client: &Connection) -> String {
    let reply = client
        .call_method(
            Some(BUS_NAME),
            ROOT,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("io.github.leoodaviid.Halo1.Manager", "Version"),
        )
        .unwrap();
    let value: zbus::zvariant::OwnedValue = reply.body().deserialize().unwrap();
    String::try_from(value).unwrap()
}

fn error_name(err: zbus::Error) -> (String, String) {
    match err {
        zbus::Error::MethodError(name, msg, _) => (name.to_string(), msg.unwrap_or_default()),
        e => panic!("erro inesperado: {e}"),
    }
}

/// Árvore sysfs falsa com um hidraw `name` que se apresenta como o Castor3. O
/// devnode da descoberta é `/dev/<name>`.
fn sys_with_castor3(name: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("class/hidraw").join(name).join("device");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("uevent"),
        "HID_ID=0003:00001044:00007A4D\nHID_NAME=ITE Tech. Castor3\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("report_descriptor"),
        [0x05, 0x00, 0x09, 0x02, 0xA1, 0x01, 0xC0],
    )
    .unwrap();
    root
}

fn apply_static(client: &Connection) -> zbus::Result<zbus::Message> {
    client.call_method(
        Some(BUS_NAME),
        WATERFORCE,
        Some("io.github.leoodaviid.Halo1.Waterforce.Led"),
        "Apply",
        &("static", vec!["ff6600"], 10u8, 0u8),
    )
}

#[test]
fn versao_e_instancia_unica() {
    let bus = Bus::start();
    let home = tempfile::tempdir().unwrap();
    let sys = tempfile::tempdir().unwrap();
    let _first = start(&bus, home.path(), sys.path(), &[]);
    let client = bus.client();
    assert_eq!(version(&client), env!("CARGO_PKG_VERSION"));

    let second = halod(&bus, home.path(), sys.path(), &[]).output().unwrap();
    assert!(!second.status.success());
    let err = String::from_utf8_lossy(&second.stderr);
    assert!(err.contains("o halo já está rodando"), "{err}");
    // O primeiro continua dono do nome e respondendo.
    assert_eq!(version(&client), env!("CARGO_PKG_VERSION"));
}

#[test]
fn sem_logind_continua_respondendo() {
    let bus = Bus::start();
    let home = tempfile::tempdir().unwrap();
    let sys = tempfile::tempdir().unwrap();
    let daemon = start(
        &bus,
        home.path(),
        sys.path(),
        &["--system-address", "unix:path=/caminho/que/nao/existe"],
    );
    let client = bus.client();
    assert_eq!(version(&client), env!("CARGO_PKG_VERSION"));
    let out = daemon.stop();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("o resume não será acompanhado"), "{err}");
}

#[test]
fn led_sem_aparelho() {
    let bus = Bus::start();
    let home = tempfile::tempdir().unwrap();
    let sys = tempfile::tempdir().unwrap();
    let _daemon = start(&bus, home.path(), sys.path(), &[]);
    let (name, msg) = error_name(apply_static(&bus.client()).unwrap_err());
    assert_eq!(name, "io.github.leoodaviid.Halo1.Error.NotFound");
    assert!(msg.contains("1044:7a4d"), "{msg}");
}

#[test]
fn led_sem_permissao() {
    let bus = Bus::start();
    let home = tempfile::tempdir().unwrap();
    // `/dev/kmsg` existe e só o root abre para escrita.
    let sys = sys_with_castor3("kmsg");
    let _daemon = start(&bus, home.path(), sys.path(), &[]);
    let (name, msg) = error_name(apply_static(&bus.client()).unwrap_err());
    assert_eq!(name, "io.github.leoodaviid.Halo1.Error.PermissionDenied");
    assert!(msg.contains("/dev/kmsg") && msg.contains("udev"), "{msg}");
}

#[test]
fn restaurar_sem_snapshot() {
    let bus = Bus::start();
    let home = tempfile::tempdir().unwrap();
    let sys = tempfile::tempdir().unwrap();
    let _daemon = start(&bus, home.path(), sys.path(), &[]);
    let err = bus
        .client()
        .call_method(
            Some(BUS_NAME),
            WATERFORCE,
            Some("io.github.leoodaviid.Halo1.Waterforce.Speed"),
            "Restore",
            &("previous",),
        )
        .unwrap_err();
    let (name, msg) = error_name(err);
    assert_eq!(name, "io.github.leoodaviid.Halo1.Error.Storage");
    assert!(msg.contains("waterforce-speed.previous"), "{msg}");
}
