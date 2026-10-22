// SPDX-License-Identifier: GPL-2.0-or-later

//! Ambiente dos testes das interfaces: um `dbus-daemon` privado, os objetos do
//! daemon servidos com o backend falso e um cliente.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use zbus::{Connection, MatchRule, Message, MessageStream, connection};

use crate::bus::{self, BUS_NAME};
use crate::profiles::ProfileStore;
use crate::worker::Worker;
use crate::worker::fake::FakeBackend;

/// `dbus-daemon` de sessão só para um teste, com o socket num diretório curto
/// em `/tmp` (o caminho do socket tem limite de tamanho).
pub struct TestBus {
    child: Child,
    pub address: String,
    _dir: tempfile::TempDir,
}

impl TestBus {
    pub fn start() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("halo-bus")
            .tempdir_in("/tmp")
            .unwrap();
        let config = dir.path().join("bus.conf");
        std::fs::write(
            &config,
            format!(
                r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:dir={}</listen>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
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
}

impl Drop for TestBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// O daemon com o backend falso, um sysfs falso e diretórios temporários.
pub struct Harness {
    pub backend: FakeBackend,
    pub server: Connection,
    pub client: Connection,
    pub sys: PathBuf,
    pub config: PathBuf,
    pub state: PathBuf,
    _dirs: tempfile::TempDir,
    _bus: TestBus,
}

impl Harness {
    pub fn new() -> Self {
        let bus = TestBus::start();
        let dirs = tempfile::tempdir().unwrap();
        let sys = dirs.path().join("sys");
        let config = dirs.path().join("config/halo");
        let state = dirs.path().join("state/halo");
        std::fs::create_dir_all(&sys).unwrap();
        let backend = FakeBackend::default();
        let worker = Worker::spawn(backend.clone());
        let (server, client) = zbus::block_on(async {
            let server = bus::serve(
                connection::Builder::address(bus.address.as_str()).unwrap(),
                worker,
                ProfileStore::new(&config, &state),
            )
            .and_then(bus::claim_name)
            .unwrap()
            .build()
            .await
            .unwrap();
            let client = connection::Builder::address(bus.address.as_str())
                .unwrap()
                .build()
                .await
                .unwrap();
            (server, client)
        });
        Self {
            backend,
            server,
            client,
            sys,
            config,
            state,
            _dirs: dirs,
            _bus: bus,
        }
    }

    pub fn profiles(&self) -> ProfileStore {
        ProfileStore::new(&self.config, &self.state)
    }

    /// Chama `method` e devolve a resposta, ou o nome do erro D-Bus.
    pub fn call<B, R>(&self, path: &str, iface: &str, method: &str, body: &B) -> Result<R, String>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
        R: DeserializeOwned + zbus::zvariant::Type,
    {
        zbus::block_on(async {
            let reply = self
                .client
                .call_method(Some(BUS_NAME), path, Some(iface), method, body)
                .await
                .map_err(error_name)?;
            Ok(reply.body().deserialize::<R>().unwrap())
        })
    }

    pub fn property<R>(&self, path: &str, iface: &str, name: &str) -> R
    where
        R: TryFrom<zbus::zvariant::OwnedValue>,
        R::Error: std::fmt::Debug,
    {
        let value: zbus::zvariant::OwnedValue = self
            .call(
                path,
                "org.freedesktop.DBus.Properties",
                "Get",
                &(iface, name),
            )
            .unwrap();
        R::try_from(value).unwrap()
    }

    /// Sinais `member` de `iface` emitidos pelo daemon a partir de agora.
    pub fn signals(&self, iface: &'static str, member: &'static str) -> MessageStream {
        zbus::block_on(async {
            let rule = MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .interface(iface)
                .unwrap()
                .member(member)
                .unwrap()
                .build();
            MessageStream::for_match_rule(rule, &self.client, None)
                .await
                .unwrap()
        })
    }
}

/// O próximo sinal em `stream`, ou `None` depois de `wait`.
pub fn next_signal(stream: &mut MessageStream, wait: Duration) -> Option<Message> {
    zbus::block_on(async {
        let timer = async_io::Timer::after(wait);
        match futures_util::future::select(stream.next(), timer).await {
            futures_util::future::Either::Left((Some(Ok(msg)), _)) => Some(msg),
            _ => None,
        }
    })
}

/// Nomes das propriedades num `PropertiesChanged`.
pub fn changed_properties(msg: &Message) -> Vec<String> {
    let (_, changed, invalidated): (
        String,
        std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        Vec<String>,
    ) = msg.body().deserialize().unwrap();
    let mut names: Vec<String> = changed.into_keys().chain(invalidated).collect();
    names.sort();
    names
}

fn error_name(e: zbus::Error) -> String {
    match e {
        zbus::Error::MethodError(name, _, _) => name.to_string(),
        e => panic!("erro inesperado: {e}"),
    }
}

/// Árvore sysfs falsa: hidraw `name` com VID:PID e usage page dados.
pub fn add_hidraw(root: &Path, name: &str, vid: u16, pid: u16, page: u16) {
    let dir = root.join("class/hidraw").join(name).join("device");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("uevent"),
        format!("HID_ID=0003:0000{vid:04X}:0000{pid:04X}\nHID_NAME=x\n"),
    )
    .unwrap();
    let [hi, lo] = page.to_be_bytes();
    // Usage page longa (06 lo hi), usage 1, coleção vazia.
    std::fs::write(
        dir.join("report_descriptor"),
        [0x06, lo, hi, 0x09, 0x01, 0xA1, 0x01, 0xC0],
    )
    .unwrap();
}

/// hwmon `waterforce` falso ligado ao Castor3.
pub fn add_hwmon(root: &Path, fan_rpm: u32, coolant_millicelsius: i32) {
    let dir = root.join("class/hwmon/hwmon4");
    std::fs::create_dir_all(dir.join("device")).unwrap();
    std::fs::write(dir.join("name"), "waterforce\n").unwrap();
    std::fs::write(
        dir.join("device/uevent"),
        "DRIVER=gigabyte_waterforce\nHID_ID=0003:00001044:00007A4D\n",
    )
    .unwrap();
    for (attr, value) in [
        ("fan1_input", fan_rpm.to_string()),
        ("fan2_input", "2600".to_owned()),
        ("pwm1", "120".to_owned()),
        ("pwm2", "200".to_owned()),
        ("temp1_input", coolant_millicelsius.to_string()),
    ] {
        std::fs::write(dir.join(attr), format!("{value}\n")).unwrap();
    }
}
