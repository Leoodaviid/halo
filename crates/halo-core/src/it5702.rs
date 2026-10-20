// SPDX-License-Identifier: GPL-2.0-or-later

//! Controlador RGB ITE IT5702 da placa-mãe (`048d:5702`), só leitura.
//!
//! Protocolo portado do driver `GigabyteRGBFusion2USBController` do OpenRGB
//! (`Controllers/GigabyteRGBFusion2USBController/`, commit `72262b8`):
//! `RefreshHardwareInfo()`, `SendCCReport()`/`SendReport()` e a struct
//! `IT8297Report`.

use std::fmt;
use std::io;
use std::path::Path;

use crate::hid::{DeviceMatch, HidDeviceInfo, Hidraw, OpenError, find_devices};

pub const VENDOR_ID: u16 = 0x048D;
pub const PRODUCT_ID: u16 = 0x5702;
/// Usage page de fabricante do descritor HID.
pub const USAGE_PAGE: u16 = 0xFF89;

pub const DEVICE: DeviceMatch = DeviceMatch {
    vendor: VENDOR_ID,
    product: PRODUCT_ID,
    usage_page: USAGE_PAGE,
};

/// Report ID por onde vão os comandos (`report_id = 0xCC` no OpenRGB).
pub const REPORT_ID: u8 = 0xCC;
/// `FUSION2_USB_BUFFER_SIZE`.
pub const PACKET_SIZE: usize = 64;
/// Comando que pede a configuração do controlador.
pub const CMD_READ_CONFIG: u8 = 0x60;

/// Pacote `CC 60` completado com zeros, como `SendCCReport(0x60, 0x00)`.
pub fn read_config_request() -> [u8; PACKET_SIZE] {
    let mut packet = [0u8; PACKET_SIZE];
    packet[0] = REPORT_ID;
    packet[1] = CMD_READ_CONFIG;
    packet
}

/// Limite de LEDs de uma fita ARGB (`enum LEDCount`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedCount {
    /// Valor de 4 bits recebido.
    pub raw: u8,
    /// Limite correspondente: 32, 64, 256, 512 ou 1024.
    pub limit: u16,
    /// Falso quando `raw` está fora do enum e o limite caiu no padrão 1024,
    /// como em `LedCountFromEnum()`.
    pub known: bool,
}

impl LedCount {
    pub fn from_nibble(raw: u8) -> Self {
        let raw = raw & 0x0F;
        let (limit, known) = match raw {
            0 => (32, true),
            1 => (64, true),
            2 => (256, true),
            3 => (512, true),
            4 => (1024, true),
            _ => (1024, false),
        };
        Self { raw, limit, known }
    }
}

/// Resposta decodificada do `CC 60` (`struct IT8297Report`, little-endian).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct It5702Report {
    pub report_id: u8,
    pub product: u8,
    pub device_num: u8,
    pub strip_detect: u8,
    pub fw_ver: u32,
    /// `argb01_count`, `argb23_count` e `argb45_count`.
    pub argb_counts: [u8; 3],
    pub support_cmd_flag: u8,
    /// `str_product`, cortado no primeiro byte nulo.
    pub product_name: String,
    pub cal_spare0: u32,
    pub cal_strip0: u32,
    pub cal_strip1: u32,
    pub rgb_cali: u32,
    pub chip_id: u32,
    pub cal_spare1: u32,
    pub raw: [u8; PACKET_SIZE],
}

impl It5702Report {
    /// Versão no formato `a.b.c.d`, do byte menos ao mais significativo.
    pub fn firmware_version(&self) -> String {
        let [a, b, c, d] = self.fw_ver.to_le_bytes();
        format!("{a}.{b}.{c}.{d}")
    }

    /// Seis fitas ARGB, nibble baixo de cada byte primeiro.
    pub fn led_counts(&self) -> [LedCount; 6] {
        let mut counts = [LedCount::from_nibble(0); 6];
        for (i, byte) in self.argb_counts.iter().enumerate() {
            counts[2 * i] = LedCount::from_nibble(byte & 0x0F);
            counts[2 * i + 1] = LedCount::from_nibble(byte >> 4);
        }
        counts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// A resposta tem menos de 64 bytes.
    ShortResponse(usize),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShortResponse(n) => {
                write!(f, "resposta curta: {n} bytes, esperado {PACKET_SIZE}")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Decodifica a resposta do `CC 60`. Não faz I/O.
pub fn decode(data: &[u8]) -> Result<It5702Report, DecodeError> {
    let Some(raw) = data.get(..PACKET_SIZE) else {
        return Err(DecodeError::ShortResponse(data.len()));
    };
    let raw: [u8; PACKET_SIZE] = raw.try_into().expect("fatia de 64 bytes");
    let u32_at =
        |offset: usize| u32::from_le_bytes(raw[offset..offset + 4].try_into().expect("4 bytes"));

    let name = &raw[12..40];
    let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(name.len())];

    Ok(It5702Report {
        report_id: raw[0],
        product: raw[1],
        device_num: raw[2],
        strip_detect: raw[3],
        fw_ver: u32_at(4),
        argb_counts: [raw[8], raw[9], raw[10]],
        support_cmd_flag: raw[11],
        product_name: String::from_utf8_lossy(name).into_owned(),
        cal_spare0: u32_at(40),
        cal_strip0: u32_at(44),
        cal_strip1: u32_at(48),
        rgb_cali: u32_at(52),
        chip_id: u32_at(56),
        cal_spare1: u32_at(60),
        raw,
    })
}

#[derive(Debug)]
pub enum ProbeError {
    /// Nenhum hidraw com VID:PID e usage page do IT5702.
    NotFound,
    Open(OpenError),
    Io(io::Error),
    Decode(DecodeError),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(
                f,
                "controlador IT5702 não encontrado ({VENDOR_ID:04x}:{PRODUCT_ID:04x}, usage page 0x{USAGE_PAGE:04X})"
            ),
            Self::Open(e) => write!(f, "{e}"),
            Self::Io(e) => write!(f, "erro de comunicação: {e}"),
            Self::Decode(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ProbeError {}

/// Primeiro IT5702 encontrado sob `sysfs_root`.
pub fn find(sysfs_root: &Path) -> Result<HidDeviceInfo, ProbeError> {
    find_devices(sysfs_root, DEVICE)
        .map_err(ProbeError::Io)?
        .into_iter()
        .next()
        .ok_or(ProbeError::NotFound)
}

/// Lê a configuração do controlador. Envia só `CC 60` e depois pede o
/// feature report `0xCC`. Nenhum outro comando é enviado.
pub fn read_config(device: &Hidraw) -> Result<It5702Report, ProbeError> {
    device
        .send_feature_report(&read_config_request())
        .map_err(ProbeError::Io)?;

    let mut buf = [0u8; PACKET_SIZE];
    buf[0] = REPORT_ID;
    let n = device
        .get_feature_report(&mut buf)
        .map_err(ProbeError::Io)?;

    decode(&buf[..n.min(PACKET_SIZE)]).map_err(ProbeError::Decode)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> [u8; PACKET_SIZE] {
        let mut r = [0u8; PACKET_SIZE];
        r[0] = 0xCC;
        r[1] = 0x01; // product
        r[2] = 0x00; // device_num
        r[3] = 0x03; // strip_detect
        r[4..8].copy_from_slice(&[1, 2, 3, 4]); // fw_ver
        r[8] = 0x10; // fitas 0/1: 32 e 64
        r[9] = 0x32; // fitas 2/3: 256 e 512
        r[10] = 0xF4; // fitas 4/5: 1024 e desconhecido
        r[11] = 0x02; // support_cmd_flag
        r[12..20].copy_from_slice(b"IT5702-G");
        r[40..44].copy_from_slice(&0x1111_1111u32.to_le_bytes());
        r[44..48].copy_from_slice(&0x0002_0100u32.to_le_bytes());
        r[48..52].copy_from_slice(&0x0001_0002u32.to_le_bytes());
        r[52..56].copy_from_slice(&0x0000_0102u32.to_le_bytes());
        r[56..60].copy_from_slice(&0x0057_0200u32.to_le_bytes());
        r[60..64].copy_from_slice(&0x2222_2222u32.to_le_bytes());
        r
    }

    #[test]
    fn pedido_cc60_tem_64_bytes_exatos() {
        let p = read_config_request();
        assert_eq!(p.len(), 64);
        assert_eq!(&p[..2], &[0xCC, 0x60]);
        assert!(p[2..].iter().all(|&b| b == 0));
    }

    #[test]
    fn decodifica_campos() {
        let r = decode(&sample()).unwrap();
        assert_eq!(r.report_id, 0xCC);
        assert_eq!(r.product, 0x01);
        assert_eq!(r.device_num, 0x00);
        assert_eq!(r.strip_detect, 0x03);
        assert_eq!(r.support_cmd_flag, 0x02);
        assert_eq!(r.product_name, "IT5702-G");
        assert_eq!(r.cal_spare0, 0x1111_1111);
        assert_eq!(r.cal_strip0, 0x0002_0100);
        assert_eq!(r.cal_strip1, 0x0001_0002);
        assert_eq!(r.rgb_cali, 0x0000_0102);
        assert_eq!(r.cal_spare1, 0x2222_2222);
        assert_eq!(r.raw, sample());
    }

    #[test]
    fn firmware_e_chip_id() {
        let r = decode(&sample()).unwrap();
        assert_eq!(r.firmware_version(), "1.2.3.4");
        assert_eq!(format!("0x{:08X}", r.chip_id), "0x00570200");
    }

    #[test]
    fn nome_sem_nulo_usa_os_28_bytes() {
        let mut s = sample();
        s[12..40].copy_from_slice(&[b'A'; 28]);
        assert_eq!(decode(&s).unwrap().product_name, "A".repeat(28));
    }

    #[test]
    fn contagens_de_led() {
        let counts = decode(&sample()).unwrap().led_counts();
        let limits: Vec<u16> = counts.iter().map(|c| c.limit).collect();
        assert_eq!(limits, vec![32, 64, 256, 512, 1024, 1024]);
        assert!(counts[..5].iter().all(|c| c.known));
        assert!(!counts[5].known);
        assert_eq!(counts[5].raw, 0x0F);
    }

    #[test]
    fn resposta_curta_e_erro() {
        assert_eq!(decode(&sample()[..63]), Err(DecodeError::ShortResponse(63)));
        assert_eq!(decode(&[]), Err(DecodeError::ShortResponse(0)));
    }

    #[test]
    fn nao_encontrado_numa_arvore_vazia() {
        let root = tempfile::tempdir().unwrap();
        assert!(matches!(find(root.path()), Err(ProbeError::NotFound)));
    }
}
