// SPDX-License-Identifier: GPL-2.0-or-later

//! Interfaces D-Bus do `halod`. Elas só validam as entradas com os tipos do
//! `halo-core` e põem o trabalho na fila do aparelho ([`Worker`]); nenhuma
//! tem lógica de protocolo própria.

use std::collections::HashMap;
use std::sync::Mutex;

use halo_core::waterforce::curve::Curve;
use halo_core::waterforce::led::{Effect, LedSetting};
use halo_core::waterforce::probe::{CurvePoint, SpeedMode};
use halo_core::waterforce::speed::{
    CurveRequest, FanPreset, PumpPreset, SetRequest, Side, SpeedError, Which,
};
use zbus::connection;
use zbus::interface;
use zbus::object_server::{ObjectServer, SignalEmitter};
use zbus::zvariant::{OwnedValue, Value};

use crate::error::Error;
use crate::profiles::{self, ProfileStore};
use crate::worker::Worker;

pub const BUS_NAME: &str = "io.github.leoodaviid.Halo1";
pub const ROOT_PATH: &str = "/io/github/leoodaviid/Halo1";
pub const WATERFORCE_PATH: &str = "/io/github/leoodaviid/Halo1/waterforce";

/// Publica todos os objetos do daemon em `builder`, com a lista de aparelhos
/// e a telemetria iniciais. O nome do bus é pedido depois, por quem chama.
pub fn serve(
    builder: connection::Builder<'static>,
    worker: Worker,
    profiles: ProfileStore,
    (devices, telemetry): (Vec<Device>, TelemetryState),
) -> zbus::Result<connection::Builder<'static>> {
    builder
        .serve_at(ROOT_PATH, Manager { devices })?
        .serve_at(ROOT_PATH, Profiles::new(profiles, worker.clone()))?
        .serve_at(WATERFORCE_PATH, Led::new(worker.clone()))?
        .serve_at(WATERFORCE_PATH, Speed::new(worker))?
        .serve_at(WATERFORCE_PATH, Telemetry { state: telemetry })
}

/// Pede [`BUS_NAME`] sem tomar o nome de outra instância e sem deixar outra
/// tomar o nosso (o padrão do zbus permite as duas coisas). Com o nome em uso,
/// o `build` falha com `NameTaken`.
pub fn claim_name(
    builder: connection::Builder<'static>,
) -> zbus::Result<connection::Builder<'static>> {
    Ok(builder
        .name(BUS_NAME)?
        .allow_name_replacements(false)
        .replace_existing_names(false))
}

/// Um aparelho da lista do `Manager`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// `waterforce`, `it5702` ou `g502`.
    pub id: &'static str,
    /// Nome para mostrar.
    pub name: String,
    /// Encontrado no sysfs.
    pub present: bool,
    /// `/dev/hidrawN`, ou vazio quando ausente.
    pub devnode: String,
    /// As interfaces D-Bus que o daemon expõe para o aparelho (`led`,
    /// `speed`, `telemetry`), presente ou não.
    pub capabilities: &'static [&'static str],
}

impl Device {
    /// `a{sv}` com as chaves fixas `id`, `name`, `present`, `devnode` e
    /// `capabilities`.
    fn to_dbus(&self) -> HashMap<String, OwnedValue> {
        HashMap::from([
            ("id".into(), owned(Value::from(self.id))),
            ("name".into(), owned(Value::from(self.name.as_str()))),
            ("present".into(), owned(Value::from(self.present))),
            ("devnode".into(), owned(Value::from(self.devnode.as_str()))),
            (
                "capabilities".into(),
                owned(Value::from(self.capabilities.to_vec())),
            ),
        ])
    }
}

/// A lista no formato `aa{sv}` do `Devices()` e do `DevicesChanged`.
pub fn devices_to_dbus(devices: &[Device]) -> Vec<HashMap<String, OwnedValue>> {
    devices.iter().map(Device::to_dbus).collect()
}

fn owned(value: Value<'_>) -> OwnedValue {
    value
        .try_to_owned()
        .expect("os valores do daemon não têm descritores")
}

// Manager

pub struct Manager {
    pub devices: Vec<Device>,
}

#[interface(name = "io.github.leoodaviid.Halo1.Manager")]
impl Manager {
    #[zbus(property)]
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_owned()
    }

    /// Todos os aparelhos conhecidos, presentes ou não, na ordem do painel.
    fn devices(&self) -> Vec<HashMap<String, OwnedValue>> {
        devices_to_dbus(&self.devices)
    }

    #[zbus(signal)]
    pub async fn devices_changed(
        emitter: &SignalEmitter<'_>,
        devices: Vec<HashMap<String, OwnedValue>>,
    ) -> zbus::Result<()>;
}

// Waterforce.Led

/// O último LED que o daemon aplicou com sucesso.
#[derive(Debug, Clone, PartialEq)]
struct LedRecord {
    effect: String,
    colors: Vec<String>,
    brightness: u8,
    speed: u8,
}

impl From<&LedSetting> for LedRecord {
    fn from(s: &LedSetting) -> Self {
        Self {
            effect: s.effect().name().to_owned(),
            colors: s.colors().iter().map(ToString::to_string).collect(),
            // O desligado vai com brilho 0.
            brightness: match s.effect() {
                Effect::Off => 0,
                _ => s.brightness().get(),
            },
            speed: s.speed().get(),
        }
    }
}

pub struct Led {
    worker: Worker,
    last: Mutex<Option<LedRecord>>,
}

impl Led {
    pub fn new(worker: Worker) -> Self {
        Self {
            worker,
            last: Mutex::new(None),
        }
    }

    fn record(&self, setting: &LedSetting) {
        *self.last.lock().unwrap() = Some(LedRecord::from(setting));
    }
}

/// Aplica pelo worker e, com sucesso, atualiza o `LastApplied` da interface
/// publicada em `server`.
async fn apply_led(
    server: &ObjectServer,
    worker: &Worker,
    setting: LedSetting,
) -> Result<(), Error> {
    let sent = setting.clone();
    worker.run(move |b| b.apply_led(&sent)).await?;
    let iface = server.interface::<_, Led>(WATERFORCE_PATH).await?;
    let led = iface.get().await;
    led.record(&setting);
    led.last_applied_changed(iface.signal_emitter()).await?;
    Ok(())
}

#[interface(name = "io.github.leoodaviid.Halo1.Waterforce.Led")]
impl Led {
    /// Brilho e velocidade 0 valem o padrão do efeito (o `off` exige brilho 0).
    async fn apply(
        &self,
        effect: &str,
        colors: Vec<String>,
        brightness: u8,
        speed: u8,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> Result<(), Error> {
        let given = |v: u8| (v != 0).then_some(v);
        let setting = profiles::led_setting(effect, &colors, given(brightness), given(speed))
            .map_err(Error::InvalidArgument)?;
        apply_led(server, &self.worker, setting).await
    }

    /// Vazio antes da primeira aplicação deste processo. Não é o estado do
    /// aparelho, que não tem leitura do LED.
    #[zbus(property)]
    fn last_applied(&self) -> HashMap<String, OwnedValue> {
        let mut map = HashMap::new();
        if let Some(r) = &*self.last.lock().unwrap() {
            map.insert("effect".into(), owned(Value::from(r.effect.as_str())));
            map.insert("colors".into(), owned(Value::from(r.colors.clone())));
            map.insert("brightness".into(), owned(Value::from(r.brightness)));
            map.insert("speed".into(), owned(Value::from(r.speed)));
        }
        map
    }
}

// Waterforce.Speed

pub struct Speed {
    worker: Worker,
}

impl Speed {
    pub fn new(worker: Worker) -> Self {
        Self { worker }
    }
}

fn mode_name(raw: u8) -> String {
    SpeedMode(raw)
        .name()
        .map(str::to_lowercase)
        .unwrap_or_default()
}

fn curve_value(curve: &[CurvePoint; 4]) -> Vec<(u8, u16)> {
    curve.iter().map(|p| (p.celsius, p.rpm)).collect()
}

fn side(name: &str) -> Result<Side, Error> {
    match name {
        "fan" => Ok(Side::Fan),
        "pump" => Ok(Side::Pump),
        _ => Err(Error::InvalidArgument(format!(
            "lado desconhecido `{name}`: use fan ou pump"
        ))),
    }
}

#[interface(name = "io.github.leoodaviid.Halo1.Waterforce.Speed")]
impl Speed {
    /// Só leitura: `DE 00`, `DD`, `D9 01`, `D9 02`. As curvas são o slot
    /// Customized guardado no aparelho.
    async fn status(&self) -> Result<HashMap<String, OwnedValue>, Error> {
        let status = self.worker.run(|b| b.speed_status()).await?;
        let s = status.state;
        let mut map = HashMap::new();
        map.insert(
            "devnode".into(),
            owned(Value::from(status.devnode.to_string_lossy().into_owned())),
        );
        map.insert("model".into(), owned(Value::from(s.model)));
        map.insert("fan_mode".into(), owned(Value::from(s.fan_mode)));
        map.insert("pump_mode".into(), owned(Value::from(s.pump_mode)));
        map.insert(
            "fan_mode_name".into(),
            owned(Value::from(mode_name(s.fan_mode))),
        );
        map.insert(
            "pump_mode_name".into(),
            owned(Value::from(mode_name(s.pump_mode))),
        );
        map.insert(
            "fan_curve".into(),
            owned(Value::from(curve_value(&s.fan_curve))),
        );
        map.insert(
            "pump_curve".into(),
            owned(Value::from(curve_value(&s.pump_curve))),
        );
        Ok(map)
    }

    /// `""` deixa o lado como está no aparelho. Sempre com `B6`.
    async fn set_presets(&self, fan: &str, pump: &str) -> Result<(), Error> {
        let fan = match fan {
            "" => None,
            name => Some(FanPreset::from_name(name).ok_or_else(|| {
                Error::InvalidArgument(format!("preset da fan desconhecido `{name}`"))
            })?),
        };
        let pump = match pump {
            "" => None,
            name => Some(PumpPreset::from_name(name).ok_or_else(|| {
                Error::InvalidArgument(format!("preset da bomba desconhecido `{name}`"))
            })?),
        };
        if fan.is_none() && pump.is_none() {
            return Err(SpeedError::NoSide.into());
        }
        let request = SetRequest {
            fan,
            pump,
            save: true,
        };
        self.worker.run(move |b| b.set_presets(&request)).await?;
        Ok(())
    }

    /// Curva Customized plana de um lado (`fan` ou `pump`). Sempre com `B6`.
    async fn set_flat_curve(&self, side_name: &str, rpm: u16) -> Result<(), Error> {
        let curve = Curve::flat(side(side_name)?, rpm)?;
        let request = CurveRequest { curve, save: true };
        self.worker.run(move |b| b.set_curve(&request)).await?;
        Ok(())
    }

    /// `previous` ou `original`.
    async fn restore(&self, which: &str) -> Result<(), Error> {
        let which = match which {
            "previous" => Which::Previous,
            "original" => Which::Original,
            _ => {
                return Err(Error::InvalidArgument(format!(
                    "snapshot desconhecido `{which}`: use previous ou original"
                )));
            }
        };
        self.worker.run(move |b| b.restore(which)).await?;
        Ok(())
    }
}

// Waterforce.Telemetry

/// Valores publicados. Sem hwmon, `available` fica falso e o resto mantém a
/// última leitura.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TelemetryState {
    pub available: bool,
    pub fan_rpm: u32,
    pub pump_rpm: u32,
    pub fan_duty: u8,
    pub pump_duty: u8,
    pub coolant_millicelsius: i32,
    pub coolant_valid: bool,
}

pub struct Telemetry {
    pub state: TelemetryState,
}

#[interface(name = "io.github.leoodaviid.Halo1.Waterforce.Telemetry")]
impl Telemetry {
    #[zbus(property)]
    fn available(&self) -> bool {
        self.state.available
    }

    #[zbus(property)]
    fn fan_rpm(&self) -> u32 {
        self.state.fan_rpm
    }

    #[zbus(property)]
    fn pump_rpm(&self) -> u32 {
        self.state.pump_rpm
    }

    #[zbus(property)]
    fn fan_duty(&self) -> u8 {
        self.state.fan_duty
    }

    #[zbus(property)]
    fn pump_duty(&self) -> u8 {
        self.state.pump_duty
    }

    #[zbus(property)]
    fn coolant_milli_celsius(&self) -> i32 {
        self.state.coolant_millicelsius
    }

    #[zbus(property)]
    fn coolant_valid(&self) -> bool {
        self.state.coolant_valid
    }
}

// Profiles

pub struct Profiles {
    store: ProfileStore,
    worker: Worker,
    active: Mutex<String>,
}

impl Profiles {
    pub fn new(store: ProfileStore, worker: Worker) -> Self {
        let active = store.active().ok().flatten().unwrap_or_default();
        Self {
            store,
            worker,
            active: Mutex::new(active),
        }
    }

    fn set_active(&self, name: Option<&str>) -> Result<(), Error> {
        self.store.set_active(name)?;
        *self.active.lock().unwrap() = name.unwrap_or_default().to_owned();
        Ok(())
    }
}

#[interface(name = "io.github.leoodaviid.Halo1.Profiles")]
impl Profiles {
    fn list(&self) -> Result<Vec<String>, Error> {
        self.store.list()
    }

    fn get(&self, name: &str) -> Result<HashMap<String, OwnedValue>, Error> {
        Ok(profiles::to_dbus(&self.store.load(name)?.file))
    }

    fn save(&self, name: &str, profile: HashMap<String, OwnedValue>) -> Result<(), Error> {
        self.store.save(name, profiles::from_dbus(&profile)?)?;
        Ok(())
    }

    async fn delete(
        &self,
        name: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), Error> {
        self.store.delete(name)?;
        let was_active = *self.active.lock().unwrap() == name;
        if was_active {
            self.set_active(None)?;
            self.active_profile_changed(&emitter).await?;
        }
        Ok(())
    }

    /// Velocidade antes do LED. Tenta todas as seções; só grava o ativo se
    /// todas derem certo. Devolve as seções aplicadas.
    async fn activate(
        &self,
        name: &str,
        #[zbus(object_server)] server: &ObjectServer,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<Vec<String>, Error> {
        let profile = self.store.load(name)?;
        let mut applied: Vec<String> = Vec::new();
        let mut failed: Vec<String> = Vec::new();
        let mut first: Option<Error> = None;
        let mut outcome = |section: &str, result: Result<(), Error>| match result {
            Ok(()) => applied.push(section.to_owned()),
            Err(e) => {
                failed.push(section.to_owned());
                first.get_or_insert(e);
            }
        };
        if let Some(desired) = profile.speed {
            let result = self.worker.run(move |b| b.apply_state(&desired)).await;
            outcome("waterforce.speed", result.map(|_| ()));
        }
        if let Some(setting) = profile.led {
            outcome(
                "waterforce.led",
                apply_led(server, &self.worker, setting).await,
            );
        }
        if let Some(e) = first {
            let list = |v: &[String]| match v {
                [] => "nenhuma".to_owned(),
                v => v.join(", "),
            };
            return Err(e.with_context(&format!(
                "aplicadas: {}; falharam: {}; o perfil ativo não mudou",
                list(&applied),
                list(&failed)
            )));
        }
        self.set_active(Some(name))?;
        self.active_profile_changed(&emitter).await?;
        Ok(applied)
    }

    /// Vazio quando nenhum perfil foi ativado.
    #[zbus(property)]
    fn active_profile(&self) -> String {
        self.active.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use halo_core::waterforce::led::Rgb;
    use halo_core::waterforce::speed::{DesiredState, FanSetting, PumpSetting};

    use crate::testbus::{Harness, add_hidraw, add_hwmon, changed_properties, next_signal};
    use crate::ticks::{self, Ticks};
    use crate::worker::fake::Call;

    const LED: &str = "io.github.leoodaviid.Halo1.Waterforce.Led";
    const SPEED: &str = "io.github.leoodaviid.Halo1.Waterforce.Speed";
    const TELEMETRY: &str = "io.github.leoodaviid.Halo1.Waterforce.Telemetry";
    const MANAGER: &str = "io.github.leoodaviid.Halo1.Manager";
    const PROFILES: &str = "io.github.leoodaviid.Halo1.Profiles";
    const PROPS: &str = "org.freedesktop.DBus.Properties";
    const SHORT: Duration = Duration::from_millis(300);

    fn err(name: &str) -> String {
        format!("io.github.leoodaviid.Halo1.Error.{name}")
    }

    fn apply(h: &Harness, effect: &str, colors: &[&str], b: u8, s: u8) -> Result<(), String> {
        let colors: Vec<String> = colors.iter().map(|c| (*c).to_owned()).collect();
        h.call(WATERFORCE_PATH, LED, "Apply", &(effect, colors, b, s))
    }

    fn tick(h: &Harness) {
        zbus::block_on(Ticks::new(h.sys.clone(), h.model.clone()).run_once(&h.server)).unwrap();
    }

    type DeviceDict = HashMap<String, OwnedValue>;

    /// `(id, name, present, devnode)` de cada aparelho.
    fn summary(devices: &[DeviceDict]) -> Vec<(String, String, bool, String)> {
        let text = |d: &DeviceDict, key: &str| <&str>::try_from(&d[key]).unwrap().to_owned();
        devices
            .iter()
            .map(|d| {
                (
                    text(d, "id"),
                    text(d, "name"),
                    bool::try_from(&d["present"]).unwrap(),
                    text(d, "devnode"),
                )
            })
            .collect()
    }

    fn listed(h: &Harness) -> Vec<DeviceDict> {
        h.call(ROOT_PATH, MANAGER, "Devices", &()).unwrap()
    }

    /// A lista do próximo `DevicesChanged`, ou `None`.
    fn next_devices(stream: &mut zbus::MessageStream) -> Option<Vec<DeviceDict>> {
        let msg = next_signal(stream, SHORT)?;
        let (devices,): (Vec<DeviceDict>,) = msg.body().deserialize().unwrap();
        Some(devices)
    }

    fn waterforce_name(devices: &[DeviceDict]) -> String {
        summary(devices)
            .into_iter()
            .find(|d| d.0 == "waterforce")
            .unwrap()
            .1
    }

    /// Nomes das propriedades de `iface` no próximo `PropertiesChanged`.
    fn next_change(stream: &mut zbus::MessageStream, iface: &str) -> Option<Vec<String>> {
        while let Some(msg) = next_signal(stream, SHORT) {
            let (name, _, _): (String, HashMap<String, OwnedValue>, Vec<String>) =
                msg.body().deserialize().unwrap();
            if name == iface {
                return Some(changed_properties(&msg));
            }
        }
        None
    }

    /// Todas as propriedades de `iface` que mudaram, somando os sinais.
    fn all_changes(stream: &mut zbus::MessageStream, iface: &str) -> Vec<String> {
        let mut names = Vec::new();
        while let Some(more) = next_change(stream, iface) {
            names.extend(more);
        }
        names.sort();
        names
    }

    fn profile_map(text: &str) -> HashMap<String, OwnedValue> {
        profiles::to_dbus(&crate::profiles::Profile::parse(text).unwrap().file)
    }

    const BOTH: &str = "[waterforce.led]\neffect = \"cycle\"\n\n[waterforce.speed]\nfan = \"quiet\"\npump = \"turbo\"\n";

    // 4.1

    #[test]
    fn lista_em_aa_sv() {
        let h = Harness::new();
        add_hidraw(&h.sys, "hidraw99", 0x1044, 0x7A4D, 0x0000);
        add_hidraw(&h.sys, "hidraw95", 0x046D, 0xC08B, 0xFF00);
        tick(&h);
        let devices = listed(&h);
        for d in &devices {
            let mut keys: Vec<&str> = d.keys().map(String::as_str).collect();
            keys.sort();
            assert_eq!(keys, ["capabilities", "devnode", "id", "name", "present"]);
            let sig = |key: &str| d[key].value_signature().to_string();
            assert_eq!(
                [sig("id"), sig("name"), sig("present"), sig("devnode")],
                ["s", "s", "b", "s"]
            );
            assert_eq!(sig("capabilities"), "as");
        }
        assert_eq!(
            summary(&devices),
            [
                (
                    "waterforce".into(),
                    "AORUS Waterforce X".into(),
                    true,
                    "/dev/hidraw99".into()
                ),
                (
                    "it5702".into(),
                    "Placa-mãe (IT5702)".into(),
                    false,
                    String::new()
                ),
                (
                    "g502".into(),
                    "Logitech G502 SE HERO".into(),
                    true,
                    "/dev/hidraw95".into()
                ),
            ]
        );
        let caps: Vec<String> = devices[0]["capabilities"]
            .try_clone()
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(caps, ["led", "speed", "telemetry"]);
        let caps: Vec<String> = devices[2]["capabilities"]
            .try_clone()
            .unwrap()
            .try_into()
            .unwrap();
        assert!(caps.is_empty());
    }

    #[test]
    fn estado_inicial_antes_do_primeiro_tick() {
        let h = Harness::with_sys(|sys| {
            add_hidraw(sys, "hidraw99", 0x1044, 0x7A4D, 0x0000);
            add_hwmon(sys, 1200, 26_000);
        });
        // Nenhum tick rodou: a lista e a telemetria vêm da primeira leitura.
        assert_eq!(
            summary(&listed(&h))[0],
            (
                "waterforce".into(),
                "AORUS Waterforce X".into(),
                true,
                "/dev/hidraw99".into()
            )
        );
        let available: bool = h.property(WATERFORCE_PATH, TELEMETRY, "Available");
        assert!(available);
        let fan: u32 = h.property(WATERFORCE_PATH, TELEMETRY, "FanRpm");
        assert_eq!(fan, 1200);

        // O primeiro tick lê o mesmo: nenhum sinal.
        let mut devices = h.signals(MANAGER, "DevicesChanged");
        let mut props = h.signals(PROPS, "PropertiesChanged");
        tick(&h);
        assert!(next_devices(&mut devices).is_none());
        assert!(all_changes(&mut props, TELEMETRY).is_empty());
        assert!(h.backend.calls().is_empty());
    }

    #[test]
    fn sinal_de_aparelhos() {
        let h = Harness::new();
        add_hidraw(&h.sys, "hidraw99", 0x1044, 0x7A4D, 0x0000);
        let mut stream = h.signals(MANAGER, "DevicesChanged");
        tick(&h);
        let devices = next_devices(&mut stream).expect("DevicesChanged");
        assert_eq!(summary(&devices)[0].3, "/dev/hidraw99");
        assert_eq!(summary(&listed(&h)), summary(&devices));

        std::fs::remove_dir_all(h.sys.join("class/hidraw/hidraw99")).unwrap();
        add_hidraw(&h.sys, "hidraw98", 0x1044, 0x7A4D, 0x0000);
        tick(&h);
        let devices = next_devices(&mut stream).expect("DevicesChanged");
        assert_eq!(
            summary(&devices)[0],
            (
                "waterforce".into(),
                "AORUS Waterforce X".into(),
                true,
                "/dev/hidraw98".into()
            )
        );

        // Nada mudou: dois ciclos iguais, nenhum sinal.
        tick(&h);
        tick(&h);
        assert!(next_devices(&mut stream).is_none());
    }

    #[test]
    fn daemon_recem_iniciado() {
        let h = Harness::new();
        add_hidraw(&h.sys, "hidraw99", 0x1044, 0x7A4D, 0x0000);
        tick(&h);
        assert_eq!(waterforce_name(&listed(&h)), "AORUS Waterforce X");
        assert!(h.backend.calls().is_empty());
        assert!(h.wake.try_recv().is_err());
    }

    #[test]
    fn nome_muda_depois_da_primeira_operacao() {
        let h = Harness::new();
        add_hidraw(&h.sys, "hidraw99", 0x1044, 0x7A4D, 0x0000);
        tick(&h);
        let mut stream = h.signals(MANAGER, "DevicesChanged");
        apply(&h, "cycle", &[], 0, 0).unwrap();
        // O worker pede uma volta dos ticks, que monta o nome novo.
        assert!(h.wake.try_recv().is_ok());
        tick(&h);
        let devices = next_devices(&mut stream).expect("DevicesChanged");
        assert_eq!(waterforce_name(&devices), "AORUS Waterforce X 240");

        // O mesmo modelo de novo: nem pedido de volta nem sinal.
        apply(&h, "cycle", &[], 0, 0).unwrap();
        assert!(h.wake.try_recv().is_err());
        tick(&h);
        assert!(next_devices(&mut stream).is_none());
    }

    #[test]
    fn leitura_de_velocidade_revela_o_modelo() {
        let h = Harness::new();
        add_hidraw(&h.sys, "hidraw99", 0x1044, 0x7A4D, 0x0000);
        tick(&h);
        let mut stream = h.signals(MANAGER, "DevicesChanged");
        h.backend.set_model(2);
        let _: HashMap<String, OwnedValue> = h.call(WATERFORCE_PATH, SPEED, "Status", &()).unwrap();
        assert!(h.wake.try_recv().is_ok());
        tick(&h);
        let devices = next_devices(&mut stream).expect("DevicesChanged");
        assert_eq!(waterforce_name(&devices), "AORUS Waterforce X 360");
    }

    #[test]
    fn modelo_desconhecido() {
        let h = Harness::new();
        add_hidraw(&h.sys, "hidraw99", 0x1044, 0x7A4D, 0x0000);
        tick(&h);
        let mut stream = h.signals(MANAGER, "DevicesChanged");
        // No backend real, o modelo fora da tabela faz a operação falhar.
        h.backend
            .fail_next([Some(Error::Device("modelo desconhecido: 5".into()))]);
        assert_eq!(apply(&h, "cycle", &[], 0, 0), Err(err("Device")));
        // E, mesmo se uma operação devolvesse o modelo 5, o nome não muda.
        h.backend.set_model(5);
        let _: HashMap<String, OwnedValue> = h.call(WATERFORCE_PATH, SPEED, "Status", &()).unwrap();
        assert!(h.wake.try_recv().is_err());
        tick(&h);
        assert!(next_devices(&mut stream).is_none());
        assert_eq!(waterforce_name(&listed(&h)), "AORUS Waterforce X");
    }

    // 4.3

    #[test]
    fn ticks_e_resume_nao_chamam_o_backend() {
        let h = Harness::new();
        add_hidraw(&h.sys, "hidraw99", 0x1044, 0x7A4D, 0x0000);
        add_hidraw(&h.sys, "hidraw97", 0x048D, 0x5702, 0xFF89);
        add_hwmon(&h.sys, 1200, 26_000);
        for _ in 0..3 {
            tick(&h);
        }
        let (tx, rx) = async_channel::bounded(1);
        ticks::on_prepare_for_sleep(false, &tx);
        assert!(rx.try_recv().is_ok());
        tick(&h);
        assert!(h.backend.calls().is_empty());
        let present: Vec<(String, bool)> = summary(&listed(&h))
            .into_iter()
            .map(|(id, _, present, _)| (id, present))
            .collect();
        assert_eq!(
            present,
            [
                ("waterforce".into(), true),
                ("it5702".into(), true),
                ("g502".into(), false)
            ]
        );
    }

    // 5.1

    #[test]
    fn devices_responde_com_o_backend_ocupado() {
        let h = Harness::new();
        h.backend.hold(true);
        let client = h.client.clone();
        let busy = std::thread::spawn(move || {
            zbus::block_on(client.call_method(
                Some(BUS_NAME),
                WATERFORCE_PATH,
                Some(LED),
                "Apply",
                &("cycle", Vec::<String>::new(), 0u8, 0u8),
            ))
            .map(|_| ())
        });
        std::thread::sleep(Duration::from_millis(100));
        let started = std::time::Instant::now();
        let _ = listed(&h);
        let _: Vec<String> = h.call(ROOT_PATH, PROFILES, "List", &()).unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(h.backend.calls().is_empty());
        h.backend.hold(false);
        busy.join().unwrap().unwrap();
        assert_eq!(h.backend.calls().len(), 1);
    }

    // 5.2

    #[test]
    fn efeito_com_paleta() {
        let h = Harness::new();
        apply(&h, "tri-color", &["ff0000", "00ff00", "0000ff"], 10, 5).unwrap();
        let expected = LedSetting::new(
            Effect::TriColor,
            &[Rgb(0xFF, 0, 0), Rgb(0, 0xFF, 0), Rgb(0, 0, 0xFF)],
            None,
            None,
        )
        .unwrap();
        assert_eq!(h.backend.calls(), [Call::Led(expected)]);
    }

    #[test]
    fn efeito_desconhecido_e_brilho_invalido() {
        let h = Harness::new();
        assert_eq!(apply(&h, "strobe", &[], 0, 0), Err(err("InvalidArgument")));
        assert_eq!(apply(&h, "static", &[], 11, 0), Err(err("InvalidArgument")));
        assert_eq!(apply(&h, "off", &[], 5, 0), Err(err("InvalidArgument")));
        assert!(h.backend.calls().is_empty());
    }

    #[test]
    fn desligar_com_padroes() {
        let h = Harness::new();
        apply(&h, "off", &[], 0, 0).unwrap();
        let expected = LedSetting::new(Effect::Off, &[], None, None).unwrap();
        assert_eq!(h.backend.calls(), [Call::Led(expected)]);
        let last: HashMap<String, OwnedValue> = h.property(WATERFORCE_PATH, LED, "LastApplied");
        assert_eq!(u8::try_from(&last["brightness"]).unwrap(), 0);
    }

    #[test]
    fn ultimo_led_aplicado() {
        let h = Harness::new();
        let last: HashMap<String, OwnedValue> = h.property(WATERFORCE_PATH, LED, "LastApplied");
        assert!(last.is_empty());

        let mut stream = h.signals(PROPS, "PropertiesChanged");
        apply(&h, "cycle", &[], 0, 3).unwrap();
        assert_eq!(
            next_change(&mut stream, LED),
            Some(vec!["LastApplied".into()])
        );
        let last: HashMap<String, OwnedValue> = h.property(WATERFORCE_PATH, LED, "LastApplied");
        assert_eq!(<&str>::try_from(&last["effect"]).unwrap(), "cycle");
        assert_eq!(u8::try_from(&last["brightness"]).unwrap(), 10);
        assert_eq!(u8::try_from(&last["speed"]).unwrap(), 3);

        // Uma aplicação que falha não muda a propriedade.
        h.backend
            .fail_next([Some(Error::Device("modelo desconhecido".into()))]);
        assert_eq!(apply(&h, "static", &["00ffff"], 5, 0), Err(err("Device")));
        let last: HashMap<String, OwnedValue> = h.property(WATERFORCE_PATH, LED, "LastApplied");
        assert_eq!(<&str>::try_from(&last["effect"]).unwrap(), "cycle");
    }

    // 5.3

    #[test]
    fn status_lido() {
        let h = Harness::new();
        let s: HashMap<String, OwnedValue> = h.call(WATERFORCE_PATH, SPEED, "Status", &()).unwrap();
        assert_eq!(u8::try_from(&s["fan_mode"]).unwrap(), 5);
        assert_eq!(
            <&str>::try_from(&s["fan_mode_name"]).unwrap(),
            "performance"
        );
        assert_eq!(u8::try_from(&s["pump_mode"]).unwrap(), 4);
        assert_eq!(<&str>::try_from(&s["pump_mode_name"]).unwrap(), "turbo");
        let curve: Vec<(u8, u16)> = s["fan_curve"].try_clone().unwrap().try_into().unwrap();
        assert_eq!(curve, [(0, 1000), (30, 1000), (50, 1500), (80, 2500)]);
        assert_eq!(h.backend.calls(), [Call::Status]);
    }

    #[test]
    fn presets_e_curva() {
        let h = Harness::new();
        let none: Result<(), String> = h.call(WATERFORCE_PATH, SPEED, "SetPresets", &("", ""));
        assert_eq!(none, Err(err("InvalidArgument")));
        let bad: Result<(), String> = h.call(WATERFORCE_PATH, SPEED, "SetPresets", &("", "quiet"));
        assert_eq!(bad, Err(err("InvalidArgument")));
        let range: Result<(), String> =
            h.call(WATERFORCE_PATH, SPEED, "SetFlatCurve", &("pump", 2000u16));
        assert_eq!(range, Err(err("InvalidArgument")));
        assert!(h.backend.calls().is_empty());

        let () = h
            .call(WATERFORCE_PATH, SPEED, "SetPresets", &("quiet", ""))
            .unwrap();
        let () = h
            .call(WATERFORCE_PATH, SPEED, "SetFlatCurve", &("fan", 1500u16))
            .unwrap();
        let () = h
            .call(WATERFORCE_PATH, SPEED, "Restore", &("original",))
            .unwrap();
        assert_eq!(
            h.backend.calls(),
            [
                Call::Presets(SetRequest {
                    fan: Some(FanPreset::Quiet),
                    pump: None,
                    save: true
                }),
                Call::Curve(CurveRequest {
                    curve: Curve::flat(Side::Fan, 1500).unwrap(),
                    save: true
                }),
                Call::Restore(Which::Original),
            ]
        );
    }

    // 5.4

    #[test]
    fn telemetria() {
        let h = Harness::new();
        add_hwmon(&h.sys, 1200, 224_000);
        let mut stream = h.signals(PROPS, "PropertiesChanged");
        tick(&h);
        assert_eq!(
            all_changes(&mut stream, TELEMETRY),
            [
                "Available",
                "CoolantMilliCelsius",
                "FanDuty",
                "FanRpm",
                "PumpDuty",
                "PumpRpm"
            ]
        );
        let get = |name| h.property::<OwnedValue>(WATERFORCE_PATH, TELEMETRY, name);
        assert!(bool::try_from(get("Available")).unwrap());
        assert_eq!(i32::try_from(get("CoolantMilliCelsius")).unwrap(), 224_000);
        assert!(!bool::try_from(get("CoolantValid")).unwrap());

        // Valores iguais: nenhum sinal.
        tick(&h);
        assert!(all_changes(&mut stream, TELEMETRY).is_empty());

        // Sem hwmon: indisponível, com os últimos valores.
        std::fs::remove_dir_all(h.sys.join("class/hwmon")).unwrap();
        tick(&h);
        assert_eq!(
            next_change(&mut stream, TELEMETRY),
            Some(vec!["Available".into()])
        );
        assert!(!bool::try_from(get("Available")).unwrap());
        assert_eq!(u32::try_from(get("FanRpm")).unwrap(), 1200);
        assert!(h.backend.calls().is_empty());
    }

    // 6.2

    #[test]
    fn salvar_e_ler() {
        let h = Harness::new();
        let map = profile_map(BOTH);
        let () = h
            .call(ROOT_PATH, PROFILES, "Save", &("noite", &map))
            .unwrap();
        let got: HashMap<String, OwnedValue> =
            h.call(ROOT_PATH, PROFILES, "Get", &("noite",)).unwrap();
        assert_eq!(
            profiles::from_dbus(&got).unwrap(),
            profiles::from_dbus(&map).unwrap()
        );
        let list: Vec<String> = h.call(ROOT_PATH, PROFILES, "List", &()).unwrap();
        assert_eq!(list, ["noite"]);
        let missing: Result<HashMap<String, OwnedValue>, String> =
            h.call(ROOT_PATH, PROFILES, "Get", &("nada",));
        assert_eq!(missing.unwrap_err(), err("ProfileNotFound"));
        let bad: Result<(), String> = h.call(
            ROOT_PATH,
            PROFILES,
            "Save",
            &("noite", profile_map("[waterforce.speed]\nfan = 1500\n")),
        );
        assert!(bad.is_ok());
        let bad: Result<(), String> = h.call(ROOT_PATH, PROFILES, "Save", &("../x", &map));
        assert_eq!(bad, Err(err("InvalidArgument")));
    }

    #[test]
    fn ativar_tudo_certo_e_apagar() {
        let h = Harness::new();
        let () = h
            .call(ROOT_PATH, PROFILES, "Save", &("noite", profile_map(BOTH)))
            .unwrap();
        let mut stream = h.signals(PROPS, "PropertiesChanged");
        let applied: Vec<String> = h
            .call(ROOT_PATH, PROFILES, "Activate", &("noite",))
            .unwrap();
        assert_eq!(applied, ["waterforce.speed", "waterforce.led"]);
        let desired = DesiredState::new(
            Some(FanSetting::Preset(FanPreset::Quiet)),
            Some(PumpSetting::Preset(PumpPreset::Turbo)),
        )
        .unwrap();
        let cycle = LedSetting::new(Effect::Cycle, &[], None, None).unwrap();
        assert_eq!(h.backend.calls(), [Call::State(desired), Call::Led(cycle)]);
        assert_eq!(h.profiles().active().unwrap().as_deref(), Some("noite"));
        assert_eq!(
            next_change(&mut stream, PROFILES),
            Some(vec!["ActiveProfile".into()])
        );
        let active: String = h.property(ROOT_PATH, PROFILES, "ActiveProfile");
        assert_eq!(active, "noite");

        let () = h.call(ROOT_PATH, PROFILES, "Delete", &("noite",)).unwrap();
        assert_eq!(
            next_change(&mut stream, PROFILES),
            Some(vec!["ActiveProfile".into()])
        );
        let active: String = h.property(ROOT_PATH, PROFILES, "ActiveProfile");
        assert_eq!(active, "");
        assert_eq!(h.profiles().active().unwrap(), None);
        let gone: Result<(), String> = h.call(ROOT_PATH, PROFILES, "Delete", &("noite",));
        assert_eq!(gone, Err(err("ProfileNotFound")));
    }

    #[test]
    fn led_falha_e_velocidade_da_certo() {
        let h = Harness::new();
        let () = h
            .call(ROOT_PATH, PROFILES, "Save", &("noite", profile_map(BOTH)))
            .unwrap();
        h.backend
            .fail_next([None, Some(Error::Device("modelo desconhecido".into()))]);
        let result: Result<Vec<String>, zbus::Error> = zbus::block_on(async {
            let reply = h
                .client
                .call_method(
                    Some(BUS_NAME),
                    ROOT_PATH,
                    Some(PROFILES),
                    "Activate",
                    &("noite",),
                )
                .await?;
            reply.body().deserialize::<Vec<String>>()
        });
        let Err(zbus::Error::MethodError(name, Some(msg), _)) = result else {
            panic!("{result:?}")
        };
        assert_eq!(name.as_str(), err("Device"));
        assert!(
            msg.contains("aplicadas: waterforce.speed; falharam: waterforce.led"),
            "{msg}"
        );
        assert_eq!(h.backend.calls().len(), 2);
        assert_eq!(h.profiles().active().unwrap(), None);
    }

    #[test]
    fn perfil_com_led_e_arquivo_invalido() {
        let h = Harness::new();
        let store = h.profiles();
        let () = h
            .call(
                ROOT_PATH,
                PROFILES,
                "Save",
                &("led", profile_map("[waterforce.led]\neffect = \"cycle\"\n")),
            )
            .unwrap();
        let applied: Vec<String> = h.call(ROOT_PATH, PROFILES, "Activate", &("led",)).unwrap();
        assert_eq!(applied, ["waterforce.led"]);
        let last: HashMap<String, OwnedValue> = h.property(WATERFORCE_PATH, LED, "LastApplied");
        assert_eq!(<&str>::try_from(&last["effect"]).unwrap(), "cycle");

        std::fs::write(
            store.path("mao"),
            "[waterforce.led]\neffect = \"static\"\nbrightness = 15\n",
        )
        .unwrap();
        let bad: Result<Vec<String>, String> = h.call(ROOT_PATH, PROFILES, "Activate", &("mao",));
        assert_eq!(bad, Err(err("InvalidArgument")));
        assert_eq!(h.backend.calls().len(), 1);
    }
}
