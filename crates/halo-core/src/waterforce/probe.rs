// SPDX-License-Identifier: GPL-2.0-or-later

//! Probe somente-leitura do Castor3 pelo hidraw.
//!
//! Só pedidos de leitura existem aqui, enviados nesta ordem:
//! - `99 D6`: `get_firmware_ver_cmd` de `drivers/hwmon/gigabyte_waterforce.c`
//!   do Linux. Resposta `99 D6 a b`, firmware `a.b`.
//! - `99 DE 00`, `99 DD`, `99 D9 01`, `99 D9 02` e `99 C7`: `DeviceApi`
//!   (`GetDeviceModelName`, `GetMode`, `GetSpeedCurve`, `GetFanQty`) do
//!   `ucCooler.dll` do GCC 26.09.10.01.
//! - `99 E8`: `LcdApi.GetMode` do mesmo `ucCooler.dll`.
//!
//! Todos vão como output report de 6144 bytes completado com zeros
//! (`MAX_REPORT_LENGTH` no kernel, `OutputReportLen` no GCC). Os bytes e a
//! decodificação estão em `docs/protocols/castor3.md`.

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

/// Pacote completo: report ID, payload e zeros até 6144 bytes.
pub fn packet(payload: &[u8]) -> Vec<u8> {
    let mut packet = vec![0u8; OUT_SIZE];
    packet[0] = REPORT_ID;
    packet[1..1 + payload.len()].copy_from_slice(payload);
    packet
}

/// Nome do modelo pelo `r[2]` do `DE` (`DeviceApi.GetDeviceModelName`).
pub fn model_name(index: u8) -> Option<&'static str> {
    match index {
        0 => Some("Waterforce X 240"),
        1 => Some("Waterforce X 280"),
        2 => Some("Waterforce X 360"),
        _ => None,
    }
}

/// Pedidos de leitura conhecidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadQuery {
    /// `D6`, do driver do kernel.
    Firmware,
    /// `DE 00`.
    Model,
    /// `DD`.
    SpeedModes,
    /// `D9 01`.
    FanCurve,
    /// `D9 02`.
    PumpCurve,
    /// `C7`.
    FanQty,
    /// `E8`.
    LcdMode,
}

impl ReadQuery {
    pub const ALL: [Self; 7] = [
        Self::Firmware,
        Self::Model,
        Self::SpeedModes,
        Self::FanCurve,
        Self::PumpCurve,
        Self::FanQty,
        Self::LcdMode,
    ];

    pub fn payload(self) -> &'static [u8] {
        match self {
            Self::Firmware => &[0xD6],
            Self::Model => &[0xDE, 0x00],
            Self::SpeedModes => &[0xDD],
            Self::FanCurve => &[0xD9, 0x01],
            Self::PumpCurve => &[0xD9, 0x02],
            Self::FanQty => &[0xC7],
            Self::LcdMode => &[0xE8],
        }
    }

    /// Primeiro byte de dados da resposta, igual ao comando.
    pub fn command(self) -> u8 {
        self.payload()[0]
    }

    pub fn packet(self) -> Vec<u8> {
        packet(self.payload())
    }

    /// Decodifica o input report, com ou sem o report ID na frente.
    /// `None` se a resposta for de outro comando ou curta demais.
    pub fn decode(self, data: &[u8]) -> Option<Reading> {
        let body = strip_report_id(data);
        if body.first() != Some(&self.command()) {
            return None;
        }
        // `r(n)` é o `r[n]` do castor3.md, que conta o report ID como `r[0]`.
        let r = |n: usize| body.get(n - 1).copied();
        Some(match self {
            Self::Firmware => Reading::Firmware {
                major: r(2)?,
                minor: r(3)?,
            },
            Self::Model => Reading::Model(r(2)?),
            Self::SpeedModes => Reading::SpeedModes {
                fan: SpeedMode(r(2)?),
                pump: SpeedMode(r(3)?),
            },
            Self::FanCurve | Self::PumpCurve => {
                let mut points = [CurvePoint::default(); 4];
                for (i, point) in points.iter_mut().enumerate() {
                    *point = CurvePoint {
                        celsius: r(4 + 3 * i)?,
                        rpm: u16::from_be_bytes([r(5 + 3 * i)?, r(6 + 3 * i)?]),
                    };
                }
                Reading::Curve(points)
            }
            Self::FanQty => Reading::FanQty(r(2)?),
            Self::LcdMode => Reading::LcdMode(i16::from(r(2)?) - 1),
        })
    }
}

/// Modo de velocidade (`SpeedMode` do GCC), com o valor bruto do aparelho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeedMode(pub u8);

impl SpeedMode {
    /// Nome que o GCC mostra. Ele lê o 3 (FixedRPM) como 0 (Balanced).
    pub fn name(self) -> Option<&'static str> {
        match self.0 {
            0 | 3 => Some("Balanced"),
            1 => Some("Customized"),
            2 => Some("Default"),
            4 => Some("Turbo"),
            5 => Some("Performance"),
            6 => Some("Quiet"),
            7 => Some("ZeroRPM"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CurvePoint {
    pub celsius: u8,
    pub rpm: u16,
}

/// Valor decodificado de uma resposta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    Firmware {
        major: u8,
        minor: u8,
    },
    /// `r[2]` do `DE`; ver [`model_name`].
    Model(u8),
    SpeedModes {
        fan: SpeedMode,
        pump: SpeedMode,
    },
    Curve([CurvePoint; 4]),
    FanQty(u8),
    /// `r[2] - 1` do `E8`.
    LcdMode(i16),
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
    pub query: ReadQuery,
    pub response: Option<Vec<u8>>,
}

impl QueryResult {
    pub fn reading(&self) -> Option<Reading> {
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

/// Envia cada pedido de [`ReadQuery::ALL`], um de cada vez, e espera a
/// resposta dele. Nenhum outro pacote é enviado.
pub fn probe(sys_root: &Path) -> Result<ProbeReport, ProbeError> {
    let info = find_hidraw(sys_root)
        .map_err(ProbeError::Io)?
        .ok_or(ProbeError::NotFound)?;
    let device = Hidraw::open(&info.devnode).map_err(ProbeError::Open)?;

    let mut results = Vec::new();
    for query in ReadQuery::ALL {
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
pub(crate) mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    /// Fila de input reports. Vazia, devolve `None` como um prazo vencido.
    pub(crate) struct Fake(pub RefCell<VecDeque<Vec<u8>>>);

    impl Fake {
        pub(crate) fn new(reports: &[&[u8]]) -> Self {
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
    fn pacotes_exatos_na_ordem() {
        let heads: Vec<Vec<u8>> = ReadQuery::ALL
            .iter()
            .map(|q| {
                let p = q.packet();
                assert_eq!(p.len(), 6144);
                let n = 1 + q.payload().len();
                assert!(p[n..].iter().all(|&b| b == 0));
                p[..n].to_vec()
            })
            .collect();
        assert_eq!(
            heads,
            [
                &[0x99, 0xD6][..],
                &[0x99, 0xDE, 0x00],
                &[0x99, 0xDD],
                &[0x99, 0xD9, 0x01],
                &[0x99, 0xD9, 0x02],
                &[0x99, 0xC7],
                &[0x99, 0xE8],
            ]
        );
    }

    #[test]
    fn firmware_e_modelo() {
        assert_eq!(
            ReadQuery::Firmware.decode(&[0x99, 0xD6, 0x01, 0x06]),
            Some(Reading::Firmware { major: 1, minor: 6 })
        );
        assert_eq!(
            ReadQuery::Model.decode(&[0x99, 0xDE, 0x00]),
            Some(Reading::Model(0))
        );
        assert_eq!(model_name(0), Some("Waterforce X 240"));
        assert_eq!(model_name(2), Some("Waterforce X 360"));
        // Sem report ID na frente.
        assert_eq!(
            ReadQuery::Model.decode(&[0xDE, 0x05]),
            Some(Reading::Model(5))
        );
        assert_eq!(model_name(5), None);
    }

    #[test]
    fn modos_e_curva() {
        let Some(Reading::SpeedModes { fan, pump }) =
            ReadQuery::SpeedModes.decode(&[0x99, 0xDD, 0x02, 0x00])
        else {
            panic!("DD não decodificou");
        };
        assert_eq!(fan.name(), Some("Default"));
        assert_eq!(pump.name(), Some("Balanced"));

        let mut d9 = vec![0x99, 0xD9, 0x01, 0x00];
        d9.extend([
            0x00, 0x04, 0xB0, 0x1E, 0x04, 0xB0, 0x32, 0x05, 0x78, 0x41, 0x07, 0x08,
        ]);
        let curve = |c, rpm| CurvePoint { celsius: c, rpm };
        assert_eq!(
            ReadQuery::FanCurve.decode(&d9),
            Some(Reading::Curve([
                curve(0, 1200),
                curve(30, 1200),
                curve(50, 1400),
                curve(65, 1800),
            ]))
        );
    }

    #[test]
    fn fixed_rpm_lido_como_balanced() {
        assert_eq!(SpeedMode(3).name(), Some("Balanced"));
        assert_eq!(SpeedMode(9).name(), None);
    }

    #[test]
    fn fans_e_lcd() {
        assert_eq!(
            ReadQuery::FanQty.decode(&[0x99, 0xC7, 0x02]),
            Some(Reading::FanQty(2))
        );
        assert_eq!(
            ReadQuery::LcdMode.decode(&[0x99, 0xE8, 0x08]),
            Some(Reading::LcdMode(7))
        );
    }

    #[test]
    fn decodificacao_rejeita_outro_comando_ou_resposta_curta() {
        assert_eq!(ReadQuery::Firmware.decode(&[0x99, 0xDA, 0x02, 0x05]), None);
        assert_eq!(ReadQuery::Firmware.decode(&[0x99, 0xD6, 0x02]), None);
        assert_eq!(ReadQuery::Model.decode(&[0x99, 0xDE]), None);
        assert_eq!(ReadQuery::SpeedModes.decode(&[0x99, 0xDD, 0x02]), None);
        assert_eq!(
            ReadQuery::PumpCurve.decode(&[0x99, 0xD9, 0x02, 0x00, 0x00]),
            None
        );
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
                    query: ReadQuery::Firmware,
                    response: Some(vec![0x99, 0xD6, 0x01, 0x06]),
                },
                QueryResult {
                    query: ReadQuery::Model,
                    response: None,
                },
            ],
        };
        assert!(!report.complete());
        assert_eq!(
            report.results[0].reading(),
            Some(Reading::Firmware { major: 1, minor: 6 })
        );
        assert_eq!(report.results[1].reading(), None);
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
