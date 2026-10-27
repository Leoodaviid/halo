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
        .env_remove("WATCHDOG_USEC")
        .env_remove("WATCHDOG_PID")
        .env_remove("NOTIFY_SOCKET")
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

/// hwmon `waterforce` falso ligado ao Castor3, com o líquido a 26 °C.
fn add_hwmon(sys: &Path) {
    let dir = sys.join("class/hwmon/hwmon4");
    std::fs::create_dir_all(dir.join("device")).unwrap();
    std::fs::write(dir.join("name"), "waterforce\n").unwrap();
    std::fs::write(
        dir.join("device/uevent"),
        "DRIVER=gigabyte_waterforce\nHID_ID=0003:00001044:00007A4D\n",
    )
    .unwrap();
    for (attr, value) in [
        ("fan1_input", "1200"),
        ("fan2_input", "2600"),
        ("pwm1", "120"),
        ("pwm2", "200"),
        ("temp1_input", "26000"),
    ] {
        std::fs::write(dir.join(attr), format!("{value}\n")).unwrap();
    }
}

/// A primeira chamada depois de o nome aparecer, como a que sobe o daemon
/// pela ativação do D-Bus, já recebe a lista e a telemetria lidas do sysfs.
#[test]
fn primeira_chamada_ja_tem_aparelhos_e_telemetria() {
    let bus = Bus::start();
    let home = tempfile::tempdir().unwrap();
    let sys = sys_with_castor3("hidraw99");
    add_hwmon(sys.path());
    let _daemon = start(&bus, home.path(), sys.path(), &[]);
    let client = bus.client();

    let reply = client
        .call_method(
            Some(BUS_NAME),
            ROOT,
            Some("io.github.leoodaviid.Halo1.Manager"),
            "Devices",
            &(),
        )
        .unwrap();
    let devices: Vec<std::collections::HashMap<String, zbus::zvariant::OwnedValue>> =
        reply.body().deserialize().unwrap();
    assert_eq!(devices.len(), 3);
    let waterforce = &devices[0];
    assert_eq!(<&str>::try_from(&waterforce["id"]).unwrap(), "waterforce");
    assert!(bool::try_from(&waterforce["present"]).unwrap());

    let reply = client
        .call_method(
            Some(BUS_NAME),
            WATERFORCE,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(
                "io.github.leoodaviid.Halo1.Waterforce.Telemetry",
                "Available",
            ),
        )
        .unwrap();
    let available: zbus::zvariant::OwnedValue = reply.body().deserialize().unwrap();
    assert!(bool::try_from(available).unwrap());
}

/// O arquivo é lido antes de publicar o nome; nem um Get inicial abre hidraw.
#[test]
fn primeiro_get_ja_tem_led_guardado_sem_abrir_hidraw() {
    use std::collections::HashMap;
    use zbus::zvariant::OwnedValue;

    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (text, valid) in [
        (
            "effect = \"static\"\ncolors = [\"ff0000\"]\nbrightness = 10\nspeed = 5\n",
            true,
        ),
        ("effect = \"static\"\ncolors = [\"zzzzzz\"]\n", false),
    ] {
        let bus = Bus::start();
        let home = tempfile::tempdir().unwrap();
        let state = home.path().join(".local/state/halo");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(state.join("waterforce-led.last"), text).unwrap();
        let sys = tempfile::tempdir().unwrap();
        let fake = Command::new("sh")
            .arg(project.join("scripts/dev/fake-sysfs.sh"))
            .arg(sys.path())
            .env_remove("HALO_FAKE_HIDRAW")
            .output()
            .unwrap();
        assert!(
            fake.status.success(),
            "{}",
            String::from_utf8_lossy(&fake.stderr)
        );

        // strace dá evidência de syscalls, inclusive tentativas de abrir hidraw.
        let trace = home.path().join("opens.log");
        let command = halod(&bus, home.path(), sys.path(), &[]);
        let mut traced = Command::new("strace");
        traced
            .args([
                "--kill-on-exit",
                "-f",
                "-e",
                "trace=open,openat,openat2",
                "-o",
            ])
            .arg(&trace)
            .arg(command.get_program())
            .args(command.get_args());
        for (key, value) in command.get_envs() {
            if let Some(value) = value {
                traced.env(key, value);
            } else {
                traced.env_remove(key);
            }
        }
        let daemon = Daemon {
            child: Some(
                traced
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap(),
            ),
        };
        let client = bus.client();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !has_owner(&client) {
            assert!(
                Instant::now() < deadline,
                "halod sob strace não pegou o nome"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let reply = client
            .call_method(
                Some(BUS_NAME),
                WATERFORCE,
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &("io.github.leoodaviid.Halo1.Waterforce.Led", "LastApplied"),
            )
            .unwrap();
        let value: OwnedValue = reply.body().deserialize().unwrap();
        let last: HashMap<String, OwnedValue> = value.try_into().unwrap();
        if valid {
            assert_eq!(<&str>::try_from(&last["effect"]).unwrap(), "static");
            let colors: Vec<String> = last["colors"].try_clone().unwrap().try_into().unwrap();
            assert_eq!(colors, ["ff0000"]);
            assert_eq!(u8::try_from(&last["brightness"]).unwrap(), 10);
            assert_eq!(u8::try_from(&last["speed"]).unwrap(), 5);
        } else {
            assert!(last.is_empty());
        }
        let out = daemon.stop();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(stderr.contains("não consegui ler"), !valid, "{stderr}");
        if valid {
            assert!(!stderr.contains("não consegui"), "{stderr}");
        }
        let opens = std::fs::read_to_string(&trace).unwrap();
        assert!(opens.contains("waterforce-led.last"), "{opens}");
        assert!(!opens.contains("\"/dev/hidraw"), "{opens}");
    }
}

// waterforce-cpu-feed

fn sigterm(daemon: &mut Daemon) -> (std::process::Output, Duration) {
    let child = daemon.child.take().unwrap();
    // SAFETY: o PID é de um filho vivo deste teste.
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
    let started = Instant::now();
    let out = child.wait_with_output().unwrap();
    (out, started.elapsed())
}

#[test]
fn cpu_feed_sigterm_processo_sem_supervisao() {
    let bus = Bus::start();
    let home = tempfile::tempdir().unwrap();
    let sys = tempfile::tempdir().unwrap();
    let mut daemon = start(&bus, home.path(), sys.path(), &[]);
    let (out, took) = sigterm(&mut daemon);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    assert!(took < Duration::from_secs(18), "{took:?}");
    assert!(err.contains("feed da CPU desligado"), "{err}");
    assert!(err.contains("sinal 15 recebido"), "{err}");
    // Sem supervisão nunca houve E0, e a saída não abre aparelho.
    assert!(!err.contains("E0 de 80"), "{err}");
}

#[test]
fn cpu_feed_watchdog_processo_supervisionado() {
    use std::os::unix::net::UnixDatagram;
    let bus = Bus::start();
    let home = tempfile::tempdir().unwrap();
    // Sem Castor3: o scan avança e o watchdog recebe progresso legítimo.
    let sys = tempfile::tempdir().unwrap();
    let notify = home.path().join("notify");
    let socket = UnixDatagram::bind(&notify).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(6)))
        .unwrap();
    let mut cmd = halod(&bus, home.path(), sys.path(), &[]);
    cmd.env("WATCHDOG_USEC", "20000000")
        .env("NOTIFY_SOCKET", &notify);
    let mut daemon = Daemon {
        child: Some(cmd.spawn().unwrap()),
    };
    let mut buf = [0u8; 64];
    for _ in 0..2 {
        let n = socket.recv(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"WATCHDOG=1");
    }
    let (out, took) = sigterm(&mut daemon);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    assert!(took < Duration::from_secs(18), "{took:?}");
    assert!(err.contains("watchdog de 20s; feed da CPU ligado"), "{err}");
}

#[test]
fn cpu_feed_watchdog_processo_ambiente_incompativel() {
    let bus = Bus::start();
    let home = tempfile::tempdir().unwrap();
    let sys = tempfile::tempdir().unwrap();
    let mut cmd = halod(&bus, home.path(), sys.path(), &[]);
    cmd.env("WATCHDOG_USEC", "20000000")
        .env("WATCHDOG_PID", "1")
        .env("NOTIFY_SOCKET", "/caminho/que/nao/existe");
    let mut daemon = Daemon {
        child: Some(cmd.spawn().unwrap()),
    };
    let client = bus.client();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !has_owner(&client) {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    // Leituras e operações legadas seguem; o feed fica desligado.
    assert_eq!(version(&client), env!("CARGO_PKG_VERSION"));
    let (out, _) = sigterm(&mut daemon);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("ambiente do watchdog incompatível"), "{err}");
}

/// `halod --waterforce-cpu-safe` sem D-Bus, HOME nem estado.
fn safe_cmd(sys: &Path, node: Option<&Path>) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_halod"));
    cmd.env_clear()
        .arg("--waterforce-cpu-safe")
        .arg("--sys-root")
        .arg(sys)
        .arg("--proc-root")
        .arg("/caminho/que/nao/existe")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if let Some(node) = node {
        cmd.arg("--hidraw-node").arg(node);
    }
    cmd
}

/// Nó falso: um arquivo comum. O `DE` escrito ocupa os primeiros 6144 bytes;
/// a "resposta" é o que já está depois deles.
fn fake_node(dir: &Path, model: u8) -> std::path::PathBuf {
    let node = dir.join("hidraw250");
    let mut data = vec![0u8; 6144];
    data.extend([0x99, 0xDE, model]);
    std::fs::write(&node, data).unwrap();
    node
}

/// Pacotes de 6144 bytes escritos depois da resposta, pelo comando.
fn written_after_reply(node: &Path) -> Vec<Vec<u8>> {
    let data = std::fs::read(node).unwrap();
    data[6147..].chunks(6144).map(<[u8]>::to_vec).collect()
}

#[test]
fn cpu_feed_stop_post_ausente() {
    let sys = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let out = safe_cmd(sys.path(), None).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    assert!(err.contains("ausente"), "{err}");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn cpu_feed_stop_post_envia_e0_alto_e_espera() {
    let sys = sys_with_castor3("hidraw250");
    let dir = tempfile::tempdir().unwrap();
    let node = fake_node(dir.path(), 0);
    let started = Instant::now();
    let out = safe_cmd(sys.path(), Some(&node)).output().unwrap();
    let took = started.elapsed();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    assert!(
        took >= Duration::from_secs(8) && took < Duration::from_secs(18),
        "{took:?}"
    );
    let packets = written_after_reply(&node);
    assert_eq!(packets.len(), 2, "{packets:?}");
    assert!(packets.iter().all(|p| p.len() == 6144));
    // E1 com o nome de reserva (metadados ausentes) e E0 de 80 °C zerado.
    assert_eq!(packets[0][..6], [0x99, 0xE1, 3, b'C', b'P', b'U']);
    assert_eq!(
        packets[1][..13],
        [0x99, 0xE0, 0, 80, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    // O DE saiu no começo; nada de B6, E5, E6.
    let data = std::fs::read(&node).unwrap();
    assert_eq!(data[..3], [0x99, 0xDE, 0x00]);
    for p in &packets {
        assert!(!matches!(p[1], 0xB6 | 0xE5 | 0xE6));
    }

    // Repetir manda a mesma sequência.
    let again = fake_node(dir.path(), 0);
    assert!(
        safe_cmd(sys.path(), Some(&again))
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(written_after_reply(&again), packets);
}

#[test]
fn cpu_feed_stop_post_modelos() {
    let sys = sys_with_castor3("hidraw250");
    let dir = tempfile::tempdir().unwrap();
    // Modelo 1: sem feed, sucesso sem E1/E0 nem espera.
    let node = fake_node(dir.path(), 1);
    let started = Instant::now();
    let out = safe_cmd(sys.path(), Some(&node)).output().unwrap();
    assert!(out.status.success());
    assert!(started.elapsed() < Duration::from_secs(4));
    assert!(written_after_reply(&node).is_empty());
    // Fora da tabela: falha explícita.
    let node = fake_node(dir.path(), 7);
    let out = safe_cmd(sys.path(), Some(&node)).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        err.contains("NÃO protegido") && err.contains("(7)"),
        "{err}"
    );
    assert!(written_after_reply(&node).is_empty());
    // Sem resposta ao DE: falha.
    let silent = dir.path().join("silent");
    std::fs::write(&silent, b"").unwrap();
    let out = safe_cmd(sys.path(), Some(&silent)).output().unwrap();
    assert!(!out.status.success());
}

#[test]
fn cpu_feed_stop_post_ocupado() {
    use std::os::fd::AsRawFd;
    let sys = sys_with_castor3("hidraw250");
    let dir = tempfile::tempdir().unwrap();
    let node = fake_node(dir.path(), 0);
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&node)
        .unwrap();
    // SAFETY: o descritor pertence a `held`, vivo até o fim do teste.
    assert_eq!(unsafe { libc::flock(held.as_raw_fd(), libc::LOCK_EX) }, 0);
    let started = Instant::now();
    let out = safe_cmd(sys.path(), Some(&node)).output().unwrap();
    let took = started.elapsed();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "{err}");
    assert!(err.contains("ocupado"), "{err}");
    assert!(
        took >= Duration::from_secs(5) && took < Duration::from_secs(7),
        "{took:?}"
    );
    // Nenhum byte escrito.
    assert_eq!(std::fs::read(&node).unwrap().len(), 6147);
    drop(held);
}
