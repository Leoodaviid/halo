// SPDX-License-Identifier: GPL-2.0-or-later

//! Tarefas periódicas e resume. Nesta versão, os ticks só leem o sysfs: a
//! presença dos aparelhos e a telemetria do hwmon. Nenhum abre hidraw.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_channel::{Receiver, Sender};
use futures_util::StreamExt;
use futures_util::future::{Either, select};
use halo_core::hid::{DeviceMatch, find_devices};
use halo_core::waterforce::probe::model_name;
use halo_core::waterforce::telemetry::{self, Coolant, TelemetryError};
use halo_core::{g502, it5702, waterforce};
use zbus::Connection;

use crate::bus::{
    Device, Manager, ROOT_PATH, Telemetry, TelemetryState, WATERFORCE_PATH, devices_to_dbus,
};
use crate::worker::{KnownModel, Model};

/// O intervalo mínimo do status no driver do kernel.
pub const INTERVAL: Duration = Duration::from_secs(2);

/// Nome do Waterforce antes de o daemon conhecer o modelo.
pub const WATERFORCE_NAME: &str = "AORUS Waterforce X";
/// Nome da placa sem o `board_name` do DMI.
pub const BOARD_NAME: &str = "Placa-mãe (IT5702)";

/// De onde vem o nome de um aparelho, sem abrir o hidraw.
enum NameSource {
    /// O modelo que o worker leu no `DE 00`.
    WaterforceModel,
    /// `class/dmi/id/board_name` do sysfs.
    DmiBoard,
    Fixed(&'static str),
}

struct Known {
    id: &'static str,
    wanted: DeviceMatch,
    capabilities: &'static [&'static str],
    name: NameSource,
}

/// Os aparelhos conhecidos, na ordem em que o painel os mostra.
const DEVICES: [Known; 3] = [
    Known {
        id: "waterforce",
        wanted: waterforce::DEVICE,
        capabilities: &["led", "speed", "telemetry"],
        name: NameSource::WaterforceModel,
    },
    Known {
        id: "it5702",
        wanted: it5702::DEVICE,
        capabilities: &[],
        name: NameSource::DmiBoard,
    },
    Known {
        id: "g502",
        wanted: g502::DEVICE,
        capabilities: &[],
        name: NameSource::Fixed("Logitech G502 SE HERO"),
    },
];

/// `AORUS ` mais o nome do modelo, ou [`WATERFORCE_NAME`] sem modelo
/// conhecido.
pub fn waterforce_name(model: Option<Model>) -> String {
    match model.and_then(model_name) {
        Some(name) => format!("AORUS {name}"),
        None => WATERFORCE_NAME.to_owned(),
    }
}

/// O `board_name` do DMI sob `sys_root`, ou [`BOARD_NAME`].
pub fn board_name(sys_root: &Path) -> String {
    std::fs::read_to_string(sys_root.join("class/dmi/id/board_name"))
        .ok()
        .map(|s| s.trim_end().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| BOARD_NAME.to_owned())
}

/// Presença e nome de cada aparelho, só pelo sysfs e pelo modelo já lido.
pub fn scan_devices(sys_root: &Path, model: Option<Model>) -> Vec<Device> {
    let class = sys_root.join("class/hidraw");
    DEVICES
        .iter()
        .map(|known| {
            let devnode = find_devices(&class, known.wanted)
                .ok()
                .and_then(|found| found.into_iter().next())
                .map(|info| info.devnode.to_string_lossy().into_owned());
            Device {
                id: known.id,
                name: match known.name {
                    NameSource::WaterforceModel => waterforce_name(model),
                    NameSource::DmiBoard => board_name(sys_root),
                    NameSource::Fixed(name) => name.to_owned(),
                },
                present: devnode.is_some(),
                devnode: devnode.unwrap_or_default(),
                capabilities: known.capabilities,
            }
        })
        .collect()
}

/// Primeira leitura do sysfs, feita antes de o daemon pegar o nome: assim a
/// chamada que sobe o daemon pela ativação do D-Bus já recebe a lista e a
/// telemetria. Não abre nenhum hidraw.
pub fn first_read(sys_root: &Path) -> (Vec<Device>, TelemetryState) {
    (
        scan_devices(sys_root, None),
        next_telemetry(TelemetryState::default(), telemetry::read(sys_root)),
    )
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
    model: KnownModel,
}

impl Ticks {
    pub fn new(sys_root: PathBuf, model: KnownModel) -> Self {
        Self { sys_root, model }
    }

    /// Lê presença, nomes e telemetria e emite os sinais só do que mudou.
    pub async fn run_once(&self, conn: &Connection) -> zbus::Result<()> {
        let server = conn.object_server();

        let devices = scan_devices(&self.sys_root, self.model.get());
        let manager = server.interface::<_, Manager>(ROOT_PATH).await?;
        let changed = {
            let mut m = manager.get_mut().await;
            let changed = m.devices != devices;
            m.devices = devices.clone();
            changed
        };
        if changed {
            Manager::devices_changed(manager.signal_emitter(), devices_to_dbus(&devices)).await?;
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

    fn device(id: &str, devices: &[Device]) -> Device {
        devices.iter().find(|d| d.id == id).unwrap().clone()
    }

    #[test]
    fn waterforce_conectado_e_placa_desconectada() {
        let sys = tempfile::tempdir().unwrap();
        add_hidraw(sys.path(), "hidraw99", 0x1044, 0x7A4D, 0x0000);
        let devices = scan_devices(sys.path(), None);
        let ids: Vec<&str> = devices.iter().map(|d| d.id).collect();
        assert_eq!(ids, ["waterforce", "it5702", "g502"]);
        assert_eq!(
            devices[0],
            Device {
                id: "waterforce",
                name: "AORUS Waterforce X".into(),
                present: true,
                devnode: "/dev/hidraw99".into(),
                capabilities: &["led", "speed", "telemetry"],
            }
        );
        let board = device("it5702", &devices);
        assert!(!board.present);
        assert_eq!(board.devnode, "");
    }

    #[test]
    fn g502_detectado_so_pelo_sysfs() {
        // O sysfs falso não tem hidraw de verdade por trás: a busca só lê
        // `uevent` e `report_descriptor`.
        let sys = tempfile::tempdir().unwrap();
        add_hidraw(sys.path(), "hidraw96", 0x046D, 0xC08B, 0x0001);
        add_hidraw(sys.path(), "hidraw95", 0x046D, 0xC08B, 0xFF00);
        let mouse = device("g502", &scan_devices(sys.path(), None));
        assert_eq!(
            mouse,
            Device {
                id: "g502",
                name: "Logitech G502 SE HERO".into(),
                present: true,
                devnode: "/dev/hidraw95".into(),
                capabilities: &[],
            }
        );
    }

    #[test]
    fn placa_pelo_nome_dmi() {
        let sys = tempfile::tempdir().unwrap();
        add_hidraw(sys.path(), "hidraw97", 0x048D, 0x5702, 0xFF89);
        let board = device("it5702", &scan_devices(sys.path(), None));
        assert_eq!(board.name, "Placa-mãe (IT5702)");

        let dmi = sys.path().join("class/dmi/id");
        fs::create_dir_all(&dmi).unwrap();
        fs::write(dmi.join("board_name"), "B550M AORUS ELITE\n").unwrap();
        let board = device("it5702", &scan_devices(sys.path(), None));
        assert!(board.present);
        assert_eq!(board.name, "B550M AORUS ELITE");
        assert!(board.capabilities.is_empty());

        fs::write(dmi.join("board_name"), "\n").unwrap();
        assert_eq!(board_name(sys.path()), "Placa-mãe (IT5702)");
    }

    #[test]
    fn nome_do_waterforce_pelo_modelo() {
        assert_eq!(waterforce_name(None), "AORUS Waterforce X");
        assert_eq!(waterforce_name(Some(0)), "AORUS Waterforce X 240");
        assert_eq!(waterforce_name(Some(2)), "AORUS Waterforce X 360");
        assert_eq!(waterforce_name(Some(5)), "AORUS Waterforce X");
    }

    #[test]
    fn devnode_muda() {
        let sys = tempfile::tempdir().unwrap();
        add_hidraw(sys.path(), "hidraw99", 0x1044, 0x7A4D, 0x0000);
        let before = scan_devices(sys.path(), None);
        fs::remove_dir_all(sys.path().join("class/hidraw/hidraw99")).unwrap();
        add_hidraw(sys.path(), "hidraw98", 0x1044, 0x7A4D, 0x0000);
        let after = scan_devices(sys.path(), None);
        assert_ne!(before, after);
        assert_eq!(after[0].devnode, "/dev/hidraw98");
        assert_eq!(scan_devices(sys.path(), None), after);
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
