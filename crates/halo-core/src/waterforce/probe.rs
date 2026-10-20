// SPDX-License-Identifier: GPL-2.0-or-later

//! Probe somente-leitura do Castor3 pelo hidraw.
//!
//! Só dois pedidos existem aqui, os dois de leitura de firmware:
//! - `99 D6`: `get_firmware_ver_cmd` de `drivers/hwmon/gigabyte_waterforce.c`
//!   do Linux. Resposta `99 D6 a b`, firmware `a * 10 + b`
//!   (`waterforce_raw_event()`).
//! - `99 DE 00`: construtor de `GigabyteCastor3Controller.cpp` do OpenRGB
//!   (`72262b8`). Resposta `99 DE v`, firmware `v`.
//!
//! Os dois vão como output report de 6144 bytes completado com zeros
//! (`MAX_REPORT_LENGTH` no kernel, `CASTOR3_OUT_TOTAL` no OpenRGB).

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::{PRODUCT_ID, UDEV_RULE, VENDOR_ID, find_hidraw};
use crate::hid::{Hidraw, OpenError};

pub const REPORT_ID: u8 = 0x99;
/// Report ID + 6143 bytes de payload.
pub const OUT_SIZE: usize = 6144;
/// Report ID + 255 bytes de input.
pub const IN_SIZE: usize = 256;
/// Prazo por pedido (`STATUS_VALIDITY` no kernel, `CASTOR3_IN_TIMEOUT_MS` no OpenRGB).
pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(2);

/// Pedidos de leitura de firmware conhecidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareQuery {
    /// `D6`, do driver do kernel.
    Kernel,
    /// `DE 00`, do driver do OpenRGB.
    OpenRgb,
}

impl FirmwareQuery {
    pub const ALL: [Self; 2] = [Self::Kernel, Self::OpenRgb];

    fn payload(self) -> &'static [u8] {
        match self {
            Self::Kernel => &[0xD6],
            Self::OpenRgb => &[0xDE, 0x00],
        }
    }

    /// Primeiro byte de dados da resposta, igual ao comando.
    pub fn command(self) -> u8 {
        self.payload()[0]
    }

    /// Pacote completo: report ID, payload e zeros até 6144 bytes.
    pub fn packet(self) -> Vec<u8> {
        let mut packet = vec![0u8; OUT_SIZE];
        packet[0] = REPORT_ID;
        let payload = self.payload();
        packet[1..1 + payload.len()].copy_from_slice(payload);
        packet
    }

    /// Versão de firmware segundo o driver de onde veio o pedido.
    /// `data` é o input report, com ou sem o report ID na frente.
    pub fn decode(self, data: &[u8]) -> Option<u16> {
        let body = strip_report_id(data);
        if body.first() != Some(&self.command()) {
            return None;
        }
        match self {
            Self::Kernel => Some(u16::from(*body.get(1)?) * 10 + u16::from(*body.get(2)?)),
            Self::OpenRgb => body.get(1).map(|&v| u16::from(v)),
        }
    }
}

fn strip_report_id(data: &[u8]) -> &[u8] {
    match data.first() {
        Some(&REPORT_ID) => &data[1..],
        _ => data,
    }
}

/// Fonte de input reports. Existe para testar o filtro sem hardware.
pub trait InputReports {
    fn read_input_report(&self, buf: &mut [u8], timeout: Duration) -> io::Result<Option<usize>>;
}

impl InputReports for Hidraw {
    fn read_input_report(&self, buf: &mut [u8], timeout: Duration) -> io::Result<Option<usize>> {
        Hidraw::read_input_report(self, buf, timeout)
    }
}

/// Espera a resposta de `command`, descartando outros input reports (como os
/// `DA` de status pedidos pelo kernel) até `timeout`.
pub fn wait_response(
    source: &impl InputReports,
    command: u8,
    timeout: Duration,
) -> io::Result<Option<Vec<u8>>> {
    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; IN_SIZE];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(None);
        }
        let Some(n) = source.read_input_report(&mut buf, remaining)? else {
            return Ok(None);
        };
        if strip_report_id(&buf[..n]).first() == Some(&command) {
            return Ok(Some(buf[..n].to_vec()));
        }
    }
}

/// Resposta de um pedido, ou `None` se não chegou no prazo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryResult {
    pub query: FirmwareQuery,
    pub response: Option<Vec<u8>>,
}

impl QueryResult {
    pub fn firmware(&self) -> Option<u16> {
        self.response.as_deref().and_then(|r| self.query.decode(r))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeReport {
    pub devnode: PathBuf,
    pub hid_name: Option<String>,
    pub results: Vec<QueryResult>,
}

impl ProbeReport {
    pub fn complete(&self) -> bool {
        self.results.iter().all(|r| r.response.is_some())
    }
}

#[derive(Debug)]
pub enum ProbeError {
    NotFound,
    Open(OpenError),
    Io(io::Error),
}

impl fmt::Display for ProbeError {
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
        }
    }
}

impl std::error::Error for ProbeError {}

/// Envia `D6` e depois `DE 00`, e espera cada resposta. Nenhum outro pacote
/// é enviado.
pub fn probe(sys_root: &Path) -> Result<ProbeReport, ProbeError> {
    let info = find_hidraw(sys_root)
        .map_err(ProbeError::Io)?
        .ok_or(ProbeError::NotFound)?;
    let device = Hidraw::open(&info.devnode).map_err(ProbeError::Open)?;

    let mut results = Vec::new();
    for query in FirmwareQuery::ALL {
        device
            .write_output_report(&query.packet())
            .map_err(ProbeError::Io)?;
        let response =
            wait_response(&device, query.command(), RESPONSE_TIMEOUT).map_err(ProbeError::Io)?;
        results.push(QueryResult { query, response });
    }

    Ok(ProbeReport {
        devnode: info.devnode,
        hid_name: info.hid_name,
        results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    /// Fila de input reports. Vazia, devolve `None` como um prazo vencido.
    struct Fake(RefCell<VecDeque<Vec<u8>>>);

    impl Fake {
        fn new(reports: &[&[u8]]) -> Self {
            Self(RefCell::new(reports.iter().map(|r| r.to_vec()).collect()))
        }
    }

    impl InputReports for Fake {
        fn read_input_report(&self, buf: &mut [u8], _: Duration) -> io::Result<Option<usize>> {
            Ok(self.0.borrow_mut().pop_front().map(|r| {
                buf[..r.len()].copy_from_slice(&r);
                r.len()
            }))
        }
    }

    #[test]
    fn pacotes_exatos() {
        let d6 = FirmwareQuery::Kernel.packet();
        assert_eq!(d6.len(), 6144);
        assert_eq!(&d6[..2], &[0x99, 0xD6]);
        assert!(d6[2..].iter().all(|&b| b == 0));

        let de = FirmwareQuery::OpenRgb.packet();
        assert_eq!(de.len(), 6144);
        assert_eq!(&de[..3], &[0x99, 0xDE, 0x00]);
        assert!(de[2..].iter().all(|&b| b == 0));
    }

    #[test]
    fn firmware_das_duas_fontes() {
        assert_eq!(
            FirmwareQuery::Kernel.decode(&[0x99, 0xD6, 0x02, 0x05]),
            Some(25)
        );
        assert_eq!(FirmwareQuery::OpenRgb.decode(&[0x99, 0xDE, 0x02]), Some(2));
        // Sem report ID, como o OpenRGB também aceita.
        assert_eq!(FirmwareQuery::OpenRgb.decode(&[0xDE, 0x02]), Some(2));
    }

    #[test]
    fn decodificacao_rejeita_outro_comando_ou_resposta_curta() {
        assert_eq!(
            FirmwareQuery::Kernel.decode(&[0x99, 0xDA, 0x02, 0x05]),
            None
        );
        assert_eq!(FirmwareQuery::Kernel.decode(&[0x99, 0xD6, 0x02]), None);
        assert_eq!(FirmwareQuery::OpenRgb.decode(&[0x99, 0xDE]), None);
    }

    #[test]
    fn descarta_status_do_kernel() {
        let fake = Fake::new(&[&[0x99, 0xDA, 0x9C, 0x08], &[0x99, 0xD6, 0x02, 0x05]]);
        let got = wait_response(&fake, 0xD6, Duration::from_secs(1)).unwrap();
        assert_eq!(got, Some(vec![0x99, 0xD6, 0x02, 0x05]));
    }

    #[test]
    fn sem_resposta_esperada() {
        let fake = Fake::new(&[&[0x99, 0xDA, 0x00]]);
        assert_eq!(
            wait_response(&fake, 0xDE, Duration::from_secs(1)).unwrap(),
            None
        );
    }

    #[test]
    fn relatorio_incompleto() {
        let report = ProbeReport {
            devnode: PathBuf::from("/dev/hidraw2"),
            hid_name: None,
            results: vec![
                QueryResult {
                    query: FirmwareQuery::Kernel,
                    response: Some(vec![0x99, 0xD6, 0x02, 0x05]),
                },
                QueryResult {
                    query: FirmwareQuery::OpenRgb,
                    response: None,
                },
            ],
        };
        assert!(!report.complete());
        assert_eq!(report.results[0].firmware(), Some(25));
        assert_eq!(report.results[1].firmware(), None);
    }

    #[test]
    fn mensagem_de_permissao_aponta_a_regra() {
        let err = ProbeError::Open(OpenError::PermissionDenied(PathBuf::from("/dev/hidraw2")));
        let msg = err.to_string();
        assert!(msg.contains("/dev/hidraw2"));
        assert!(msg.contains("packaging/udev/70-halo.rules"));
    }

    #[test]
    fn nao_encontrado_num_sysfs_vazio() {
        let sys = tempfile::tempdir().unwrap();
        assert!(matches!(probe(sys.path()), Err(ProbeError::NotFound)));
    }
}
