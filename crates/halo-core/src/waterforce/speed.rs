// SPDX-License-Identifier: GPL-2.0-or-later

//! Modo de velocidade da fan e da bomba do Castor3, com os presets do firmware.
//!
//! Só estes pacotes existem aqui, todos do `ucCooler.dll` do GCC 26.09.10.01
//! (`docs/protocols/castor3.md`, "Velocidade"):
//! - leitura: `99 DE 00`, `99 DD`, `99 D9 01` e `99 D9 02` (`DeviceApi`
//!   `GetDeviceModelName`, `GetMode` e `GetSpeedCurve`);
//! - `99 E5 <tipo> <modo> 00`: `DeviceApi.SetSpeedMode`;
//! - `99 E6 00 <tipo>` e 4 pontos `<°C> <rpm_hi> <rpm_lo>`:
//!   `DeviceApi.SetSpeedCurve`, só para restaurar um lado salvo em Customized;
//! - `99 B6`: `DeviceApi.Save`.
//!
//! A ordem `E5` fan, `E5` bomba, `E6` fan, `E6` bomba e `B6` é a do
//! `FanControl.OnClickApply`. Antes de qualquer escrita, o estado lido é
//! gravado como snapshot em `$XDG_STATE_HOME/halo/`. Sem snapshot gravado,
//! nada é enviado.

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::led::{OutputReports, PACKET_INTERVAL};
use super::probe::{
    CurvePoint, InputReports, RESPONSE_TIMEOUT, ReadQuery, Reading, SpeedMode, model_name, packet,
    wait_response,
};
use super::{PRODUCT_ID, UDEV_RULE, VENDOR_ID, find_hidraw};
use crate::hid::{Hidraw, OpenError};

/// `SpeedType` do GCC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Fan,
    Pump,
}

impl Side {
    pub fn wire(self) -> u8 {
        match self {
            Self::Fan => 0x01,
            Self::Pump => 0x02,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Fan => "fan",
            Self::Pump => "bomba",
        }
    }
}

/// Valores do `SpeedMode` do GCC usados aqui.
pub mod mode {
    pub const BALANCED: u8 = 0;
    pub const CUSTOMIZED: u8 = 1;
    pub const DEFAULT: u8 = 2;
    pub const FIXED_RPM: u8 = 3;
    pub const TURBO: u8 = 4;
    pub const PERFORMANCE: u8 = 5;
    pub const QUIET: u8 = 6;
    pub const ZERO_RPM: u8 = 7;
}

/// Modo que o halo aceita repetir ou restaurar: 0 a 7, menos o 3 (FixedRPM),
/// que o GCC lê como Balanced e nunca envia.
pub fn restorable_mode(value: u8) -> bool {
    value <= mode::ZERO_RPM && value != mode::FIXED_RPM
}

/// Presets da fan que o halo aplica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanPreset {
    Default,
    Quiet,
    Balanced,
    Performance,
    Turbo,
}

impl FanPreset {
    pub const ALL: [Self; 5] = [
        Self::Default,
        Self::Quiet,
        Self::Balanced,
        Self::Performance,
        Self::Turbo,
    ];

    pub const NAMES: [&'static str; 5] = ["default", "quiet", "balanced", "performance", "turbo"];

    pub fn name(self) -> &'static str {
        let i = Self::ALL.iter().position(|&p| p == self).unwrap();
        Self::NAMES[i]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let i = Self::NAMES.iter().position(|&n| n == name)?;
        Some(Self::ALL[i])
    }

    pub fn wire(self) -> u8 {
        match self {
            Self::Default => mode::DEFAULT,
            Self::Quiet => mode::QUIET,
            Self::Balanced => mode::BALANCED,
            Self::Performance => mode::PERFORMANCE,
            Self::Turbo => mode::TURBO,
        }
    }

    /// `None` para ZeroRPM, Customized, FixedRPM e valores desconhecidos.
    pub fn from_wire(value: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.wire() == value)
    }
}

impl fmt::Display for FanPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Presets da bomba que o halo aplica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PumpPreset {
    Balanced,
    Turbo,
}

impl PumpPreset {
    pub const ALL: [Self; 2] = [Self::Balanced, Self::Turbo];

    pub const NAMES: [&'static str; 2] = ["balanced", "turbo"];

    pub fn name(self) -> &'static str {
        let i = Self::ALL.iter().position(|&p| p == self).unwrap();
        Self::NAMES[i]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let i = Self::NAMES.iter().position(|&n| n == name)?;
        Some(Self::ALL[i])
    }

    pub fn wire(self) -> u8 {
        match self {
            Self::Balanced => mode::BALANCED,
            Self::Turbo => mode::TURBO,
        }
    }

    /// `None` para Quiet, Customized e os outros modos.
    pub fn from_wire(value: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.wire() == value)
    }
}

impl fmt::Display for PumpPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// `99 E5 <tipo> <modo> 00`.
pub fn set_mode_payload(side: Side, mode: u8) -> Vec<u8> {
    vec![0x99, 0xE5, side.wire(), mode, 0x00]
}

/// `99 E6 00 <tipo>` e os 4 pontos, RPM em big-endian.
pub fn set_curve_payload(side: Side, curve: &[CurvePoint; 4]) -> Vec<u8> {
    let mut p = vec![0x99, 0xE6, 0x00, side.wire()];
    for point in curve {
        let [hi, lo] = point.rpm.to_be_bytes();
        p.extend([point.celsius, hi, lo]);
    }
    p
}

/// `99 B6`.
pub const SAVE_PAYLOAD: [u8; 2] = [0x99, 0xB6];

/// Estado a escrever.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub fan: u8,
    pub pump: u8,
    /// Curva a reenviar quando a fan vai para Customized.
    pub fan_curve: Option<[CurvePoint; 4]>,
    pub pump_curve: Option<[CurvePoint; 4]>,
}

impl Target {
    /// Sequência na ordem do `OnClickApply`, cada pacote com o report ID
    /// `0x99` e sem o preenchimento. Com `save`, termina com um único `B6`.
    pub fn packets(&self, save: bool) -> Vec<Vec<u8>> {
        let mut out = vec![
            set_mode_payload(Side::Fan, self.fan),
            set_mode_payload(Side::Pump, self.pump),
        ];
        if let Some(curve) = &self.fan_curve {
            out.push(set_curve_payload(Side::Fan, curve));
        }
        if let Some(curve) = &self.pump_curve {
            out.push(set_curve_payload(Side::Pump, curve));
        }
        if save {
            out.push(SAVE_PAYLOAD.to_vec());
        }
        out
    }
}

/// Modos de um `set`: o lado não informado repete o valor bruto lido no `DD`.
pub fn resolve_modes(
    fan: Option<FanPreset>,
    pump: Option<PumpPreset>,
    current_fan: SpeedMode,
    current_pump: SpeedMode,
) -> Result<Target, SpeedError> {
    if fan.is_none() && pump.is_none() {
        return Err(SpeedError::NoSide);
    }
    let keep = |side, current: SpeedMode| {
        if restorable_mode(current.0) {
            Ok(current.0)
        } else {
            Err(SpeedError::CurrentModeUnknown {
                side,
                raw: current.0,
            })
        }
    };
    let fan = match fan {
        Some(p) => p.wire(),
        None => keep(Side::Fan, current_fan)?,
    };
    let pump = match pump {
        Some(p) => p.wire(),
        None => keep(Side::Pump, current_pump)?,
    };
    Ok(Target {
        fan,
        pump,
        fan_curve: None,
        pump_curve: None,
    })
}

/// Temperaturas dos pontos das tabelas do GCC (`DeviceApi.GetSpeedCurve`).
pub const REFERENCE_CELSIUS: [u8; 4] = [0, 30, 50, 65];

/// `FanApi.dcX240`.
const FAN_X240: [(u8, [u16; 4]); 7] = [
    (0, [1500, 1500, 1800, 1800]),
    (1, [2250, 2250, 2250, 2250]),
    (2, [1200, 1200, 1400, 1800]),
    (4, [2500, 2500, 2500, 2500]),
    (5, [2000, 2000, 2200, 2500]),
    (6, [900, 900, 1400, 1600]),
    (7, [0, 0, 0, 0]),
];

/// `FanApi.dcX280_360`.
const FAN_X280_360: [(u8, [u16; 4]); 7] = [
    (0, [1500, 1500, 1800, 1800]),
    (1, [2250, 2250, 2250, 2250]),
    (2, [1200, 1200, 1400, 1800]),
    (4, [2500, 2500, 2500, 2500]),
    (5, [1800, 1800, 1800, 2500]),
    (6, [900, 900, 1400, 1600]),
    (7, [0, 0, 0, 0]),
];

/// `PumpApi.dcX240_280_360`.
const PUMP_X: [(u8, [u16; 4]); 3] = [
    (0, [2600, 2600, 2600, 3150]),
    (1, [2800, 2800, 2800, 2800]),
    (4, [3150, 3150, 3150, 3150]),
];

/// Curva que a interface do GCC mostra para um preset, pelo modelo do `DE`
/// (o GCC escolhe a tabela do 240 pelo nome do modelo). Não é lida do
/// aparelho. `None` para Customized, FixedRPM, modelos e modos sem tabela.
pub fn reference_curve(model: u8, side: Side, mode: SpeedMode) -> Option<[CurvePoint; 4]> {
    if mode.0 == mode::CUSTOMIZED {
        return None;
    }
    let table: &[(u8, [u16; 4])] = match (side, model) {
        (Side::Fan, 0) => &FAN_X240,
        (Side::Fan, 1 | 2) => &FAN_X280_360,
        (Side::Pump, 0..=2) => &PUMP_X,
        _ => return None,
    };
    let (_, rpm) = table.iter().find(|(m, _)| *m == mode.0)?;
    Some(std::array::from_fn(|i| CurvePoint {
        celsius: REFERENCE_CELSIUS[i],
        rpm: rpm[i],
    }))
}

/// `DeviceApi.MaxSpeed`.
pub const MAX_RPM: u16 = 3200;
pub const MAX_CELSIUS: u8 = 100;

/// Checagem de sanidade de uma curva a reenviar.
pub fn check_curve(curve: &[CurvePoint; 4]) -> Result<(), String> {
    for (i, p) in curve.iter().enumerate() {
        if p.celsius > MAX_CELSIUS {
            return Err(format!("{} °C acima de {MAX_CELSIUS} °C", p.celsius));
        }
        if p.rpm > MAX_RPM {
            return Err(format!("{} RPM acima de {MAX_RPM} RPM", p.rpm));
        }
        if i > 0 && p.celsius <= curve[i - 1].celsius {
            return Err("temperaturas não estritamente crescentes".to_owned());
        }
    }
    Ok(())
}

/// Estado de velocidade lido do aparelho, como fica no snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub taken_unix: u64,
    /// `r[2]` do `DE`.
    pub model: u8,
    pub fan_mode: u8,
    pub pump_mode: u8,
    /// Slot Customized (`D9 01`).
    pub fan_curve: [CurvePoint; 4],
    /// Slot Customized (`D9 02`).
    pub pump_curve: [CurvePoint; 4],
}

pub const SNAPSHOT_VERSION: u32 = 1;

const KEYS: [&str; 7] = [
    "version",
    "taken_unix",
    "model",
    "fan_mode",
    "pump_mode",
    "fan_curve",
    "pump_curve",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotError {
    /// Linha sem `=`.
    BadLine(usize),
    UnknownKey(String),
    DuplicateKey(String),
    MissingKey(&'static str),
    BadValue {
        key: &'static str,
        value: String,
    },
    UnsupportedVersion(u32),
    BadModel(u8),
    BadMode {
        key: &'static str,
        value: u8,
    },
    BadCurve {
        key: &'static str,
        reason: String,
    },
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadLine(n) => write!(f, "linha {n} sem `chave=valor`"),
            Self::UnknownKey(k) => write!(f, "chave desconhecida `{k}`"),
            Self::DuplicateKey(k) => write!(f, "chave repetida `{k}`"),
            Self::MissingKey(k) => write!(f, "falta a chave `{k}`"),
            Self::BadValue { key, value } => write!(f, "valor inválido em `{key}`: `{value}`"),
            Self::UnsupportedVersion(v) => write!(f, "versão {v} não suportada"),
            Self::BadModel(v) => write!(f, "modelo {v} desconhecido"),
            Self::BadMode { key, value } => {
                write!(f, "`{key}` = {value}: o halo não restaura esse modo")
            }
            Self::BadCurve { key, reason } => write!(f, "curva `{key}` recusada: {reason}"),
        }
    }
}

impl std::error::Error for SnapshotError {}

impl Snapshot {
    /// Texto `chave=valor`, versão 1.
    pub fn serialize(&self) -> String {
        format!(
            "# halo: snapshot de velocidade do Waterforce\n\
             version={SNAPSHOT_VERSION}\n\
             taken_unix={}\n\
             model={}\n\
             fan_mode={}\n\
             pump_mode={}\n\
             fan_curve={}\n\
             pump_curve={}\n",
            self.taken_unix,
            self.model,
            self.fan_mode,
            self.pump_mode,
            format_curve(&self.fan_curve),
            format_curve(&self.pump_curve),
        )
    }

    /// Parser estrito: chave desconhecida, repetida ou faltando, valor fora da
    /// faixa e curva insana num lado em Customized recusam o arquivo.
    pub fn parse(text: &str) -> Result<Self, SnapshotError> {
        let mut values: [Option<&str>; 7] = [None; 7];
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=').ok_or(SnapshotError::BadLine(n + 1))?;
            let i = KEYS
                .iter()
                .position(|&k| k == key)
                .ok_or_else(|| SnapshotError::UnknownKey(key.to_owned()))?;
            if values[i].replace(value).is_some() {
                return Err(SnapshotError::DuplicateKey(key.to_owned()));
            }
        }
        let get = |i: usize| values[i].ok_or(SnapshotError::MissingKey(KEYS[i]));
        let num = |i: usize| -> Result<u64, SnapshotError> {
            let v = get(i)?;
            v.parse().map_err(|_| SnapshotError::BadValue {
                key: KEYS[i],
                value: v.to_owned(),
            })
        };
        let byte = |i: usize| -> Result<u8, SnapshotError> {
            let v = get(i)?;
            v.parse().map_err(|_| SnapshotError::BadValue {
                key: KEYS[i],
                value: v.to_owned(),
            })
        };
        let curve = |i: usize| -> Result<[CurvePoint; 4], SnapshotError> {
            let v = get(i)?;
            parse_curve(v).ok_or_else(|| SnapshotError::BadValue {
                key: KEYS[i],
                value: v.to_owned(),
            })
        };

        let version = num(0)?;
        if version != u64::from(SNAPSHOT_VERSION) {
            return Err(SnapshotError::UnsupportedVersion(
                version.try_into().unwrap_or(u32::MAX),
            ));
        }
        let snapshot = Self {
            taken_unix: num(1)?,
            model: byte(2)?,
            fan_mode: byte(3)?,
            pump_mode: byte(4)?,
            fan_curve: curve(5)?,
            pump_curve: curve(6)?,
        };
        if model_name(snapshot.model).is_none() {
            return Err(SnapshotError::BadModel(snapshot.model));
        }
        for (key, mode, curve) in [
            ("fan_mode", snapshot.fan_mode, &snapshot.fan_curve),
            ("pump_mode", snapshot.pump_mode, &snapshot.pump_curve),
        ] {
            if !restorable_mode(mode) {
                return Err(SnapshotError::BadMode { key, value: mode });
            }
            if mode == mode::CUSTOMIZED {
                let curve_key = if key == "fan_mode" {
                    "fan_curve"
                } else {
                    "pump_curve"
                };
                check_curve(curve).map_err(|reason| SnapshotError::BadCurve {
                    key: curve_key,
                    reason,
                })?;
            }
        }
        Ok(snapshot)
    }

    /// O que a restauração envia: os dois modos e, nos lados em Customized, a
    /// curva salva.
    pub fn target(&self) -> Target {
        let custom = |mode: u8, curve: [CurvePoint; 4]| (mode == mode::CUSTOMIZED).then_some(curve);
        Target {
            fan: self.fan_mode,
            pump: self.pump_mode,
            fan_curve: custom(self.fan_mode, self.fan_curve),
            pump_curve: custom(self.pump_mode, self.pump_curve),
        }
    }
}

fn format_curve(curve: &[CurvePoint; 4]) -> String {
    let points: Vec<String> = curve
        .iter()
        .map(|p| format!("{}:{}", p.celsius, p.rpm))
        .collect();
    points.join(",")
}

fn parse_curve(text: &str) -> Option<[CurvePoint; 4]> {
    let points: Vec<CurvePoint> = text
        .split(',')
        .map(|p| {
            let (c, r) = p.split_once(':')?;
            Some(CurvePoint {
                celsius: c.parse().ok()?,
                rpm: r.parse().ok()?,
            })
        })
        .collect::<Option<_>>()?;
    points.try_into().ok()
}

/// Qual snapshot restaurar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    Previous,
    Original,
}

/// Diretório de estado: `$XDG_STATE_HOME/halo`, ou `~/.local/state/halo`. Um
/// `XDG_STATE_HOME` vazio ou relativo é ignorado, como manda a especificação.
pub fn state_dir_from(xdg_state_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let xdg = xdg_state_home
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    let base = match xdg {
        Some(p) => p,
        None => PathBuf::from(home.filter(|h| !h.is_empty())?).join(".local/state"),
    };
    Some(base.join("halo"))
}

/// [`state_dir_from`] com as variáveis do processo.
pub fn state_dir() -> Option<PathBuf> {
    state_dir_from(std::env::var_os("XDG_STATE_HOME"), std::env::var_os("HOME"))
}

/// Os dois snapshots num diretório de estado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotStore {
    dir: PathBuf,
}

impl SnapshotStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path(&self, which: Which) -> PathBuf {
        self.dir.join(match which {
            Which::Previous => "waterforce-speed.previous",
            Which::Original => "waterforce-speed.original",
        })
    }

    /// Lê e valida um snapshot.
    pub fn load(&self, which: Which) -> Result<Snapshot, SpeedError> {
        let path = self.path(which);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(SpeedError::NoSnapshot(path));
            }
            Err(e) => return Err(SpeedError::State(path, e)),
        };
        Snapshot::parse(&text).map_err(|e| SpeedError::BadSnapshot(path, e))
    }

    /// Grava `snapshot` como anterior e, se ainda não existir, como original.
    /// Devolve `true` se o original foi criado agora.
    pub fn save(&self, snapshot: &Snapshot) -> Result<bool, SpeedError> {
        fs::create_dir_all(&self.dir).map_err(|e| SpeedError::State(self.dir.clone(), e))?;
        let text = snapshot.serialize();
        let previous = self.path(Which::Previous);
        write_atomic(&previous, &text).map_err(|e| SpeedError::State(previous, e))?;
        let original = self.path(Which::Original);
        if original.exists() {
            return Ok(false);
        }
        write_atomic(&original, &text).map_err(|e| SpeedError::State(original, e))?;
        Ok(true)
    }
}

/// Escreve num temporário no mesmo diretório e troca com `rename`.
fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let name = path.file_name().unwrap().to_string_lossy();
    let tmp = path.with_file_name(format!(".{name}.tmp"));
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[derive(Debug)]
pub enum SpeedError {
    NotFound,
    Open(OpenError),
    Io(io::Error),
    /// Nenhuma resposta ao pedido no prazo.
    NoResponse(ReadQuery),
    /// `r[2]` do `DE` fora de 0, 1 e 2.
    UnknownModel(u8),
    /// Nem `--fan` nem `--pump`.
    NoSide,
    /// O lado não informado tem um valor que o halo não repete.
    CurrentModeUnknown {
        side: Side,
        raw: u8,
    },
    /// Sem `$XDG_STATE_HOME` nem `$HOME`.
    NoStateDir,
    /// Erro ao ler ou gravar o arquivo ou diretório de estado.
    State(PathBuf, io::Error),
    NoSnapshot(PathBuf),
    BadSnapshot(PathBuf, SnapshotError),
}

const NOTHING_SENT: &str = "nada foi enviado ao aparelho";

impl fmt::Display for SpeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(
                f,
                "Waterforce não encontrado ({VENDOR_ID:04x}:{PRODUCT_ID:04x})"
            ),
            Self::Open(e @ OpenError::PermissionDenied(_)) => write!(
                f,
                "{e}. Instale a regra udev do halo ({UDEV_RULE}, veja docs/permissions.md)"
            ),
            Self::Open(e) => write!(f, "{e}"),
            Self::Io(e) => write!(f, "erro de comunicação: {e}"),
            Self::NoResponse(q) => {
                let hex: Vec<String> = q.payload().iter().map(|b| format!("{b:02X}")).collect();
                write!(
                    f,
                    "o aparelho não respondeu ao pedido {} em 2 s; {NOTHING_SENT}",
                    hex.join(" ")
                )
            }
            Self::UnknownModel(v) => write!(f, "modelo desconhecido no DE ({v}); {NOTHING_SENT}"),
            Self::NoSide => write!(f, "informe --fan, --pump ou os dois"),
            Self::CurrentModeUnknown { side, raw } => write!(
                f,
                "o modo atual da {} é {raw}, que o halo não repete; informe o modo da {} ({NOTHING_SENT})",
                side.name(),
                side.name()
            ),
            Self::NoStateDir => write!(
                f,
                "sem XDG_STATE_HOME nem HOME para gravar o snapshot; {NOTHING_SENT}"
            ),
            Self::State(path, e) => write!(
                f,
                "não consegui usar {}: {e}; {NOTHING_SENT}",
                path.display()
            ),
            Self::NoSnapshot(path) => write!(
                f,
                "não há estado para restaurar ({} não existe); {NOTHING_SENT}",
                path.display()
            ),
            Self::BadSnapshot(path, e) => {
                write!(
                    f,
                    "snapshot {} recusado: {e}; {NOTHING_SENT}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for SpeedError {}

fn query(
    device: &(impl InputReports + OutputReports),
    q: ReadQuery,
) -> Result<Reading, SpeedError> {
    device
        .write_output_report(&q.packet())
        .map_err(SpeedError::Io)?;
    let response = wait_response(device, q.command(), RESPONSE_TIMEOUT)
        .map_err(SpeedError::Io)?
        .ok_or(SpeedError::NoResponse(q))?;
    q.decode(&response).ok_or(SpeedError::NoResponse(q))
}

/// Envia `DE 00`, `DD`, `D9 01` e `D9 02`, nessa ordem, e exige as quatro
/// respostas. Com `known_model`, para logo depois do `DE` se o modelo não for
/// um Waterforce X conhecido.
pub fn read_state(
    device: &(impl InputReports + OutputReports),
    known_model: bool,
) -> Result<Snapshot, SpeedError> {
    let Reading::Model(model) = query(device, ReadQuery::Model)? else {
        unreachable!()
    };
    if known_model && model_name(model).is_none() {
        return Err(SpeedError::UnknownModel(model));
    }
    let Reading::SpeedModes { fan, pump } = query(device, ReadQuery::SpeedModes)? else {
        unreachable!()
    };
    let Reading::Curve(fan_curve) = query(device, ReadQuery::FanCurve)? else {
        unreachable!()
    };
    let Reading::Curve(pump_curve) = query(device, ReadQuery::PumpCurve)? else {
        unreachable!()
    };
    let taken_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    Ok(Snapshot {
        taken_unix,
        model,
        fan_mode: fan.0,
        pump_mode: pump.0,
        fan_curve,
        pump_curve,
    })
}

/// Resultado de uma escrita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// Estado lido antes, gravado como snapshot anterior.
    pub before: Snapshot,
    /// Pacotes enviados depois do snapshot, sem o preenchimento.
    pub packets: Vec<Vec<u8>>,
    /// O snapshot original foi criado nesta execução.
    pub original_created: bool,
}

/// Lê o estado, decide o alvo com `plan`, grava o snapshot e só então envia a
/// sequência, com [`PACKET_INTERVAL`] entre os pacotes. As respostas aos
/// pacotes de escrita não são lidas, como no GCC.
fn write_to(
    device: &(impl InputReports + OutputReports),
    store: &SnapshotStore,
    plan: impl FnOnce(&Snapshot) -> Result<Target, SpeedError>,
    save: bool,
) -> Result<Written, SpeedError> {
    let before = read_state(device, true)?;
    let packets = plan(&before)?.packets(save);
    let original_created = store.save(&before)?;
    for (i, payload) in packets.iter().enumerate() {
        if i > 0 {
            std::thread::sleep(PACKET_INTERVAL);
        }
        device
            .write_output_report(&packet(&payload[1..]))
            .map_err(SpeedError::Io)?;
    }
    Ok(Written {
        before,
        packets,
        original_created,
    })
}

/// Pedido de mudança de preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetRequest {
    pub fan: Option<FanPreset>,
    pub pump: Option<PumpPreset>,
    /// `false` com o `--no-save`: os `E5` vão sem o `B6`.
    pub save: bool,
}

pub fn set_to(
    device: &(impl InputReports + OutputReports),
    store: &SnapshotStore,
    request: &SetRequest,
) -> Result<Written, SpeedError> {
    write_to(
        device,
        store,
        |s| {
            resolve_modes(
                request.fan,
                request.pump,
                SpeedMode(s.fan_mode),
                SpeedMode(s.pump_mode),
            )
        },
        request.save,
    )
}

/// Aplica `target`, um snapshot já validado, sempre com `B6`.
pub fn restore_to(
    device: &(impl InputReports + OutputReports),
    store: &SnapshotStore,
    target: &Snapshot,
) -> Result<Written, SpeedError> {
    write_to(device, store, |_| Ok(target.target()), true)
}

/// Estado mostrado pelo `status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedStatus {
    pub devnode: PathBuf,
    pub state: Snapshot,
}

fn open(sys_root: &Path) -> Result<(PathBuf, Hidraw), SpeedError> {
    let info = find_hidraw(sys_root)
        .map_err(SpeedError::Io)?
        .ok_or(SpeedError::NotFound)?;
    let device = Hidraw::open(&info.devnode).map_err(SpeedError::Open)?;
    Ok((info.devnode, device))
}

/// Só leitura: `DE 00`, `DD`, `D9 01` e `D9 02`. Não grava snapshot.
pub fn status(sys_root: &Path) -> Result<SpeedStatus, SpeedError> {
    let (devnode, device) = open(sys_root)?;
    let state = read_state(&device, false)?;
    Ok(SpeedStatus { devnode, state })
}

/// Resultado de [`set`] e [`restore`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub devnode: PathBuf,
    pub written: Written,
}

/// Recusa o pedido sem lado antes de abrir o hidraw e chama [`set_to`].
pub fn set(
    sys_root: &Path,
    store: &SnapshotStore,
    request: &SetRequest,
) -> Result<Applied, SpeedError> {
    if request.fan.is_none() && request.pump.is_none() {
        return Err(SpeedError::NoSide);
    }
    let (devnode, device) = open(sys_root)?;
    let written = set_to(&device, store, request)?;
    Ok(Applied { devnode, written })
}

/// Lê e valida o snapshot antes de abrir o hidraw e chama [`restore_to`].
pub fn restore(
    sys_root: &Path,
    store: &SnapshotStore,
    which: Which,
) -> Result<Applied, SpeedError> {
    let target = store.load(which)?;
    let (devnode, device) = open(sys_root)?;
    let written = restore_to(&device, store, &target)?;
    Ok(Applied { devnode, written })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::waterforce::led::tests::FakeDevice;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    fn curve(points: [(u8, u16); 4]) -> [CurvePoint; 4] {
        points.map(|(celsius, rpm)| CurvePoint { celsius, rpm })
    }

    /// `D9 01` lido em 2026-10-20.
    fn fan_slot() -> [CurvePoint; 4] {
        curve([(0, 1000), (30, 1000), (50, 1500), (80, 2500)])
    }

    fn pump_slot() -> [CurvePoint; 4] {
        curve([(0, 2800), (30, 2800), (50, 2800), (80, 2800)])
    }

    fn d9(side: u8, c: &[CurvePoint; 4]) -> Vec<u8> {
        let mut r = vec![0x99, 0xD9, side, side];
        for p in c {
            let [hi, lo] = p.rpm.to_be_bytes();
            r.extend([p.celsius, hi, lo]);
        }
        r
    }

    /// Respostas de um X 240 com `DD` = `fan pump`.
    fn device(fan: u8, pump: u8) -> FakeDevice {
        FakeDevice::new(&[
            &[0x99, 0xDA, 0x00],
            &[0x99, 0xDE, 0x00],
            &[0x99, 0xDD, fan, pump],
            &d9(1, &fan_slot()),
            &[0x99, 0xDA, 0x00],
            &d9(2, &pump_slot()),
        ])
    }

    fn snapshot(fan_mode: u8, pump_mode: u8) -> Snapshot {
        Snapshot {
            taken_unix: 1_792_540_800,
            model: 0,
            fan_mode,
            pump_mode,
            fan_curve: fan_slot(),
            pump_curve: pump_slot(),
        }
    }

    const READS: [&[u8]; 4] = [
        &[0x99, 0xDE, 0x00],
        &[0x99, 0xDD],
        &[0x99, 0xD9, 0x01],
        &[0x99, 0xD9, 0x02],
    ];

    fn store() -> (tempfile::TempDir, SnapshotStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(dir.path().join("halo"));
        (dir, store)
    }

    fn is_root() -> bool {
        fs::metadata("/proc/self").unwrap().uid() == 0
    }

    // 1.2

    #[test]
    fn presets_da_fan() {
        let expected = [
            ("default", 2),
            ("quiet", 6),
            ("balanced", 0),
            ("performance", 5),
            ("turbo", 4),
        ];
        for (name, wire) in expected {
            let p = FanPreset::from_name(name).unwrap();
            assert_eq!(p.wire(), wire, "{name}");
            assert_eq!(p.name(), name);
            assert_eq!(FanPreset::from_wire(wire), Some(p));
        }
        for refused in [mode::CUSTOMIZED, mode::FIXED_RPM, mode::ZERO_RPM, 8] {
            assert_eq!(FanPreset::from_wire(refused), None, "{refused}");
        }
        for refused in ["zero-rpm", "zerorpm", "customized", "fixed-rpm", "Quiet"] {
            assert_eq!(FanPreset::from_name(refused), None, "{refused}");
        }
    }

    #[test]
    fn presets_da_bomba() {
        assert_eq!(PumpPreset::from_name("balanced").unwrap().wire(), 0);
        assert_eq!(PumpPreset::from_name("turbo").unwrap().wire(), 4);
        for refused in ["quiet", "default", "performance", "customized"] {
            assert_eq!(PumpPreset::from_name(refused), None, "{refused}");
        }
        for refused in [1, 2, 3, 5, 6, 7] {
            assert_eq!(PumpPreset::from_wire(refused), None, "{refused}");
        }
    }

    // 1.3

    #[test]
    fn pacotes_dos_cenarios() {
        assert_eq!(
            set_mode_payload(Side::Fan, 6),
            [0x99, 0xE5, 0x01, 0x06, 0x00]
        );
        assert_eq!(
            set_mode_payload(Side::Pump, 4),
            [0x99, 0xE5, 0x02, 0x04, 0x00]
        );
        assert_eq!(
            set_curve_payload(Side::Fan, &fan_slot()),
            [
                0x99, 0xE6, 0x00, 0x01, 0x00, 0x03, 0xE8, 0x1E, 0x03, 0xE8, 0x32, 0x05, 0xDC, 0x50,
                0x09, 0xC4
            ]
        );
        assert_eq!(SAVE_PAYLOAD, [0x99, 0xB6]);
    }

    #[test]
    fn ordem_do_on_click_apply() {
        let t = Target {
            fan: 1,
            pump: 1,
            fan_curve: Some(fan_slot()),
            pump_curve: Some(pump_slot()),
        };
        let p = t.packets(true);
        let heads: Vec<&[u8]> = p.iter().map(|p| &p[..p.len().min(4)]).collect();
        assert_eq!(
            heads,
            [
                &[0x99, 0xE5, 0x01, 0x01][..],
                &[0x99, 0xE5, 0x02, 0x01],
                &[0x99, 0xE6, 0x00, 0x01],
                &[0x99, 0xE6, 0x00, 0x02],
                &[0x99, 0xB6],
            ]
        );
        assert_eq!(t.packets(false).len(), 4);
    }

    // 1.4

    #[test]
    fn so_a_bomba_informada() {
        let t =
            resolve_modes(None, Some(PumpPreset::Balanced), SpeedMode(5), SpeedMode(4)).unwrap();
        assert_eq!(
            t.packets(true),
            [
                vec![0x99, 0xE5, 0x01, 0x05, 0x00],
                vec![0x99, 0xE5, 0x02, 0x00, 0x00],
                vec![0x99, 0xB6],
            ]
        );
    }

    #[test]
    fn lado_atual_desconhecido() {
        for raw in [3, 8, 0xFF] {
            let err = resolve_modes(Some(FanPreset::Quiet), None, SpeedMode(5), SpeedMode(raw))
                .unwrap_err();
            assert!(
                matches!(err, SpeedError::CurrentModeUnknown { side: Side::Pump, raw: r } if r == raw)
            );
            let msg = err.to_string();
            assert!(msg.contains(&format!("é {raw}")), "{msg}");
            assert!(msg.contains("informe o modo da bomba"), "{msg}");
        }
        // Customized e ZeroRPM já estavam no aparelho e são repetidos.
        let t = resolve_modes(Some(FanPreset::Quiet), None, SpeedMode(5), SpeedMode(1)).unwrap();
        assert_eq!((t.fan, t.pump), (6, 1));
        assert!(matches!(
            resolve_modes(None, None, SpeedMode(5), SpeedMode(4)),
            Err(SpeedError::NoSide)
        ));
    }

    // 1.5

    #[test]
    fn tabelas_de_referencia() {
        let rpm = |c: Option<[CurvePoint; 4]>| c.map(|c| c.map(|p| p.rpm));
        let perf = reference_curve(0, Side::Fan, SpeedMode(mode::PERFORMANCE)).unwrap();
        assert_eq!(perf.map(|p| p.celsius), [0, 30, 50, 65]);
        assert_eq!(perf.map(|p| p.rpm), [2000, 2000, 2200, 2500]);
        assert_eq!(
            rpm(reference_curve(2, Side::Fan, SpeedMode(mode::PERFORMANCE))),
            Some([1800, 1800, 1800, 2500])
        );
        assert_eq!(
            rpm(reference_curve(1, Side::Pump, SpeedMode(mode::TURBO))),
            Some([3150; 4])
        );
        assert_eq!(
            reference_curve(0, Side::Fan, SpeedMode(mode::CUSTOMIZED)),
            None
        );
        assert_eq!(
            reference_curve(0, Side::Fan, SpeedMode(mode::FIXED_RPM)),
            None
        );
        assert_eq!(reference_curve(0, Side::Pump, SpeedMode(mode::QUIET)), None);
        assert_eq!(reference_curve(5, Side::Fan, SpeedMode(mode::TURBO)), None);
    }

    // 2.1

    #[test]
    fn snapshot_ida_e_volta() {
        let s = snapshot(1, 4);
        let text = s.serialize();
        assert!(text.contains("version=1\n"));
        assert!(text.contains("fan_curve=0:1000,30:1000,50:1500,80:2500\n"));
        assert_eq!(Snapshot::parse(&text), Ok(s));
    }

    #[test]
    fn snapshot_recusado() {
        let good = snapshot(5, 4).serialize();
        let cut = &good[..good.find("pump_curve").unwrap() + 20];
        assert!(matches!(
            Snapshot::parse(cut),
            Err(SnapshotError::BadValue {
                key: "pump_curve",
                ..
            })
        ));
        let cut = &good[..good.find("fan_curve").unwrap()];
        assert_eq!(
            Snapshot::parse(cut),
            Err(SnapshotError::MissingKey("fan_curve"))
        );
        assert_eq!(
            Snapshot::parse(""),
            Err(SnapshotError::MissingKey("version"))
        );

        let edit = |from: &str, to: &str| Snapshot::parse(&good.replace(from, to));
        assert_eq!(
            edit("fan_mode=5", "fan_mode=3"),
            Err(SnapshotError::BadMode {
                key: "fan_mode",
                value: 3
            })
        );
        assert_eq!(
            edit("pump_mode=4", "pump_mode=8"),
            Err(SnapshotError::BadMode {
                key: "pump_mode",
                value: 8
            })
        );
        assert_eq!(
            edit("version=1", "version=2"),
            Err(SnapshotError::UnsupportedVersion(2))
        );
        assert_eq!(edit("model=0", "model=5"), Err(SnapshotError::BadModel(5)));
        assert!(matches!(
            edit("model=0", "model=x"),
            Err(SnapshotError::BadValue { .. })
        ));
        assert_eq!(
            Snapshot::parse(&format!("{good}cor=azul\n")),
            Err(SnapshotError::UnknownKey("cor".into()))
        );
        assert_eq!(
            Snapshot::parse(&format!("{good}model=0\n")),
            Err(SnapshotError::DuplicateKey("model".into()))
        );
        assert_eq!(
            Snapshot::parse(&format!("{good}lixo\n")),
            Err(SnapshotError::BadLine(9))
        );
        // Três pontos.
        assert!(matches!(
            edit(",80:2500", ""),
            Err(SnapshotError::BadValue {
                key: "fan_curve",
                ..
            })
        ));

        // A curva a reenviar (fan em Customized) passa pela checagem.
        let custom = snapshot(1, 4).serialize();
        let edit = |from: &str, to: &str| Snapshot::parse(&custom.replace(from, to));
        assert!(matches!(
            edit("50:1500,80:2500", "80:1500,50:2500"),
            Err(SnapshotError::BadCurve {
                key: "fan_curve",
                ..
            })
        ));
        assert!(matches!(
            edit("80:2500", "80:3300"),
            Err(SnapshotError::BadCurve {
                key: "fan_curve",
                ..
            })
        ));
        assert!(matches!(
            edit("80:2500", "101:2500"),
            Err(SnapshotError::BadCurve {
                key: "fan_curve",
                ..
            })
        ));
        assert!(edit("80:2500", "80:3200").is_ok());
        // Num lado em preset, o slot não é reenviado e não é checado.
        assert!(Snapshot::parse(&good.replace("80:2500", "80:3300")).is_ok());
    }

    #[test]
    fn alvo_da_restauracao() {
        let t = snapshot(5, 4).target();
        assert_eq!(
            t.packets(true),
            [
                vec![0x99, 0xE5, 0x01, 0x05, 0x00],
                vec![0x99, 0xE5, 0x02, 0x04, 0x00],
                vec![0x99, 0xB6],
            ]
        );
        let t = snapshot(1, 4).target();
        assert_eq!(t.fan_curve, Some(fan_slot()));
        assert_eq!(t.pump_curve, None);
    }

    // 2.2

    #[test]
    fn diretorio_de_estado() {
        let dir = |x: Option<&str>, h: Option<&str>| {
            state_dir_from(x.map(OsString::from), h.map(OsString::from))
        };
        assert_eq!(
            dir(Some("/x/state"), Some("/home/u")),
            Some("/x/state/halo".into())
        );
        assert_eq!(
            dir(None, Some("/home/u")),
            Some("/home/u/.local/state/halo".into())
        );
        assert_eq!(
            dir(Some(""), Some("/home/u")),
            Some("/home/u/.local/state/halo".into())
        );
        assert_eq!(
            dir(Some("rel"), Some("/home/u")),
            Some("/home/u/.local/state/halo".into())
        );
        assert_eq!(dir(None, None), None);
    }

    #[test]
    fn primeira_mudanca_e_seguintes() {
        let (_tmp, store) = store();
        assert!(store.save(&snapshot(5, 4)).unwrap());
        let original = fs::read(store.path(Which::Original)).unwrap();
        assert_eq!(store.load(Which::Previous).unwrap(), snapshot(5, 4));
        assert_eq!(store.load(Which::Original).unwrap(), snapshot(5, 4));

        assert!(!store.save(&snapshot(6, 4)).unwrap());
        assert_eq!(store.load(Which::Previous).unwrap(), snapshot(6, 4));
        assert_eq!(fs::read(store.path(Which::Original)).unwrap(), original);
        // Nenhum temporário sobrando.
        assert_eq!(fs::read_dir(store.dir()).unwrap().count(), 2);
    }

    #[test]
    fn sem_snapshot_e_snapshot_corrompido() {
        let (_tmp, store) = store();
        assert!(matches!(
            store.load(Which::Previous),
            Err(SpeedError::NoSnapshot(_))
        ));
        fs::create_dir_all(store.dir()).unwrap();
        fs::write(store.path(Which::Previous), "version=1\ntaken_unix=1\nmod").unwrap();
        let err = store.load(Which::Previous).unwrap_err();
        assert!(matches!(err, SpeedError::BadSnapshot(..)));
        assert!(
            err.to_string().contains("waterforce-speed.previous"),
            "{err}"
        );
    }

    #[test]
    fn diretorio_sem_permissao() {
        if is_root() {
            eprintln!("teste pulado: rodando como root");
            return;
        }
        let (tmp, store) = store();
        fs::create_dir_all(store.dir()).unwrap();
        fs::set_permissions(store.dir(), fs::Permissions::from_mode(0o500)).unwrap();
        let err = store.save(&snapshot(5, 4)).unwrap_err();
        fs::set_permissions(store.dir(), fs::Permissions::from_mode(0o700)).unwrap();
        assert!(matches!(err, SpeedError::State(..)));
        assert!(
            err.to_string().contains(&tmp.path().display().to_string()),
            "{err}"
        );
        assert!(!store.path(Which::Previous).exists());
    }

    // 3.1

    #[test]
    fn fan_em_quiet_com_a_bomba_em_turbo() {
        let (_tmp, store) = store();
        let dev = device(5, 4);
        let request = SetRequest {
            fan: Some(FanPreset::Quiet),
            pump: None,
            save: true,
        };
        let w = set_to(&dev, &store, &request).unwrap();
        dev.assert_written(&[
            READS[0],
            READS[1],
            READS[2],
            READS[3],
            &[0x99, 0xE5, 0x01, 0x06, 0x00],
            &[0x99, 0xE5, 0x02, 0x04, 0x00],
            &[0x99, 0xB6],
        ]);
        assert!(w.original_created);
        assert_eq!(w.before.fan_mode, 5);
        assert_eq!(w.before.fan_curve, fan_slot());
        assert_eq!(store.load(Which::Previous).unwrap().fan_mode, 5);
    }

    #[test]
    fn sem_salvar_nao_envia_b6() {
        let (_tmp, store) = store();
        let dev = device(5, 4);
        let request = SetRequest {
            fan: Some(FanPreset::Quiet),
            pump: None,
            save: false,
        };
        set_to(&dev, &store, &request).unwrap();
        dev.assert_written(&[
            READS[0],
            READS[1],
            READS[2],
            READS[3],
            &[0x99, 0xE5, 0x01, 0x06, 0x00],
            &[0x99, 0xE5, 0x02, 0x04, 0x00],
        ]);
        assert!(store.path(Which::Previous).exists());
        assert!(store.path(Which::Original).exists());
    }

    #[test]
    fn modelo_desconhecido_nao_escreve() {
        let (_tmp, store) = store();
        let dev = FakeDevice::new(&[&[0x99, 0xDE, 0x05]]);
        let request = SetRequest {
            fan: Some(FanPreset::Quiet),
            pump: None,
            save: true,
        };
        let err = set_to(&dev, &store, &request).unwrap_err();
        assert!(matches!(err, SpeedError::UnknownModel(5)));
        assert!(err.to_string().contains("(5)"));
        dev.assert_written(&[READS[0]]);
        assert!(!store.dir().exists());
    }

    #[test]
    fn sem_resposta_ao_d9_02_nao_escreve() {
        let (_tmp, store) = store();
        let dev = FakeDevice::new(&[
            &[0x99, 0xDE, 0x00],
            &[0x99, 0xDD, 0x05, 0x04],
            &d9(1, &fan_slot()),
            &[0x99, 0xDA, 0x00],
        ]);
        let request = SetRequest {
            fan: Some(FanPreset::Quiet),
            pump: None,
            save: true,
        };
        let err = set_to(&dev, &store, &request).unwrap_err();
        assert!(matches!(err, SpeedError::NoResponse(ReadQuery::PumpCurve)));
        assert!(err.to_string().contains("D9 02"), "{err}");
        dev.assert_written(&READS);
        assert!(!store.dir().exists());
    }

    #[test]
    fn falha_no_snapshot_nao_escreve() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("arquivo");
        fs::write(&file, "").unwrap();
        // Um diretório dentro de um arquivo não pode ser criado, nem como root.
        let store = SnapshotStore::new(file.join("halo"));
        let dev = device(5, 4);
        let request = SetRequest {
            fan: Some(FanPreset::Quiet),
            pump: None,
            save: true,
        };
        let err = set_to(&dev, &store, &request).unwrap_err();
        assert!(matches!(err, SpeedError::State(..)));
        dev.assert_written(&READS);
    }

    #[test]
    fn lado_desconhecido_nao_escreve_nem_grava() {
        let (_tmp, store) = store();
        let dev = device(5, 3);
        let request = SetRequest {
            fan: Some(FanPreset::Quiet),
            pump: None,
            save: true,
        };
        let err = set_to(&dev, &store, &request).unwrap_err();
        assert!(matches!(err, SpeedError::CurrentModeUnknown { raw: 3, .. }));
        dev.assert_written(&READS);
        assert!(!store.dir().exists());
    }

    #[test]
    fn restauracao_para_customized() {
        let (_tmp, store) = store();
        let dev = device(6, 4);
        let w = restore_to(&dev, &store, &snapshot(1, 4)).unwrap();
        dev.assert_written(&[
            READS[0],
            READS[1],
            READS[2],
            READS[3],
            &[0x99, 0xE5, 0x01, 0x01, 0x00],
            &[0x99, 0xE5, 0x02, 0x04, 0x00],
            &[
                0x99, 0xE6, 0x00, 0x01, 0x00, 0x03, 0xE8, 0x1E, 0x03, 0xE8, 0x32, 0x05, 0xDC, 0x50,
                0x09, 0xC4,
            ],
            &[0x99, 0xB6],
        ]);
        // O estado de antes da restauração vira o anterior.
        assert_eq!(w.before.fan_mode, 6);
        assert_eq!(store.load(Which::Previous).unwrap().fan_mode, 6);
    }

    #[test]
    fn duas_restauracoes_alternam() {
        let (_tmp, store) = store();
        store.save(&snapshot(5, 4)).unwrap();
        // Aparelho em Quiet; o anterior é Performance.
        let target = store.load(Which::Previous).unwrap();
        let w = restore_to(&device(6, 4), &store, &target).unwrap();
        assert_eq!(w.packets[0], [0x99, 0xE5, 0x01, 0x05, 0x00]);
        // Agora em Performance; o anterior é Quiet.
        let target = store.load(Which::Previous).unwrap();
        let w = restore_to(&device(5, 4), &store, &target).unwrap();
        assert_eq!(w.packets[0], [0x99, 0xE5, 0x01, 0x06, 0x00]);
        assert_eq!(store.load(Which::Original).unwrap().fan_mode, 5);
    }

    // 3.2

    #[test]
    fn nao_encontrado_num_sysfs_vazio() {
        let sys = tempfile::tempdir().unwrap();
        let (_tmp, store) = store();
        assert!(matches!(status(sys.path()), Err(SpeedError::NotFound)));
        let request = SetRequest {
            fan: Some(FanPreset::Quiet),
            pump: None,
            save: true,
        };
        assert!(matches!(
            set(sys.path(), &store, &request),
            Err(SpeedError::NotFound)
        ));
        store.save(&snapshot(5, 4)).unwrap();
        assert!(matches!(
            restore(sys.path(), &store, Which::Previous),
            Err(SpeedError::NotFound)
        ));
    }

    #[test]
    fn recusas_antes_de_abrir() {
        let missing = Path::new("/caminho/que/nao/existe");
        let (_tmp, store) = store();
        let none = SetRequest {
            fan: None,
            pump: None,
            save: true,
        };
        assert!(matches!(
            set(missing, &store, &none),
            Err(SpeedError::NoSide)
        ));
        assert!(matches!(
            restore(missing, &store, Which::Original),
            Err(SpeedError::NoSnapshot(_))
        ));
    }

    #[test]
    fn mensagem_de_permissao_aponta_a_regra() {
        let err = SpeedError::Open(OpenError::PermissionDenied(PathBuf::from("/dev/hidraw2")));
        let msg = err.to_string();
        assert!(msg.contains("/dev/hidraw2"));
        assert!(msg.contains("packaging/udev/70-halo.rules"));
    }
}
