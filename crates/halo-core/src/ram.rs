// SPDX-License-Identifier: GPL-2.0-or-later

//! SPD dos pentes DDR4 no SMBus da placa, só leitura pelo sysfs.
//!
//! O halo não fala com o barramento: só lê o arquivo `eeprom` que o driver
//! `ee1004` do kernel publica. A troca de página do SPD (send byte em
//! `0x36`/`0x37`) é feita pelo próprio kernel, como no boot. Nenhum
//! `/dev/i2c-*` é aberto.
//!
//! O critério de suporte ao RGB vem do `FuryCTRL.dll` 2.0.66.0, analisado com
//! o REA (Ghidra 12.1.4) em
//! `tools/samples/kingston-fury-ctrl/analysis/decomp-FuryCTRL/functions/`:
//! - `FUN_1000af50` (`0x1000af50.c`): lê os bytes `0x14A`, `0x167` e `0x168`
//!   do SPD e aceita `0x10`, `0x20`, `0x22` e `0x23` no último;
//! - `CheckDRAMSupport` (`0x1000f9a0.c`): exige a Kingston como fabricante e
//!   dá o nome por marca e modelo.
//!
//! Ver `docs/hardware.md`, seções "Software oficial: FURY CTRL 2.0.66.0" e
//! "Primeiro boot com o lax: leitura do SPD".

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

/// Tamanho do SPD de um DDR4 (duas páginas de 256 bytes).
pub const SPD_SIZE: usize = 512;
/// Byte 2 do SPD: tipo de memória.
pub const BYTE_TYPE: usize = 2;
/// Valor do byte 2 para DDR4.
pub const TYPE_DDR4: u8 = 0x0C;
/// Bytes 320–321: fabricante do módulo (JEDEC JEP-106).
pub const BYTES_MANUFACTURER: usize = 320;
/// Kingston: banco 2 com paridade (`0x01`) e código `0x98`.
pub const KINGSTON: [u8; 2] = [0x01, 0x98];
/// Bytes 329–348: part number do módulo.
pub const PART_NUMBER: std::ops::RangeInclusive<usize> = 329..=348;
/// Byte 330 (`0x14A`): marca, o 2º caractere do part number.
pub const BYTE_BRAND: usize = 330;
/// Byte 359 (`0x167`): modelo.
pub const BYTE_MODEL: usize = 359;
/// Byte 360 (`0x168`): tipo do RGB aceito pelo FURY CTRL.
pub const BYTE_RGB: usize = 360;
/// Valores do byte 360 que o `FUN_1000af50` aceita.
pub const RGB_ACCEPTED: [u8; 4] = [0x10, 0x20, 0x22, 0x23];
/// Maior modelo com nome no `CheckDRAMSupport`.
pub const MODEL_MAX: u8 = 4;

/// Marca do byte 330.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Brand {
    /// `0x48` (`H`): HyperX, part number `HX…`.
    HyperX,
    /// `0x46` (`F`): Fury, part number `KF…`.
    Fury,
}

impl Brand {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            b'H' => Some(Self::HyperX),
            b'F' => Some(Self::Fury),
            _ => None,
        }
    }

    /// Letra do byte 330.
    pub fn letter(self) -> char {
        match self {
            Self::HyperX => 'H',
            Self::Fury => 'F',
        }
    }
}

/// Nome do modelo pela tabela do `CheckDRAMSupport`.
pub fn model_name(brand: Brand, model: u8) -> Option<&'static str> {
    match (brand, model) {
        (Brand::HyperX, 0) => Some("HyperX Predator"),
        (Brand::HyperX, 1) => Some("HyperX Fury"),
        (Brand::Fury, 0) => Some("Fury Renegade"),
        (Brand::Fury, 1) => Some("Fury Beast"),
        (Brand::Fury, 2) => Some("Fury Beast SE"),
        (Brand::Fury, 3) => Some("Fury Renegade Refresh"),
        (Brand::Fury, 4) => Some("Fury Beast Refresh"),
        _ => None,
    }
}

/// Byte que reprovou o pente no critério do FURY CTRL, com o valor lido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    /// Byte 330 diferente de `F` e de `H`.
    Brand(u8),
    /// Byte 359 acima de 4.
    Model(u8),
    /// Byte 360 fora de `0x10`, `0x20`, `0x22` e `0x23`.
    Rgb(u8),
}

impl Unsupported {
    /// Posição do byte no SPD.
    pub fn byte(self) -> usize {
        match self {
            Self::Brand(_) => BYTE_BRAND,
            Self::Model(_) => BYTE_MODEL,
            Self::Rgb(_) => BYTE_RGB,
        }
    }

    pub fn value(self) -> u8 {
        match self {
            Self::Brand(v) | Self::Model(v) | Self::Rgb(v) => v,
        }
    }
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let expected = match self {
            Self::Brand(_) => "0x46 (F) ou 0x48 (H)",
            Self::Model(_) => "de 0x00 a 0x04",
            Self::Rgb(_) => "0x10, 0x20, 0x22 ou 0x23",
        };
        write!(
            f,
            "byte {} = 0x{:02X}, esperado {expected}",
            self.byte(),
            self.value()
        )
    }
}

/// Veredito de RGB pelo critério do FURY CTRL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Byte 2 diferente de `0x0C`: sem veredito.
    NotDdr4,
    /// Fabricante diferente de `01 98`: sem veredito.
    NotKingston,
    /// Marca `F`/`H`, modelo de 0 a 4 e byte 360 aceito. `name` é `None` quando
    /// a combinação não tem nome na tabela (por exemplo, `H` com modelo 3).
    Supported {
        brand: Brand,
        model: u8,
        name: Option<&'static str>,
    },
    NotSupported {
        reason: Unsupported,
    },
}

/// Campos lidos de um SPD de 512 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpdInfo {
    /// Byte 2.
    pub memory_type: u8,
    /// Bytes 320–321.
    pub manufacturer: [u8; 2],
    /// Bytes 329–348, sem os espaços e nulos do fim.
    pub part_number: String,
    /// Byte 360, mostrado no veredito.
    pub rgb_type: u8,
    pub verdict: Verdict,
}

impl SpdInfo {
    pub fn is_ddr4(&self) -> bool {
        self.memory_type == TYPE_DDR4
    }

    pub fn is_kingston(&self) -> bool {
        self.manufacturer == KINGSTON
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// O SPD não tem 512 bytes; leva o tamanho lido.
    WrongSize(usize),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSize(n) => write!(f, "SPD com {n} bytes, esperado {SPD_SIZE}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Decodifica um SPD de DDR4 e aplica o critério do FURY CTRL.
pub fn decode(spd: &[u8]) -> Result<SpdInfo, DecodeError> {
    if spd.len() != SPD_SIZE {
        return Err(DecodeError::WrongSize(spd.len()));
    }
    let memory_type = spd[BYTE_TYPE];
    let manufacturer = [spd[BYTES_MANUFACTURER], spd[BYTES_MANUFACTURER + 1]];
    let part_number = String::from_utf8_lossy(&spd[PART_NUMBER])
        .trim_end_matches([' ', '\0'])
        .to_owned();
    let rgb_type = spd[BYTE_RGB];
    let verdict = if memory_type != TYPE_DDR4 {
        Verdict::NotDdr4
    } else if manufacturer != KINGSTON {
        Verdict::NotKingston
    } else {
        fury_ctrl_verdict(spd[BYTE_BRAND], spd[BYTE_MODEL], rgb_type)
    };
    Ok(SpdInfo {
        memory_type,
        manufacturer,
        part_number,
        rgb_type,
        verdict,
    })
}

/// Critério do `FUN_1000af50`, na ordem em que os bytes são lidos.
fn fury_ctrl_verdict(brand: u8, model: u8, rgb: u8) -> Verdict {
    let Some(brand) = Brand::from_byte(brand) else {
        return Verdict::NotSupported {
            reason: Unsupported::Brand(brand),
        };
    };
    if model > MODEL_MAX {
        return Verdict::NotSupported {
            reason: Unsupported::Model(model),
        };
    }
    if !RGB_ACCEPTED.contains(&rgb) {
        return Verdict::NotSupported {
            reason: Unsupported::Rgb(rgb),
        };
    }
    Verdict::Supported {
        brand,
        model,
        name: model_name(brand, model),
    }
}

/// Prefixo do nome dos adaptadores do `i2c-piix4` (`piix4_add_adapter`).
pub const ADAPTER_PREFIX: &str = "SMBus PIIX4 adapter";
/// Nome das instâncias do driver de SPD do DDR4.
pub const EE1004_NAME: &str = "ee1004";

/// Resultado da leitura do `eeprom` de uma instância.
#[derive(Debug)]
pub enum SpdRead {
    Bytes(Vec<u8>),
    /// `ENXIO`: o pente não respondeu (como a `2-0051` no barramento ASF).
    NoResponse,
    /// `EACCES`.
    PermissionDenied,
    Failed(io::Error),
}

impl SpdRead {
    pub fn from_io_error(err: io::Error) -> Self {
        if err.raw_os_error() == Some(libc::ENXIO) {
            Self::NoResponse
        } else if err.kind() == io::ErrorKind::PermissionDenied {
            Self::PermissionDenied
        } else {
            Self::Failed(err)
        }
    }

    fn of(result: io::Result<Vec<u8>>) -> Self {
        match result {
            Ok(bytes) => Self::Bytes(bytes),
            Err(e) => Self::from_io_error(e),
        }
    }
}

/// Uma instância do `ee1004` num adaptador PIIX4.
#[derive(Debug)]
pub struct SpdSlot {
    /// `name` do adaptador, por exemplo `SMBus PIIX4 adapter port 0 at 0b00`.
    pub adapter: String,
    /// Nome da instância, `<barramento>-<endereço>`, por exemplo `0-0052`.
    pub instance: String,
    /// Endereço de 7 bits do SPD (`0x50`–`0x57`).
    pub address: u8,
    pub read: SpdRead,
}

impl SpdSlot {
    /// Índice do slot de SPD (endereço − `0x50`).
    pub fn slot(&self) -> u8 {
        self.address.wrapping_sub(0x50)
    }
}

#[derive(Debug)]
pub enum FindError {
    /// Nenhum adaptador cujo nome começa com `SMBus PIIX4 adapter`.
    NoAdapter,
    /// Os adaptadores existem, mas nenhum tem instância do `ee1004`.
    NoEe1004 { adapters: Vec<String> },
    /// Falha ao listar o sysfs.
    Io(io::Error),
}

impl fmt::Display for FindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter => write!(f, "nenhum adaptador \"{ADAPTER_PREFIX}\" no sysfs"),
            Self::NoEe1004 { adapters } => {
                write!(f, "nenhuma instância do ee1004 em {}", adapters.join(", "))
            }
            Self::Io(e) => write!(f, "falha ao ler o sysfs: {e}"),
        }
    }
}

impl std::error::Error for FindError {}

fn read_name(dir: &Path) -> Option<String> {
    fs::read_to_string(dir.join("name"))
        .ok()
        .map(|s| s.trim_end().to_owned())
}

/// Endereço de um nome de instância `<n>-00XX`.
fn parse_instance(name: &str) -> Option<u8> {
    let (bus, addr) = name.split_once('-')?;
    if bus.is_empty() || !bus.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if addr.len() != 4 || !addr.starts_with("00") {
        return None;
    }
    u8::from_str_radix(&addr[2..], 16).ok()
}

/// Número do adaptador `i2c-N`, para ordenar.
fn adapter_number(name: &str) -> Option<u32> {
    name.strip_prefix("i2c-")?.parse().ok()
}

/// Lista as instâncias do `ee1004` nos adaptadores PIIX4 e lê o `eeprom` de
/// cada uma. Só lê arquivos sob `sys_root`.
pub fn find(sys_root: &Path) -> Result<Vec<SpdSlot>, FindError> {
    let devices = sys_root.join("bus/i2c/devices");
    let entries = match fs::read_dir(&devices) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(FindError::NoAdapter),
        Err(e) => return Err(FindError::Io(e)),
    };
    let mut adapters: Vec<(u32, String, std::path::PathBuf)> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(FindError::Io)?;
        let file_name = entry.file_name();
        let Some(number) = file_name.to_str().and_then(adapter_number) else {
            continue;
        };
        let path = entry.path();
        let Some(name) = read_name(&path) else {
            continue;
        };
        if name.starts_with(ADAPTER_PREFIX) {
            adapters.push((number, name, path));
        }
    }
    if adapters.is_empty() {
        return Err(FindError::NoAdapter);
    }
    adapters.sort_by_key(|(n, _, _)| *n);

    let mut slots = Vec::new();
    for (_, adapter, path) in &adapters {
        let mut children: Vec<(String, u8, std::path::PathBuf)> = Vec::new();
        for entry in fs::read_dir(path).map_err(FindError::Io)? {
            let entry = entry.map_err(FindError::Io)?;
            let Some(instance) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(address) = parse_instance(&instance) else {
                continue;
            };
            let dir = entry.path();
            let eeprom = dir.join("eeprom");
            if read_name(&dir).as_deref() != Some(EE1004_NAME) || !eeprom.exists() {
                continue;
            }
            children.push((instance, address, eeprom));
        }
        children.sort_by(|a, b| a.0.cmp(&b.0));
        for (instance, address, eeprom) in children {
            slots.push(SpdSlot {
                adapter: adapter.clone(),
                instance,
                address,
                read: SpdRead::of(fs::read(&eeprom)),
            });
        }
    }
    if slots.is_empty() {
        return Err(FindError::NoEe1004 {
            adapters: adapters.into_iter().map(|(_, name, _)| name).collect(),
        });
    }
    Ok(slots)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SPD de 512 bytes com os bytes lidos do `0-0052` em 2026-10-31 e o
    /// resto zerado.
    fn spd_desta_maquina() -> Vec<u8> {
        let mut spd = vec![0u8; SPD_SIZE];
        spd[BYTE_TYPE] = 0x0C;
        spd[320] = 0x01;
        spd[321] = 0x98;
        let part = b"KF3200C16D4/16GX    ";
        spd[PART_NUMBER].copy_from_slice(part);
        assert_eq!(spd[BYTE_BRAND], 0x46);
        spd[BYTE_MODEL] = 0x04;
        spd[BYTE_RGB] = 0x23;
        spd[364] = 0x43;
        spd
    }

    fn with(changes: &[(usize, u8)]) -> Vec<u8> {
        let mut spd = spd_desta_maquina();
        for &(i, v) in changes {
            spd[i] = v;
        }
        spd
    }

    #[test]
    fn ram_pente_desta_maquina() {
        let info = decode(&spd_desta_maquina()).unwrap();
        assert!(info.is_ddr4());
        assert!(info.is_kingston());
        assert_eq!(info.part_number, "KF3200C16D4/16GX");
        assert_eq!(
            info.verdict,
            Verdict::Supported {
                brand: Brand::Fury,
                model: 4,
                name: Some("Fury Beast Refresh"),
            }
        );
    }

    #[test]
    fn ram_byte_360_fora_da_lista() {
        let info = decode(&with(&[(BYTE_MODEL, 1), (BYTE_RGB, 0x00)])).unwrap();
        let Verdict::NotSupported { reason } = info.verdict else {
            panic!("{:?}", info.verdict);
        };
        assert_eq!(reason, Unsupported::Rgb(0x00));
        assert_eq!(reason.byte(), 360);
        assert!(reason.to_string().contains("byte 360 = 0x00"), "{reason}");
    }

    #[test]
    fn ram_hyperx_sem_nome() {
        let info = decode(&with(&[
            (BYTE_BRAND, b'H'),
            (BYTE_MODEL, 3),
            (BYTE_RGB, 0x10),
        ]))
        .unwrap();
        assert_eq!(
            info.verdict,
            Verdict::Supported {
                brand: Brand::HyperX,
                model: 3,
                name: None,
            }
        );
    }

    #[test]
    fn ram_marca_desconhecida() {
        let info = decode(&with(&[(BYTE_BRAND, 0x41)])).unwrap();
        let Verdict::NotSupported { reason } = info.verdict else {
            panic!("{:?}", info.verdict);
        };
        assert_eq!(reason, Unsupported::Brand(0x41));
        assert!(reason.to_string().contains("byte 330 = 0x41"), "{reason}");
    }

    #[test]
    fn ram_modelo_acima_de_4() {
        let info = decode(&with(&[(BYTE_MODEL, 0x05)])).unwrap();
        let Verdict::NotSupported { reason } = info.verdict else {
            panic!("{:?}", info.verdict);
        };
        assert_eq!(reason, Unsupported::Model(0x05));
        assert!(reason.to_string().contains("byte 359 = 0x05"), "{reason}");
    }

    #[test]
    fn ram_outro_fabricante() {
        let info = decode(&with(&[(320, 0x80), (321, 0x2C)])).unwrap();
        assert_eq!(info.verdict, Verdict::NotKingston);
        assert_eq!(info.manufacturer, [0x80, 0x2C]);
    }

    #[test]
    fn ram_nao_e_ddr4() {
        let info = decode(&with(&[(BYTE_TYPE, 0x0B)])).unwrap();
        assert_eq!(info.verdict, Verdict::NotDdr4);
        assert_eq!(info.memory_type, 0x0B);
    }

    #[test]
    fn ram_spd_curto() {
        let err = decode(&[0u8; 256]).unwrap_err();
        assert_eq!(err, DecodeError::WrongSize(256));
        assert!(err.to_string().contains("256 bytes"), "{err}");
    }

    #[test]
    fn ram_part_number_sem_nulos() {
        let mut spd = spd_desta_maquina();
        spd[PART_NUMBER].copy_from_slice(b"KF3200C16D4/16GX\0\0\0\0");
        assert_eq!(decode(&spd).unwrap().part_number, "KF3200C16D4/16GX");
    }

    #[test]
    fn ram_nomes_dos_modelos() {
        assert_eq!(model_name(Brand::HyperX, 0), Some("HyperX Predator"));
        assert_eq!(model_name(Brand::HyperX, 1), Some("HyperX Fury"));
        assert_eq!(model_name(Brand::HyperX, 2), None);
        assert_eq!(model_name(Brand::Fury, 0), Some("Fury Renegade"));
        assert_eq!(model_name(Brand::Fury, 1), Some("Fury Beast"));
        assert_eq!(model_name(Brand::Fury, 2), Some("Fury Beast SE"));
        assert_eq!(model_name(Brand::Fury, 3), Some("Fury Renegade Refresh"));
        assert_eq!(model_name(Brand::Fury, 5), None);
    }

    // Busca no sysfs.

    fn adapter(sys: &Path, dir: &str, name: &str) -> std::path::PathBuf {
        let path = sys.join("bus/i2c/devices").join(dir);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("name"), format!("{name}\n")).unwrap();
        path
    }

    fn client(adapter: &Path, instance: &str, name: &str, eeprom: Option<&[u8]>) {
        let dir = adapter.join(instance);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("name"), format!("{name}\n")).unwrap();
        if let Some(bytes) = eeprom {
            fs::write(dir.join("eeprom"), bytes).unwrap();
        }
    }

    #[test]
    fn ram_acha_pelo_nome_do_adaptador() {
        let sys = tempfile::tempdir().unwrap();
        let spd = spd_desta_maquina();
        let a = adapter(sys.path(), "i2c-7", "SMBus PIIX4 adapter port 0 at 0b00");
        client(&a, "7-0053", "ee1004", Some(&spd));
        client(&a, "7-0052", "ee1004", Some(&spd));
        client(&a, "7-0036", "dummy", None);
        let slots = find(sys.path()).unwrap();
        let names: Vec<&str> = slots.iter().map(|s| s.instance.as_str()).collect();
        assert_eq!(names, ["7-0052", "7-0053"]);
        assert_eq!(slots[0].address, 0x52);
        assert_eq!(slots[0].slot(), 2);
        assert_eq!(slots[0].adapter, "SMBus PIIX4 adapter port 0 at 0b00");
        assert!(matches!(&slots[1].read, SpdRead::Bytes(b) if b.len() == SPD_SIZE));
    }

    #[test]
    fn ram_ignora_adaptador_da_gpu() {
        let sys = tempfile::tempdir().unwrap();
        let spd = spd_desta_maquina();
        let gpu = adapter(sys.path(), "i2c-3", "AMDGPU SMU 0");
        client(&gpu, "3-0050", "ee1004", Some(&spd));
        let a = adapter(sys.path(), "i2c-0", "SMBus PIIX4 adapter port 0 at 0b00");
        client(&a, "0-0052", "ee1004", Some(&spd));
        let slots = find(sys.path()).unwrap();
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].instance, "0-0052");
    }

    #[test]
    fn ram_sem_adaptador_piix4() {
        let sys = tempfile::tempdir().unwrap();
        let gpu = adapter(sys.path(), "i2c-3", "AMDGPU SMU 0");
        client(&gpu, "3-0050", "ee1004", Some(&spd_desta_maquina()));
        assert!(matches!(find(sys.path()), Err(FindError::NoAdapter)));
        let vazio = tempfile::tempdir().unwrap();
        assert!(matches!(find(vazio.path()), Err(FindError::NoAdapter)));
    }

    #[test]
    fn ram_adaptador_sem_ee1004() {
        let sys = tempfile::tempdir().unwrap();
        adapter(sys.path(), "i2c-0", "SMBus PIIX4 adapter port 0 at 0b00");
        let Err(FindError::NoEe1004 { adapters }) = find(sys.path()) else {
            panic!("esperado NoEe1004");
        };
        assert_eq!(adapters, ["SMBus PIIX4 adapter port 0 at 0b00"]);
    }

    #[test]
    fn ram_ignora_filho_com_outro_nome() {
        let sys = tempfile::tempdir().unwrap();
        let a = adapter(sys.path(), "i2c-0", "SMBus PIIX4 adapter port 0 at 0b00");
        client(&a, "0-0050", "spd", Some(&spd_desta_maquina()));
        client(&a, "0-0052", "ee1004", Some(&spd_desta_maquina()));
        let slots = find(sys.path()).unwrap();
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].instance, "0-0052");
    }

    #[test]
    fn ram_adaptador_so_com_filho_de_outro_nome() {
        let sys = tempfile::tempdir().unwrap();
        let a = adapter(sys.path(), "i2c-0", "SMBus PIIX4 adapter port 0 at 0b00");
        client(&a, "0-0050", "spd", Some(&spd_desta_maquina()));
        assert!(matches!(find(sys.path()), Err(FindError::NoEe1004 { .. })));
    }

    #[test]
    fn ram_erros_de_leitura() {
        let enxio = io::Error::from_raw_os_error(libc::ENXIO);
        assert!(matches!(SpdRead::from_io_error(enxio), SpdRead::NoResponse));
        let eacces = io::Error::from_raw_os_error(libc::EACCES);
        assert!(matches!(
            SpdRead::from_io_error(eacces),
            SpdRead::PermissionDenied
        ));
        let eio = io::Error::from_raw_os_error(libc::EIO);
        assert!(matches!(SpdRead::from_io_error(eio), SpdRead::Failed(_)));
    }

    #[test]
    fn ram_nomes_de_instancia() {
        assert_eq!(parse_instance("0-0052"), Some(0x52));
        assert_eq!(parse_instance("12-0050"), Some(0x50));
        assert_eq!(parse_instance("i2c-dev"), None);
        assert_eq!(parse_instance("0-52"), None);
        assert_eq!(parse_instance("-0052"), None);
    }
}
