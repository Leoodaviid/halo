// SPDX-License-Identifier: GPL-2.0-or-later

//! Controlador RGB de cada pente Kingston Fury DDR4 da API v2 do FURY CTRL.
//!
//! No FURY CTRL 2.0.66.0, o `QueryDDRInformation` do serviço dá `api_ver = 2`
//! ao Fury Beast SE, ao Fury Beast Refresh e ao Fury Renegade Refresh DDR4, com
//! o controlador de cada pente em `0x58` + índice do SPD (`iDDRBaseAddress =
//! 88`, `address_offset = j_index`). No `FuryCTRL.dll`, no caminho comum (sem o
//! LPC Intel do X99/X299):
//! - `GetKingstonFuryDDR5MasterSlaveRole`, `Style` e `Speed` fazem read word
//!   data nos registradores `0x0B`, `0x09` e `0x0E` e guardam o byte alto, com
//!   `Sleep(5)` depois;
//! - cada `SetKingstonFuryDDR5*` faz write byte data com até 30 tentativas e
//!   `Sleep(5)` depois.
//!
//! Os registradores batem com o `KingstonFuryDRAMController` do OpenRGB
//! (`72262b8`). Ver `docs/hardware.md`, "Protocolo do Beast Refresh".
//!
//! Os endereços só existem como [`Controller`], que só nasce de um SPD lido e
//! confirmado como um modelo da API v2. O `I2cDev` nunca usa `I2C_SLAVE_FORCE`.
//! O resto do barramento (VRM em `0x60`, DACs em `0x10` e `0x15`, SPD, `0x27`)
//! nunca é endereçado.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{Brand, FindError, SpdRead, SpdSlot, Verdict, decode, find};
use crate::hid::hidraw::{LOCK_TIMEOUT, lock_exclusive};

/// Endereço de 7 bits do controlador do slot 0 (`iDDRBaseAddress = 88`).
pub const BASE_ADDRESS: u16 = 0x58;
/// Maior slot de SPD que o halo endereça.
pub const MAX_SLOT: u8 = 3;
/// Pausa depois de cada transação, como o `Sleep(5)` do `FuryCTRL.dll`.
pub const TRANSACTION_PAUSE: Duration = Duration::from_millis(5);

/// O controlador de um pente da API v2. Só é construído a partir de um SPD
/// confirmado (ver [`locate`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Controller {
    slot: u8,
}

impl Controller {
    /// Endereço de 7 bits: `0x58` + slot.
    pub fn address(self) -> u16 {
        BASE_ADDRESS + u16::from(self.slot)
    }

    /// Índice do slot de SPD (endereço do SPD − `0x50`).
    pub fn slot(self) -> u8 {
        self.slot
    }
}

/// Registradores que o halo lê (`GetKingstonFuryDDR5*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadReg {
    /// `0x0B`, índice de posição do pente (`FURY_REG_INDEX` do OpenRGB).
    Index,
    /// `0x09`, estilo do efeito.
    Style,
    /// `0x0E`, velocidade.
    Speed,
}

impl ReadReg {
    pub fn code(self) -> u8 {
        match self {
            Self::Index => 0x0B,
            Self::Style => 0x09,
            Self::Speed => 0x0E,
        }
    }
}

/// Registradores que o halo escreve (`SetKingstonFuryDDR5*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteReg {
    /// `0x08`: `0x53` início, `0x44` fim (`CmdStart`, `CmdDone`).
    Command,
    /// `0x09`.
    Style,
    /// `0x0B`, índice de posição.
    Index,
    /// `0x0C`.
    Direction,
    /// `0x0D` (`IRDelay`).
    Delay,
    /// `0x0E`.
    Speed,
    /// `0x10`.
    Width,
    /// `0x11`.
    Hue,
    /// `0x1E`.
    HyperSpeed,
    /// `0x20`.
    Brightness,
}

impl WriteReg {
    pub fn code(self) -> u8 {
        match self {
            Self::Command => 0x08,
            Self::Style => 0x09,
            Self::Index => 0x0B,
            Self::Direction => 0x0C,
            Self::Delay => 0x0D,
            Self::Speed => 0x0E,
            Self::Width => 0x10,
            Self::Hue => 0x11,
            Self::HyperSpeed => 0x1E,
            Self::Brightness => 0x20,
        }
    }
}

/// Transações que o halo faz nos controladores. Os endereços e registradores
/// vêm só dos tipos acima.
pub trait Smbus {
    /// Read word data. Devolve o word como o kernel o entrega.
    fn read_word_data(&mut self, controller: Controller, reg: ReadReg) -> io::Result<u16>;
    /// Write byte data.
    fn write_byte_data(
        &mut self,
        controller: Controller,
        reg: WriteReg,
        value: u8,
    ) -> io::Result<()>;
}

/// Um pente da API v2 e o controlador dele.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// Instância do `ee1004`, por exemplo `0-0052`.
    pub instance: String,
    pub controller: Controller,
}

/// Adaptador escolhido, o nó dele e os pentes controláveis, em ordem de slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub adapter: String,
    pub bus: u32,
    pub node: PathBuf,
    pub modules: Vec<Module>,
}

#[derive(Debug)]
pub enum LocateError {
    /// Sem adaptador PIIX4, sem `ee1004` ou erro do sysfs.
    Find(FindError),
    /// Os SPDs foram lidos e nenhum passa no critério do FURY CTRL.
    NoSupported,
    /// Há pentes suportados, mas nenhum da API v2 (ou fora dos slots 0–3).
    /// Leva a instância e o motivo.
    NotApiV2(Vec<(String, String)>),
    /// Nenhum pente da API v2 confirmado, e algum SPD não pôde ser lido
    /// (`EACCES`, outro erro, SPD curto) ou nenhum foi decodificado.
    SpdUnreadable {
        failures: Vec<(String, String)>,
        permission_denied: bool,
    },
    /// O adaptador não tem o nó do `i2c-dev`.
    NoNode { adapter: String, node: PathBuf },
}

fn join_pairs(pairs: &[(String, String)]) -> String {
    let list: Vec<String> = pairs.iter().map(|(i, r)| format!("{i}: {r}")).collect();
    list.join("; ")
}

impl fmt::Display for LocateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Find(e) => e.fmt(f),
            Self::NoSupported => write!(
                f,
                "nenhum pente Kingston DDR4 suportado pelo FURY CTRL no SMBus; nada foi enviado"
            ),
            Self::NotApiV2(modules) => write!(
                f,
                "nenhum pente da API v2 do FURY CTRL (Fury Beast SE, Beast Refresh ou Renegade Refresh) no SMBus ({}); nada foi enviado",
                join_pairs(modules)
            ),
            Self::SpdUnreadable { failures, .. } if failures.is_empty() => write!(
                f,
                "nenhum SPD pôde ser lido; não dá para confirmar o pente; nada foi enviado"
            ),
            Self::SpdUnreadable { failures, .. } => write!(
                f,
                "nenhum pente da API v2 confirmado, e o SPD não pôde ser lido em {}; nada foi enviado",
                join_pairs(failures)
            ),
            Self::NoNode { adapter, node } => write!(
                f,
                "o nó do i2c-dev de \"{adapter}\" ({}) não existe; nada foi enviado",
                node.display()
            ),
        }
    }
}

impl std::error::Error for LocateError {}

/// Como um SPD foi classificado.
enum Class {
    /// Instância sem resposta (`ENXIO`): não conta.
    Absent,
    /// Não pôde ser lido ou é curto: leva a causa.
    Unreadable(String),
    Rejected,
    /// Suportado, mas não endereçável por este módulo: leva o motivo.
    NotApiV2(String),
    ApiV2(Controller),
}

fn classify(slot: &SpdSlot) -> Class {
    let bytes = match &slot.read {
        SpdRead::NoResponse => return Class::Absent,
        SpdRead::PermissionDenied => return Class::Unreadable("sem permissão (EACCES)".into()),
        SpdRead::Failed(e) => return Class::Unreadable(e.to_string()),
        SpdRead::Bytes(b) => b,
    };
    let info = match decode(bytes) {
        Ok(info) => info,
        Err(e) => return Class::Unreadable(e.to_string()),
    };
    let Verdict::Supported { brand, model, name } = info.verdict else {
        return Class::Rejected;
    };
    let label = name.map_or_else(
        || format!("modelo desconhecido 0x{model:02X}"),
        str::to_owned,
    );
    if brand != Brand::Fury || !(2..=4).contains(&model) {
        return Class::NotApiV2(format!("{label}, API v1 do FURY CTRL (0x27)"));
    }
    let index = slot.slot();
    if index > MAX_SLOT {
        return Class::NotApiV2(format!("{label}, slot {index} fora de 0–{MAX_SLOT}"));
    }
    Class::ApiV2(Controller { slot: index })
}

/// Acha os pentes da API v2, o adaptador deles e o nó, só pelo sysfs. Não abre
/// nenhum `/dev/i2c-*`.
pub fn locate(sys_root: &Path, dev_root: &Path) -> Result<Target, LocateError> {
    let slots = find(sys_root).map_err(LocateError::Find)?;
    let mut controllable: Vec<(&SpdSlot, Controller)> = Vec::new();
    let mut failures = Vec::new();
    let mut permission_denied = false;
    let mut decoded = false;
    let mut not_v2 = Vec::new();
    for s in &slots {
        match classify(s) {
            Class::Absent => {}
            Class::Unreadable(cause) => {
                permission_denied |= matches!(s.read, SpdRead::PermissionDenied);
                failures.push((s.instance.clone(), cause));
            }
            Class::Rejected => decoded = true,
            Class::NotApiV2(reason) => {
                decoded = true;
                not_v2.push((s.instance.clone(), reason));
            }
            Class::ApiV2(c) => controllable.push((s, c)),
        }
    }
    let Some(&(first, _)) = controllable.first() else {
        return Err(if !failures.is_empty() || !decoded {
            LocateError::SpdUnreadable {
                failures,
                permission_denied,
            }
        } else if !not_v2.is_empty() {
            LocateError::NotApiV2(not_v2)
        } else {
            LocateError::NoSupported
        });
    };
    let bus = first.bus;
    let name = format!("i2c-{bus}");
    let node = dev_root.join(&name);
    let class = sys_root
        .join("bus/i2c/devices")
        .join(&name)
        .join("i2c-dev")
        .join(&name);
    if !class.exists() || !node.exists() {
        return Err(LocateError::NoNode {
            adapter: first.adapter.clone(),
            node,
        });
    }
    let mut modules: Vec<Module> = controllable
        .iter()
        .filter(|(s, _)| s.bus == bus)
        .map(|(s, c)| Module {
            instance: s.instance.clone(),
            controller: *c,
        })
        .collect();
    modules.sort_by_key(|m| m.controller);
    Ok(Target {
        adapter: first.adapter.clone(),
        bus,
        node,
        modules,
    })
}

/// Falha ao abrir o nó.
#[derive(Debug)]
pub enum OpenError {
    /// `EACCES`/`EPERM`: o nó é restrito ao root de propósito.
    PermissionDenied(PathBuf),
    /// Outro processo do halo segurou o nó além de [`LOCK_TIMEOUT`].
    Busy(PathBuf),
    /// `EBUSY` no `I2C_SLAVE`: um driver do kernel ocupa o endereço.
    AddressBusy(PathBuf, u16),
    Other(PathBuf, io::Error),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied(p) => write!(f, "sem permissão para abrir {}", p.display()),
            Self::Busy(p) => write!(f, "{} ocupado por outro processo do halo", p.display()),
            Self::AddressBusy(p, a) => write!(
                f,
                "o endereço 0x{a:02x} de {} está ocupado por um driver do kernel; nada foi enviado",
                p.display()
            ),
            Self::Other(p, e) => write!(f, "erro ao abrir {}: {e}", p.display()),
        }
    }
}

impl std::error::Error for OpenError {}

// linux/i2c-dev.h e linux/i2c.h.
const I2C_SLAVE: libc::c_ulong = 0x0703;
const I2C_SMBUS: libc::c_ulong = 0x0720;
const I2C_SMBUS_READ: u8 = 1;
const I2C_SMBUS_WRITE: u8 = 0;
const I2C_SMBUS_BYTE_DATA: u32 = 2;
const I2C_SMBUS_WORD_DATA: u32 = 3;

/// `union i2c_smbus_data`: 34 bytes, alinhada como o `__u16`.
#[repr(C, align(2))]
struct SmbusData([u8; 34]);

/// `struct i2c_smbus_ioctl_data`.
#[repr(C)]
struct SmbusIoctlData {
    read_write: u8,
    command: u8,
    size: u32,
    data: *mut SmbusData,
}

/// O nó `/dev/i2c-N`, com o `flock` tomado. Fala só com [`Controller`]s.
pub struct I2cDev {
    file: File,
    current: Option<Controller>,
}

impl I2cDev {
    /// Abre `node`, toma o `flock` (até [`LOCK_TIMEOUT`]) e confere com o
    /// `I2C_SLAVE` que nenhum driver ocupa os endereços de `controllers`. O
    /// `I2C_SLAVE` não manda nada ao barramento.
    pub fn open(node: &Path, controllers: &[Controller]) -> Result<Self, OpenError> {
        Self::open_with_timeout(node, controllers, LOCK_TIMEOUT)
    }

    pub fn open_with_timeout(
        node: &Path,
        controllers: &[Controller],
        timeout: Duration,
    ) -> Result<Self, OpenError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(node)
            .map_err(|e| match e.raw_os_error() {
                Some(libc::EACCES) | Some(libc::EPERM) => {
                    OpenError::PermissionDenied(node.to_owned())
                }
                _ => OpenError::Other(node.to_owned(), e),
            })?;
        match lock_exclusive(&file, timeout) {
            Ok(true) => {}
            Ok(false) => return Err(OpenError::Busy(node.to_owned())),
            Err(e) => return Err(OpenError::Other(node.to_owned(), e)),
        }
        let mut dev = Self {
            file,
            current: None,
        };
        for &c in controllers {
            dev.select(c).map_err(|e| {
                if e.raw_os_error() == Some(libc::EBUSY) {
                    OpenError::AddressBusy(node.to_owned(), c.address())
                } else {
                    OpenError::Other(node.to_owned(), e)
                }
            })?;
        }
        Ok(dev)
    }

    fn select(&mut self, c: Controller) -> io::Result<()> {
        if self.current == Some(c) {
            return Ok(());
        }
        // SAFETY: o descritor pertence a `file`; o `I2C_SLAVE` recebe o
        // endereço por valor.
        let r = unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                I2C_SLAVE as _,
                libc::c_ulong::from(c.address()),
            )
        };
        if r < 0 {
            self.current = None;
            return Err(io::Error::last_os_error());
        }
        self.current = Some(c);
        Ok(())
    }

    fn smbus(
        &self,
        read_write: u8,
        command: u8,
        size: u32,
        data: &mut SmbusData,
    ) -> io::Result<()> {
        let mut args = SmbusIoctlData {
            read_write,
            command,
            size,
            data,
        };
        // SAFETY: `args` e `data` vivem durante a chamada e têm o layout do
        // kernel; para BYTE_DATA e WORD_DATA o kernel só toca nos dois
        // primeiros bytes.
        let r = unsafe { libc::ioctl(self.file.as_raw_fd(), I2C_SMBUS as _, &mut args) };
        if r < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl Smbus for I2cDev {
    fn read_word_data(&mut self, controller: Controller, reg: ReadReg) -> io::Result<u16> {
        self.select(controller)?;
        let mut data = SmbusData([0; 34]);
        self.smbus(I2C_SMBUS_READ, reg.code(), I2C_SMBUS_WORD_DATA, &mut data)?;
        Ok(u16::from_le_bytes([data.0[0], data.0[1]]))
    }

    fn write_byte_data(
        &mut self,
        controller: Controller,
        reg: WriteReg,
        value: u8,
    ) -> io::Result<()> {
        self.select(controller)?;
        let mut data = SmbusData([0; 34]);
        data.0[0] = value;
        self.smbus(I2C_SMBUS_WRITE, reg.code(), I2C_SMBUS_BYTE_DATA, &mut data)
    }
}

/// Falha de leitura de um controlador.
#[derive(Debug)]
pub enum ReadError {
    /// `ENXIO`: nada respondeu no endereço.
    NoResponse {
        controller: Controller,
        reg: ReadReg,
    },
    /// Outra falha: não prova ausência.
    Io {
        controller: Controller,
        reg: ReadReg,
        err: io::Error,
    },
    /// A última tentativa devolveu byte alto `0xFF` (lixo do barramento).
    Invalid {
        controller: Controller,
        reg: ReadReg,
    },
}

impl ReadError {
    fn new(controller: Controller, reg: ReadReg, err: io::Error) -> Self {
        if err.raw_os_error() == Some(libc::ENXIO) {
            Self::NoResponse { controller, reg }
        } else {
            Self::Io {
                controller,
                reg,
                err,
            }
        }
    }

    pub fn is_no_response(&self) -> bool {
        matches!(self, Self::NoResponse { .. })
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoResponse { controller, .. } => {
                write!(f, "nada respondeu em 0x{:02x}", controller.address())
            }
            Self::Invalid { controller, reg } => write!(
                f,
                "leitura inválida (0xFF) do registrador 0x{:02X} em 0x{:02x} nas {READ_ATTEMPTS} tentativas",
                reg.code(),
                controller.address()
            ),
            Self::Io {
                controller,
                reg,
                err,
            } => write!(
                f,
                "a leitura do registrador 0x{:02X} em 0x{:02x} falhou: {err}",
                reg.code(),
                controller.address()
            ),
        }
    }
}

impl std::error::Error for ReadError {}

/// Tentativas por leitura, como o `SmbusRead` do OpenRGB.
pub const READ_ATTEMPTS: u32 = 5;
/// Passo da pausa entre tentativas de leitura (10 ms × tentativa).
pub const READ_RETRY_STEP: Duration = Duration::from_millis(10);

/// Lê um registrador e devolve o byte alto, como o `FuryCTRL.dll` e o OpenRGB,
/// com a pausa de 5 ms depois de cada transação. Só repete enquanto a leitura
/// falha ou devolve `0xFF`, até [`READ_ATTEMPTS`] vezes.
pub fn read_register(
    bus: &mut impl Smbus,
    controller: Controller,
    reg: ReadReg,
) -> Result<u8, ReadError> {
    let mut all_no_response = true;
    let mut last = None;
    for attempt in 1..=READ_ATTEMPTS {
        let word = bus.read_word_data(controller, reg);
        std::thread::sleep(TRANSACTION_PAUSE);
        match word {
            Ok(w) => {
                let value = (w >> 8) as u8;
                if value != 0xFF {
                    return Ok(value);
                }
                all_no_response = false;
                last = Some(ReadError::Invalid { controller, reg });
            }
            Err(e) => {
                let err = ReadError::new(controller, reg, e);
                all_no_response &= err.is_no_response();
                last = Some(err);
            }
        }
        if attempt < READ_ATTEMPTS {
            std::thread::sleep(READ_RETRY_STEP * attempt);
        }
    }
    let last = last.expect("pelo menos uma tentativa");
    // `NoResponse` só quando todas as tentativas foram `ENXIO`.
    Err(match last {
        ReadError::NoResponse { .. } if !all_no_response => ReadError::Io {
            controller,
            reg,
            err: io::Error::from_raw_os_error(libc::ENXIO),
        },
        other => other,
    })
}

/// Índice, estilo e velocidade lidos de um controlador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registers {
    pub index: u8,
    pub style: u8,
    pub speed: u8,
}

/// Nome do estilo (`DDR5LEDmode` do FURY CTRL e `FURY_MODE_*` do OpenRGB).
pub fn style_name(style: u8) -> Option<&'static str> {
    match style {
        0 => Some("estático"),
        1 => Some("rainbow"),
        3 => Some("respiração"),
        4 => Some("dinâmico"),
        _ => None,
    }
}

/// Lê `0x0B`, `0x09` e `0x0E` de cada controlador, nessa ordem. Num controlador
/// que falha, as leituras seguintes dele não são feitas; os outros são lidos.
pub fn read_controllers(
    bus: &mut impl Smbus,
    modules: &[Module],
) -> Vec<Result<Registers, ReadError>> {
    modules
        .iter()
        .map(|m| {
            let c = m.controller;
            Ok(Registers {
                index: read_register(bus, c, ReadReg::Index)?,
                style: read_register(bus, c, ReadReg::Style)?,
                speed: read_register(bus, c, ReadReg::Speed)?,
            })
        })
        .collect()
}

/// Resultado geral das leituras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOutcome {
    /// Todos os controladores responderam.
    AllOk,
    /// Houve falhas, todas `ENXIO`.
    NoResponse,
    /// Alguma falha não foi `ENXIO`.
    Failed,
}

pub fn read_outcome(results: &[Result<Registers, ReadError>]) -> ReadOutcome {
    let mut failures = results.iter().filter_map(|r| r.as_ref().err()).peekable();
    if failures.peek().is_none() {
        ReadOutcome::AllOk
    } else if failures.all(ReadError::is_no_response) {
        ReadOutcome::NoResponse
    } else {
        ReadOutcome::Failed
    }
}

/// Pausa depois do `CommandStart` e do `CommandDoneAndRun` do serviço.
pub const FRAME_PAUSE: Duration = Duration::from_millis(50);
/// Tentativas por escrita, como o `0x1E` do `FuryCTRL.dll`.
pub const WRITE_ATTEMPTS: u32 = 30;
/// `CmdStart` e `CmdDone` (registrador `0x08`).
pub const CMD_START: u8 = 0x53;
pub const CMD_DONE: u8 = 0x44;
/// Brilho padrão do serviço (`_brightness = 80`).
pub const DEFAULT_BRIGHTNESS: u8 = 80;

/// As três escritas mínimas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RamCommand {
    /// `setDRAMDDR5_AllOff`: papel e brilho 0.
    Off,
    /// A mesma moldura com o brilho padrão (80).
    On,
    /// `setDRAMDDR5_DefaultRainbow`.
    Default,
}

impl RamCommand {
    pub const NAMES: [&'static str; 3] = ["off", "on", "default"];

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "off" => Some(Self::Off),
            "on" => Some(Self::On),
            "default" => Some(Self::Default),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
            Self::Default => "default",
        }
    }

    /// Escritas de cada pente, na ordem do serviço.
    fn writes(self) -> &'static [(WriteReg, Value)] {
        use Value::{Fixed, Index};
        match self {
            Self::Off => &[(WriteReg::Index, Index), (WriteReg::Brightness, Fixed(0))],
            Self::On => &[
                (WriteReg::Index, Index),
                (WriteReg::Brightness, Fixed(DEFAULT_BRIGHTNESS)),
            ],
            Self::Default => &[
                (WriteReg::Style, Fixed(1)),
                (WriteReg::Index, Index),
                (WriteReg::Delay, Fixed(0)),
                (WriteReg::Direction, Fixed(1)),
                (WriteReg::Speed, Fixed(25)),
                (WriteReg::Brightness, Fixed(DEFAULT_BRIGHTNESS)),
                (WriteReg::Width, Fixed(0)),
                (WriteReg::Hue, Fixed(0)),
                (WriteReg::HyperSpeed, Fixed(0)),
            ],
        }
    }
}

/// Valor de uma escrita da tabela de cada comando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Value {
    Fixed(u8),
    /// O índice de posição do pente ([`index_of`]).
    Index,
}

/// Índice de posição de `c` entre `controllers`: os slots em ordem decrescente
/// recebem 0, 1, 2… (o FURY CTRL escreve os índices em ordem decrescente; o
/// OpenRGB chama o `0x0B` de `FURY_REG_INDEX`). Nunca vem de uma leitura.
pub fn index_of(c: Controller, controllers: &[Controller]) -> u8 {
    controllers.iter().filter(|o| o.slot > c.slot).count() as u8
}

/// Um passo do plano. O plano não tem leituras. Privado: fora deste módulo,
/// passos só existem dentro de um [`Plan`] montado por [`plan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Write(Controller, WriteReg, u8),
    Pause(Duration),
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(c, reg, v) => write!(
                f,
                "0x{:02x}  escreve 0x{:02X} = 0x{v:02X}",
                c.address(),
                reg.code()
            ),
            Self::Pause(d) => write!(f, "pausa de {} ms", d.as_millis()),
        }
    }
}

/// Sequência de escritas de um comando. Opaca: só [`plan`] a monta, com os
/// registradores e valores da spec, e só [`apply`] a executa.
///
/// Fora do módulo não dá para montar um passo com outro valor:
///
/// ```compile_fail
/// use halo_core::ram::controller::Step;
/// ```
///
/// Nem executar o texto do `--dry-run`, que não é um [`Plan`]:
///
/// ```compile_fail
/// use halo_core::ram::controller::{apply, dry_run_lines, I2cDev, RamCommand};
/// let mut bus = I2cDev::open(std::path::Path::new("/dev/null"), &[]).unwrap();
/// apply(&mut bus, &dry_run_lines(RamCommand::Off)).unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    steps: Vec<Step>,
}

impl Plan {
    /// Uma linha por passo, para mostrar.
    pub fn lines(&self) -> Vec<String> {
        self.steps.iter().map(ToString::to_string).collect()
    }
}

/// Plano completo de `command` sobre `controllers`: `CommandStart`, as escritas
/// de cada pente e `CommandDoneAndRun`.
pub fn plan(command: RamCommand, controllers: &[Controller]) -> Plan {
    let mut sorted = controllers.to_vec();
    sorted.sort();
    sorted.dedup();
    let mut steps = Vec::new();
    for &c in sorted.iter().rev() {
        steps.push(Step::Write(c, WriteReg::Command, CMD_START));
    }
    steps.push(Step::Pause(FRAME_PAUSE));
    for &c in &sorted {
        for &(reg, value) in command.writes() {
            let value = match value {
                Value::Fixed(v) => v,
                Value::Index => index_of(c, &sorted),
            };
            steps.push(Step::Write(c, reg, value));
        }
    }
    for &c in &sorted {
        steps.push(Step::Write(c, WriteReg::Command, CMD_DONE));
    }
    steps.push(Step::Pause(FRAME_PAUSE));
    Plan { steps }
}

/// Texto do `--dry-run`, com os controladores dos pentes `0x52` e `0x53`
/// desta máquina (`0x5A` e `0x5B`). Só devolve texto: nenhum [`Controller`] nem
/// [`Plan`] sai daqui sem um SPD confirmado.
pub fn dry_run_lines(command: RamCommand) -> Vec<String> {
    plan(command, &[Controller { slot: 2 }, Controller { slot: 3 }]).lines()
}

/// Falha ao aplicar um plano.
#[derive(Debug)]
pub enum ApplyError {
    /// A primeira escrita deu `ENXIO` em todas as tentativas: nada foi escrito.
    NoResponse { controller: Controller },
    /// Uma escrita falhou nas 30 tentativas: as seguintes não foram enviadas.
    Write {
        controller: Controller,
        reg: WriteReg,
        value: u8,
        err: io::Error,
    },
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoResponse { controller } => write!(
                f,
                "nada respondeu em 0x{:02x}; nada foi escrito",
                controller.address()
            ),
            Self::Write {
                controller,
                reg,
                value,
                err,
            } => write!(
                f,
                "a escrita 0x{:02X} = 0x{value:02X} em 0x{:02x} falhou nas {WRITE_ATTEMPTS} tentativas ({err}); \
                 a moldura ficou incompleta e as escritas seguintes não foram enviadas. \
                 Repita o mesmo comando para refazê-la desde o começo",
                reg.code(),
                controller.address()
            ),
        }
    }
}

impl std::error::Error for ApplyError {}

/// Executa `steps` e para na primeira falha definitiva. Não lê nada.
pub fn apply(bus: &mut impl Smbus, plan: &Plan) -> Result<(), ApplyError> {
    let mut written = false;
    for step in &plan.steps {
        match *step {
            Step::Pause(d) => std::thread::sleep(d),
            Step::Write(c, reg, value) => {
                if let Err((err, all_no_response)) = write_with_retries(bus, c, reg, value) {
                    return Err(if !written && all_no_response {
                        ApplyError::NoResponse { controller: c }
                    } else {
                        ApplyError::Write {
                            controller: c,
                            reg,
                            value,
                            err,
                        }
                    });
                }
                written = true;
            }
        }
    }
    Ok(())
}

/// Até [`WRITE_ATTEMPTS`] tentativas. No erro, devolve a última causa e se
/// todas foram `ENXIO`.
fn write_with_retries(
    bus: &mut impl Smbus,
    c: Controller,
    reg: WriteReg,
    value: u8,
) -> Result<(), (io::Error, bool)> {
    let mut all_no_response = true;
    let mut attempt = 1;
    loop {
        match bus.write_byte_data(c, reg, value) {
            Ok(()) => {
                std::thread::sleep(TRANSACTION_PAUSE);
                return Ok(());
            }
            Err(err) => {
                all_no_response &= err.raw_os_error() == Some(libc::ENXIO);
                if attempt >= WRITE_ATTEMPTS {
                    return Err((err, all_no_response));
                }
                attempt += 1;
                std::thread::sleep(TRANSACTION_PAUSE);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::{HashMap, VecDeque};
    use std::fs;

    /// Transação gravada pelo [`FakeBus`].
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum Op {
        Read(u16, u8),
        Write(u16, u8, u8),
    }

    /// `Smbus` falso: grava as transações, devolve os words de `words` por
    /// (endereço, registrador) e as falhas da fila `errors` (vazia = sucesso).
    #[derive(Default)]
    pub(crate) struct FakeBus {
        pub ops: Vec<Op>,
        pub at: Vec<std::time::Instant>,
        pub words: HashMap<(u16, u8), u16>,
        /// Words devolvidos em ordem, antes de `words`, por qualquer leitura.
        pub queue: VecDeque<u16>,
        pub errors: VecDeque<Option<i32>>,
    }

    impl FakeBus {
        fn next(&mut self) -> io::Result<()> {
            match self.errors.pop_front().flatten() {
                Some(errno) => Err(io::Error::from_raw_os_error(errno)),
                None => Ok(()),
            }
        }
    }

    impl Smbus for FakeBus {
        fn read_word_data(&mut self, c: Controller, reg: ReadReg) -> io::Result<u16> {
            self.ops.push(Op::Read(c.address(), reg.code()));
            self.at.push(std::time::Instant::now());
            self.next()?;
            if let Some(w) = self.queue.pop_front() {
                return Ok(w);
            }
            Ok(*self.words.get(&(c.address(), reg.code())).unwrap_or(&0))
        }

        fn write_byte_data(&mut self, c: Controller, reg: WriteReg, value: u8) -> io::Result<()> {
            self.ops.push(Op::Write(c.address(), reg.code(), value));
            self.at.push(std::time::Instant::now());
            self.next()
        }
    }

    pub(crate) fn controller(slot: u8) -> Controller {
        Controller { slot }
    }

    pub(crate) fn module(slot: u8) -> Module {
        Module {
            instance: format!("0-005{}", slot),
            controller: controller(slot),
        }
    }

    /// SPD de 512 bytes com marca, modelo e byte 360 dados.
    fn spd(brand: u8, model: u8, rgb: u8) -> Vec<u8> {
        let mut spd = vec![0u8; super::super::SPD_SIZE];
        spd[2] = 0x0C;
        spd[320] = 0x01;
        spd[321] = 0x98;
        spd[329..349].copy_from_slice(b"KF3200C16D4/16GX    ");
        spd[330] = brand;
        spd[359] = model;
        spd[360] = rgb;
        spd
    }

    fn beast_refresh() -> Vec<u8> {
        spd(b'F', 4, 0x23)
    }

    /// sysfs e /dev falsos: adaptador PIIX4 `i2c-<bus>` com os SPDs dados.
    fn fake(bus: u32, spds: &[(&str, Vec<u8>)], i2c_dev: bool) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let name = format!("i2c-{bus}");
        let adapter = root.path().join("sys/bus/i2c/devices").join(&name);
        fs::create_dir_all(&adapter).unwrap();
        fs::write(adapter.join("name"), "SMBus PIIX4 adapter port 0 at 0b00\n").unwrap();
        for (instance, bytes) in spds {
            let dir = adapter.join(instance);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("name"), "ee1004\n").unwrap();
            fs::write(dir.join("eeprom"), bytes).unwrap();
        }
        fs::create_dir_all(root.path().join("dev")).unwrap();
        if i2c_dev {
            fs::create_dir_all(adapter.join("i2c-dev").join(&name)).unwrap();
            fs::write(root.path().join("dev").join(&name), "").unwrap();
        }
        root
    }

    fn sys(root: &tempfile::TempDir) -> PathBuf {
        root.path().join("sys")
    }

    fn dev(root: &tempfile::TempDir) -> PathBuf {
        root.path().join("dev")
    }

    #[test]
    fn ram_controlador_dos_pentes_desta_maquina() {
        let root = fake(
            0,
            &[("0-0053", beast_refresh()), ("0-0052", beast_refresh())],
            true,
        );
        let t = locate(&sys(&root), &dev(&root)).unwrap();
        assert_eq!(t.bus, 0);
        assert_eq!(t.node, dev(&root).join("i2c-0"));
        let addrs: Vec<u16> = t.modules.iter().map(|m| m.controller.address()).collect();
        assert_eq!(addrs, [0x5A, 0x5B]);
        assert_eq!(t.modules[0].instance, "0-0052");
    }

    #[test]
    fn ram_controlador_so_api_v2() {
        for (brand, model) in [(b'F', 2), (b'F', 3), (b'F', 4)] {
            let root = fake(0, &[("0-0052", spd(brand, model, 0x23))], true);
            assert!(locate(&sys(&root), &dev(&root)).is_ok(), "modelo {model}");
        }
        for (brand, model) in [(b'F', 1), (b'F', 0), (b'H', 0), (b'H', 1), (b'H', 3)] {
            let root = fake(0, &[("0-0052", spd(brand, model, 0x10))], true);
            let err = locate(&sys(&root), &dev(&root)).unwrap_err();
            assert!(
                matches!(err, LocateError::NotApiV2(_)),
                "{brand} {model}: {err:?}"
            );
            assert!(err.to_string().contains("0-0052"), "{err}");
        }
        let root = fake(0, &[("0-0052", spd(b'F', 1, 0x23))], true);
        let msg = locate(&sys(&root), &dev(&root)).unwrap_err().to_string();
        assert!(msg.contains("Fury Beast, API v1"), "{msg}");
    }

    #[test]
    fn ram_controlador_sem_pente_suportado() {
        let root = fake(0, &[("0-0052", spd(b'F', 4, 0x00))], true);
        assert!(matches!(
            locate(&sys(&root), &dev(&root)),
            Err(LocateError::NoSupported)
        ));
        let mut other = beast_refresh();
        other[320] = 0x80;
        other[321] = 0x2C;
        let root = fake(0, &[("0-0052", other)], true);
        assert!(matches!(
            locate(&sys(&root), &dev(&root)),
            Err(LocateError::NoSupported)
        ));
    }

    #[test]
    fn ram_controlador_spd_ilegivel_nao_e_falta_de_suporte() {
        use std::os::unix::fs::PermissionsExt;
        let root = fake(0, &[("0-0052", vec![0; 256])], true);
        let Err(LocateError::SpdUnreadable {
            failures,
            permission_denied,
        }) = locate(&sys(&root), &dev(&root))
        else {
            panic!("esperado SpdUnreadable");
        };
        assert!(!permission_denied);
        assert!(failures[0].1.contains("256 bytes"), "{failures:?}");

        // Reprovado ao lado de um ilegível: o ilegível pode ser o suportado.
        let root = fake(
            0,
            &[("0-0052", spd(b'F', 4, 0x00)), ("0-0053", vec![0; 256])],
            true,
        );
        assert!(matches!(
            locate(&sys(&root), &dev(&root)),
            Err(LocateError::SpdUnreadable { .. })
        ));

        let root = fake(0, &[("0-0052", beast_refresh())], true);
        let eeprom = sys(&root).join("bus/i2c/devices/i2c-0/0-0052/eeprom");
        fs::set_permissions(&eeprom, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read(&eeprom).is_ok() {
            eprintln!("aviso: o modo 000 não bloqueia a leitura (root?); parte pulada");
            return;
        }
        let err = locate(&sys(&root), &dev(&root)).unwrap_err();
        assert!(matches!(
            err,
            LocateError::SpdUnreadable {
                permission_denied: true,
                ..
            }
        ));
        assert!(err.to_string().contains("EACCES"), "{err}");
    }

    #[test]
    fn ram_controlador_fantasma_nao_conta() {
        let slot = |instance: &str, read| SpdSlot {
            adapter: "SMBus PIIX4 adapter port 1 at 0b20".into(),
            bus: 2,
            instance: instance.into(),
            address: 0x51,
            read,
        };
        assert!(matches!(
            classify(&slot("2-0051", SpdRead::NoResponse)),
            Class::Absent
        ));
        assert!(matches!(
            classify(&slot("2-0051", SpdRead::Bytes(spd(b'F', 4, 0x00)))),
            Class::Rejected
        ));
    }

    #[test]
    fn ram_controlador_sem_no_do_i2c_dev() {
        let root = fake(0, &[("0-0052", beast_refresh())], false);
        let Err(LocateError::NoNode { node, .. }) = locate(&sys(&root), &dev(&root)) else {
            panic!("esperado NoNode");
        };
        assert_eq!(node, dev(&root).join("i2c-0"));
    }

    #[test]
    fn ram_controlador_sem_adaptador() {
        let root = tempfile::tempdir().unwrap();
        assert!(matches!(
            locate(root.path(), root.path()),
            Err(LocateError::Find(FindError::NoAdapter))
        ));
    }

    #[test]
    fn ram_leitura_dos_controladores() {
        let mut bus = FakeBus::default();
        bus.words.insert((0x5A, 0x0B), 0x0012);
        bus.words.insert((0x5A, 0x09), 0x0400);
        bus.words.insert((0x5A, 0x0E), 0x19FF);
        bus.words.insert((0x5B, 0x0B), 0x0100);
        let r = read_controllers(&mut bus, &[module(2), module(3)]);
        assert_eq!(
            r[0].as_ref().unwrap(),
            &Registers {
                index: 0x00,
                style: 0x04,
                speed: 0x19
            }
        );
        assert_eq!(r[1].as_ref().unwrap().index, 0x01);
        assert_eq!(
            bus.ops,
            [
                Op::Read(0x5A, 0x0B),
                Op::Read(0x5A, 0x09),
                Op::Read(0x5A, 0x0E),
                Op::Read(0x5B, 0x0B),
                Op::Read(0x5B, 0x09),
                Op::Read(0x5B, 0x0E),
            ]
        );
        assert_eq!(style_name(4), Some("dinâmico"));
        assert_eq!(style_name(2), None);
    }

    const ENXIO5: [Option<i32>; 5] = [Some(libc::ENXIO); 5];

    #[test]
    fn ram_leitura_enxio_e_eio() {
        // 5 × ENXIO no 0x5A: nada respondeu; o 0x5B é lido inteiro.
        let mut bus = FakeBus::default();
        bus.errors.extend(ENXIO5);
        let r = read_controllers(&mut bus, &[module(2), module(3)]);
        let err = r[0].as_ref().unwrap_err();
        assert!(err.is_no_response());
        assert_eq!(err.to_string(), "nada respondeu em 0x5a");
        assert!(r[1].is_ok());
        assert_eq!(bus.ops.len(), 5 + 3);

        // A última tentativa com EIO: erro com a causa, não ausência.
        let mut bus = FakeBus::default();
        bus.errors.extend([Some(libc::EIO); 5]);
        let r = read_controllers(&mut bus, &[module(2)]);
        let err = r[0].as_ref().unwrap_err();
        assert!(!err.is_no_response());
        let msg = err.to_string();
        assert!(msg.contains("0x0B") && msg.contains("falhou"), "{msg}");
        assert!(!msg.contains("nada respondeu"), "{msg}");

        // ENXIO misturado com EIO não é "nada respondeu".
        let mut bus = FakeBus::default();
        bus.errors.extend([
            Some(libc::EIO),
            Some(libc::ENXIO),
            Some(libc::ENXIO),
            Some(libc::ENXIO),
            Some(libc::ENXIO),
        ]);
        let err = read_register(&mut bus, controller(2), ReadReg::Index).unwrap_err();
        assert!(!err.is_no_response(), "{err}");
    }

    #[test]
    fn ram_leitura_com_tentativas() {
        // Lixo isolado: refeito, e o valor bom é usado.
        let mut bus = FakeBus::default();
        bus.queue.extend([0xFF00, 0x0100]);
        assert_eq!(
            read_register(&mut bus, controller(2), ReadReg::Style).unwrap(),
            0x01
        );
        assert_eq!(bus.ops.len(), 2);

        // Uma leitura boa não se repete.
        let mut bus = FakeBus::default();
        bus.queue.push_back(0x0100);
        read_register(&mut bus, controller(2), ReadReg::Style).unwrap();
        assert_eq!(bus.ops.len(), 1);

        // ENXIO seguido de sucesso segue.
        let mut bus = FakeBus::default();
        bus.errors.push_back(Some(libc::ENXIO));
        bus.queue.push_back(0x0400);
        assert_eq!(
            read_register(&mut bus, controller(3), ReadReg::Style).unwrap(),
            0x04
        );

        // 5 × 0xFF: leitura inválida, que não é "nada respondeu".
        let mut bus = FakeBus::default();
        bus.queue.extend([0xFFFF; 5]);
        let err = read_register(&mut bus, controller(2), ReadReg::Style).unwrap_err();
        assert!(matches!(err, ReadError::Invalid { .. }), "{err:?}");
        assert!(!err.is_no_response());
        assert!(err.to_string().contains("leitura inválida (0xFF)"), "{err}");
        assert_eq!(bus.ops.len(), 5);
    }

    #[test]
    fn ram_resultado_das_leituras() {
        let read = |errors: &[Option<i32>]| {
            let mut bus = FakeBus::default();
            bus.errors.extend(errors.iter().copied());
            read_outcome(&read_controllers(&mut bus, &[module(2), module(3)]))
        };
        assert_eq!(read(&[]), ReadOutcome::AllOk);
        // A fila vale por transação: 5 ENXIO esgotam o 0x5A, os 5 seguintes o 0x5B.
        assert_eq!(read(&[Some(libc::ENXIO); 10]), ReadOutcome::NoResponse);
        assert_eq!(read(&ENXIO5), ReadOutcome::NoResponse);
        let mut mixed = ENXIO5.to_vec();
        mixed.extend([Some(libc::EIO); 5]);
        assert_eq!(read(&mixed), ReadOutcome::Failed);
    }

    /// `FakeBus` com o `0x0B` dos dois pentes em `0x0E`, como depois das séries
    /// de leituras: o plano não pode usar esse valor.
    fn bus_with_index_0e() -> FakeBus {
        let mut bus = FakeBus::default();
        bus.words.insert((0x5A, 0x0B), 0x0E00);
        bus.words.insert((0x5B, 0x0B), 0x0E00);
        bus
    }

    fn writes(bus: &FakeBus) -> Vec<(u16, u8, u8)> {
        bus.ops
            .iter()
            .filter_map(|op| match op {
                Op::Write(a, r, v) => Some((*a, *r, *v)),
                Op::Read(..) => None,
            })
            .collect()
    }

    fn pair() -> Vec<Controller> {
        vec![controller(2), controller(3)]
    }

    #[test]
    fn ram_indice_calculado() {
        assert_eq!(index_of(controller(2), &pair()), 1);
        assert_eq!(index_of(controller(3), &pair()), 0);
        let four: Vec<Controller> = (0..4).map(controller).collect();
        let idx: Vec<u8> = four.iter().map(|&c| index_of(c, &four)).collect();
        assert_eq!(idx, [3, 2, 1, 0]);
    }

    #[test]
    fn ram_apagar_sequencia_exata() {
        let mut bus = bus_with_index_0e();
        apply(&mut bus, &plan(RamCommand::Off, &pair())).unwrap();
        assert!(
            bus.ops.iter().all(|op| matches!(op, Op::Write(..))),
            "{:?}",
            bus.ops
        );
        assert_eq!(
            writes(&bus),
            [
                (0x5B, 0x08, 0x53),
                (0x5A, 0x08, 0x53),
                (0x5A, 0x0B, 0x01),
                (0x5A, 0x20, 0x00),
                (0x5B, 0x0B, 0x00),
                (0x5B, 0x20, 0x00),
                (0x5A, 0x08, 0x44),
                (0x5B, 0x08, 0x44),
            ]
        );
    }

    #[test]
    fn ram_reacender_e_padrao() {
        let mut bus = bus_with_index_0e();
        apply(&mut bus, &plan(RamCommand::On, &pair())).unwrap();
        let w = writes(&bus);
        assert_eq!(
            w[2..6],
            [
                (0x5A, 0x0B, 0x01),
                (0x5A, 0x20, 0x50),
                (0x5B, 0x0B, 0x00),
                (0x5B, 0x20, 0x50)
            ]
        );
        assert_eq!(w.len(), 8);

        let mut bus = bus_with_index_0e();
        apply(&mut bus, &plan(RamCommand::Default, &pair())).unwrap();
        let w = writes(&bus);
        let regs_5a: Vec<(u8, u8)> = w[2..11]
            .iter()
            .map(|&(a, r, v)| {
                assert_eq!(a, 0x5A);
                (r, v)
            })
            .collect();
        assert_eq!(
            regs_5a,
            [
                (0x09, 0x01),
                (0x0B, 0x01),
                (0x0D, 0x00),
                (0x0C, 0x01),
                (0x0E, 0x19),
                (0x20, 0x50),
                (0x10, 0x00),
                (0x11, 0x00),
                (0x1E, 0x00),
            ]
        );
        assert_eq!(w[12], (0x5B, 0x0B, 0x00));
        assert_eq!(w.len(), 2 + 18 + 2);
        assert!(bus.ops.iter().all(|op| matches!(op, Op::Write(..))));
    }

    #[test]
    fn ram_primeiro_pente_sem_resposta_nao_escreve() {
        let mut bus = FakeBus::default();
        bus.errors.extend([Some(libc::ENXIO); 30]);
        let err = apply(&mut bus, &plan(RamCommand::Off, &pair())).unwrap_err();
        assert!(
            matches!(err, ApplyError::NoResponse { controller } if controller.address() == 0x5B),
            "{err:?}"
        );
        // Só as 30 tentativas da primeira escrita.
        assert_eq!(writes(&bus), [(0x5B, 0x08, 0x53); 30]);
        assert_eq!(err.to_string(), "nada respondeu em 0x5b; nada foi escrito");
    }

    #[test]
    fn ram_escrita_que_falha_para_a_moldura() {
        let mut bus = FakeBus::default();
        // Os 2 inícios e o índice do 0x5A dão certo; o brilho do 0x5A falha sempre.
        bus.errors.extend([None; 3]);
        bus.errors.extend([Some(libc::EIO); 30]);
        let err = apply(&mut bus, &plan(RamCommand::Off, &pair())).unwrap_err();
        let ApplyError::Write {
            controller, reg, ..
        } = &err
        else {
            panic!("{err:?}");
        };
        assert_eq!((controller.address(), reg.code()), (0x5A, 0x20));
        let w = writes(&bus);
        assert_eq!(w.len(), 3 + 30);
        assert!(w[3..].iter().all(|&x| x == (0x5A, 0x20, 0x00)));
        let msg = err.to_string();
        assert!(msg.contains("0x5a") && msg.contains("0x20"), "{msg}");
        assert!(
            msg.contains("moldura ficou incompleta") && msg.contains("Repita"),
            "{msg}"
        );

        // ENXIO depois de uma escrita aceita não é "nada foi escrito".
        let mut bus = FakeBus::default();
        bus.errors.push_back(None);
        bus.errors.extend([Some(libc::ENXIO); 30]);
        let err = apply(&mut bus, &plan(RamCommand::Off, &pair())).unwrap_err();
        assert!(matches!(err, ApplyError::Write { .. }), "{err:?}");
    }

    #[test]
    fn ram_falha_passageira_segue() {
        let mut bus = FakeBus::default();
        bus.errors.extend([None; 2]);
        bus.errors.extend([Some(libc::EIO), Some(libc::EIO)]);
        apply(&mut bus, &plan(RamCommand::Off, &pair())).unwrap();
        let w = writes(&bus);
        assert_eq!(w.len(), 8 + 2);
        assert_eq!(w[2..5], [(0x5A, 0x0B, 0x01); 3]);
        assert_eq!(w.last(), Some(&(0x5B, 0x08, 0x44)));
    }

    #[test]
    fn ram_pausas_da_moldura() {
        let mut bus = FakeBus::default();
        apply(&mut bus, &plan(RamCommand::Off, &pair())).unwrap();
        let gaps: Vec<Duration> = bus.at.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(gaps.iter().all(|g| *g >= TRANSACTION_PAUSE), "{gaps:?}");
        // ops: 2 inícios, [pausa de 50 ms], 4 escritas, 2 fins.
        assert!(gaps[1] >= FRAME_PAUSE, "{gaps:?}");
    }

    #[test]
    fn ram_plano_do_dry_run() {
        let lines = dry_run_lines(RamCommand::Off);
        assert_eq!(lines[0], "0x5b  escreve 0x08 = 0x53");
        assert_eq!(lines[2], "pausa de 50 ms");
        assert_eq!(lines[3], "0x5a  escreve 0x0B = 0x01");
        assert_eq!(lines[5], "0x5b  escreve 0x0B = 0x00");
        assert_eq!(lines.last().unwrap(), "pausa de 50 ms");
        assert!(lines.iter().all(|l| !l.contains("lê")), "{lines:?}");
        assert_eq!(RamCommand::from_name("purple"), None);
        assert_eq!(RamCommand::from_name("default"), Some(RamCommand::Default));
    }

    #[test]
    fn ram_controlador_em_arquivo_comum() {
        let dir = tempfile::tempdir().unwrap();
        let node = dir.path().join("i2c-0");
        fs::write(&node, "").unwrap();
        let Err(OpenError::Other(_, e)) = I2cDev::open(&node, &[controller(2)]) else {
            panic!("esperado erro do ioctl");
        };
        assert_eq!(e.raw_os_error(), Some(libc::ENOTTY));
    }

    #[test]
    fn ram_controlador_ocupado() {
        let dir = tempfile::tempdir().unwrap();
        let node = dir.path().join("i2c-0");
        fs::write(&node, "").unwrap();
        let holder = File::open(&node).unwrap();
        assert!(lock_exclusive(&holder, Duration::ZERO).unwrap());
        let second = I2cDev::open_with_timeout(&node, &[controller(2)], Duration::from_millis(100));
        assert!(matches!(second, Err(OpenError::Busy(ref p)) if *p == node));
    }

    #[test]
    fn ram_controlador_sem_permissao() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let node = dir.path().join("i2c-0");
        fs::write(&node, "").unwrap();
        fs::set_permissions(&node, fs::Permissions::from_mode(0o000)).unwrap();
        if OpenOptions::new()
            .read(true)
            .write(true)
            .open(&node)
            .is_ok()
        {
            eprintln!("aviso: o modo 000 não bloqueia a abertura (root?); teste pulado");
            return;
        }
        assert!(matches!(
            I2cDev::open(&node, &[controller(2)]),
            Err(OpenError::PermissionDenied(_))
        ));
    }
}
