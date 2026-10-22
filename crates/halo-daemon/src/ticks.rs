// SPDX-License-Identifier: GPL-2.0-or-later

//! Tarefas periódicas e resume. Nesta versão, os ticks só leem o sysfs: a
//! presença dos aparelhos e a telemetria do hwmon. Nenhum abre hidraw.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_channel::{Receiver, Sender};
use futures_util::StreamExt;
use futures_util::future::{Either, select};
use halo_core::hid::{DeviceMatch, find_devices};
use halo_core::waterforce::telemetry::{self, Coolant, TelemetryError};
use halo_core::{it5702, waterforce};
use zbus::Connection;

use crate::bus::{DeviceEntry, Manager, ROOT_PATH, Telemetry, TelemetryState, WATERFORCE_PATH};

/// O intervalo mínimo do status no driver do kernel.
pub const INTERVAL: Duration = Duration::from_secs(2);

const DEVICES: [(&str, DeviceMatch); 2] = [
    ("it5702", it5702::DEVICE),
    ("waterforce", waterforce::DEVICE),
];

/// Presença de cada aparelho, só pelo sysfs.
pub fn scan_devices(sys_root: &Path) -> Vec<DeviceEntry> {
    let class = sys_root.join("class/hidraw");
    DEVICES
        .iter()
        .map(|(id, wanted)| {
            let devnode = find_devices(&class, *wanted)
                .ok()
                .and_then(|found| found.into_iter().next())
                .map(|info| info.devnode.to_string_lossy().into_owned());
            (
                (*id).to_owned(),
                devnode.is_some(),
                devnode.unwrap_or_default(),
            )
        })
        .collect()
}

/// Próximo estado da telemetria a partir de uma leitura do hwmon.
pub fn next_telemetry(
    prev: TelemetryState,
    read: Result<telemetry::Telemetry, TelemetryError>,
) -> TelemetryState {
    match read {
        Ok(t) => TelemetryState {
            available: true,
            fan_rpm: t.fan_rpm,
            pump_rpm: t.pump_rpm,
            fan_duty: t.fan_duty,
            pump_duty: t.pump_duty,
            coolant_millicelsius: t.coolant_millicelsius,
            coolant_valid: matches!(t.coolant(), Coolant::Valid(_)),
        },
        Err(_) => TelemetryState {
            available: false,
            ..prev
        },
    }
}

pub struct Ticks {
    sys_root: PathBuf,
}

impl Ticks {
    pub fn new(sys_root: PathBuf) -> Self {
        Self { sys_root }
    }

    /// Lê presença e telemetria e emite os sinais só do que mudou.
    pub async fn run_once(&self, conn: &Connection) -> zbus::Result<()> {
        let server = conn.object_server();

        let devices = scan_devices(&self.sys_root);
        let manager = server.interface::<_, Manager>(ROOT_PATH).await?;
        let changed = {
            let mut m = manager.get_mut().await;
            let changed = m.devices != devices;
            m.devices = devices.clone();
            changed
        };
        if changed {
            Manager::devices_changed(manager.signal_emitter(), devices).await?;
        }

        let iface = server.interface::<_, Telemetry>(WATERFORCE_PATH).await?;
        let (old, new) = {
            let mut t = iface.get_mut().await;
            let old = t.state;
            t.state = next_telemetry(old, telemetry::read(&self.sys_root));
            (old, t.state)
        };
        let t = iface.get().await;
        let e = iface.signal_emitter();
        if old.available != new.available {
            t.available_changed(e).await?;
        }
        if old.fan_rpm != new.fan_rpm {
            t.fan_rpm_changed(e).await?;
        }
        if old.pump_rpm != new.pump_rpm {
            t.pump_rpm_changed(e).await?;
        }
        if old.fan_duty != new.fan_duty {
            t.fan_duty_changed(e).await?;
        }
        if old.pump_duty != new.pump_duty {
            t.pump_duty_changed(e).await?;
        }
        if old.coolant_millicelsius != new.coolant_millicelsius {
            t.coolant_milli_celsius_changed(e).await?;
        }
        if old.coolant_valid != new.coolant_valid {
            t.coolant_valid_changed(e).await?;
        }
        Ok(())
    }

    /// Roda a cada [`INTERVAL`] e, fora do ritmo, a cada pedido em `refresh`.
    pub async fn run(&self, conn: &Connection, refresh: Receiver<()>) {
        loop {
            if let Err(e) = self.run_once(conn).await {
                eprintln!("halod: erro nas tarefas periódicas: {e}");
            }
            let timer = async_io::Timer::after(INTERVAL);
            let wake = refresh.recv();
            futures_util::pin_mut!(wake);
            if let Either::Right((Err(_), _)) = select(timer, wake).await {
                // Ninguém mais pede atualização fora do ritmo.
                async_io::Timer::after(INTERVAL).await;
            }
        }
    }
}

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Login1Manager {
    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
}

/// `PrepareForSleep(false)` é o fim da suspensão: pede uma atualização.
pub fn on_prepare_for_sleep(start: bool, refresh: &Sender<()>) {
    if !start {
        let _ = refresh.try_send(());
    }
}

async fn connect_system(address: Option<&str>) -> zbus::Result<Connection> {
    match address {
        Some(a) => zbus::connection::Builder::address(a)?.build().await,
        None => Connection::system().await,
    }
}

/// Acompanha o resume pelo logind. Sem D-Bus do sistema, só avisa.
pub async fn watch_resume(system_address: Option<String>, refresh: Sender<()>) {
    let result = async {
        let conn = connect_system(system_address.as_deref()).await?;
        let proxy = Login1ManagerProxy::new(&conn).await?;
        let mut signals = proxy.receive_prepare_for_sleep().await?;
        while let Some(signal) = signals.next().await {
            if let Ok(args) = signal.args() {
                on_prepare_for_sleep(*args.start(), &refresh);
            }
        }
        zbus::Result::Ok(())
    }
    .await;
    if let Err(e) = result {
        eprintln!(
            "halod: aviso: sem o logind no D-Bus do sistema ({e}); o resume não será acompanhado"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testbus::add_hidraw;
    use std::fs;

    #[test]
    fn um_presente_e_outro_ausente() {
        let sys = tempfile::tempdir().unwrap();
        add_hidraw(sys.path(), "hidraw3", 0x1044, 0x7A4D, 0x0000);
        assert_eq!(
            scan_devices(sys.path()),
            [
                ("it5702".to_owned(), false, String::new()),
                ("waterforce".to_owned(), true, "/dev/hidraw3".to_owned()),
            ]
        );
    }

    #[test]
    fn devnode_muda() {
        let sys = tempfile::tempdir().unwrap();
        add_hidraw(sys.path(), "hidraw3", 0x1044, 0x7A4D, 0x0000);
        let before = scan_devices(sys.path());
        fs::remove_dir_all(sys.path().join("class/hidraw/hidraw3")).unwrap();
        add_hidraw(sys.path(), "hidraw5", 0x1044, 0x7A4D, 0x0000);
        let after = scan_devices(sys.path());
        assert_ne!(before, after);
        assert_eq!(
            after[1],
            ("waterforce".to_owned(), true, "/dev/hidraw5".to_owned())
        );
        assert_eq!(scan_devices(sys.path()), after);
    }

    fn reading(millicelsius: i32) -> telemetry::Telemetry {
        telemetry::Telemetry {
            fan_rpm: 1200,
            pump_rpm: 2600,
            fan_duty: 120,
            pump_duty: 200,
            coolant_millicelsius: millicelsius,
        }
    }

    #[test]
    fn liquido_fora_da_faixa() {
        let s = next_telemetry(TelemetryState::default(), Ok(reading(224_000)));
        assert!(s.available);
        assert_eq!(s.coolant_millicelsius, 224_000);
        assert!(!s.coolant_valid);
        assert!(next_telemetry(s, Ok(reading(26_000))).coolant_valid);
    }

    #[test]
    fn driver_nao_carregado_mantem_os_valores() {
        let s = next_telemetry(TelemetryState::default(), Ok(reading(26_000)));
        let gone = next_telemetry(s, Err(TelemetryError::DriverNotLoaded));
        assert!(!gone.available);
        assert_eq!(gone.fan_rpm, 1200);
        assert_eq!(gone.coolant_millicelsius, 26_000);
    }

    #[test]
    fn fim_da_suspensao_pede_atualizacao() {
        let (tx, rx) = async_channel::bounded(1);
        on_prepare_for_sleep(true, &tx);
        assert!(rx.try_recv().is_err());
        on_prepare_for_sleep(false, &tx);
        assert!(rx.try_recv().is_ok());
    }
}
