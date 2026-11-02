// SPDX-License-Identifier: GPL-2.0-or-later

//! Leitura do LCD 320×320 do Castor3, só com os pedidos que o GIGABYTE
//! Control Center 26.09.10.01 faz ao abrir a página do LCD.
//!
//! O `LcdControl` (`ucCooler.Views/LcdControl.cs`, linhas 195–330) lê o modo
//! com o `LcdApi.GetMode` e depois só as leituras daquele modo:
//! - modos 0 a 3 e 5: `GetDisplay` (`99 DF`) e `GetZoneColor` (`99 EA <modo+1>`);
//! - modo 4: `GetGBTLogo` (`99 E9`) e `GetZoneColor`;
//! - modo 6: `GetZoneColor`;
//! - modos 7 a 9: `GetMediaList` (`99 F3`, 500 ms, `99 F4`, `99 F5` por item),
//!   `GetMediaLoopIndex` (`99 FD <modo+1>`) e `GetDiskFree` (`99 FA <disco>`);
//! - modo 11: `GetLoop` (`99 FC`).
//!
//! Nenhum comando de escrita do LCD (`E7`, `E2`, `E4`, `FB`, `F0`, `F1`, `F2`,
//! `F6`, `FE`) nem `B6` existe aqui. O `F3` não tem resposta (`CoolerApi.Send`)
//! e só é enviado nos modos de mídia, como no GCC. Bytes e decodificação em
//! `docs/protocols/castor3.md`, seção "LCD".

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::find_hidraw;
use super::led::{OutputReports, Rgb};
use super::probe::{IN_SIZE, InputReports, ProbeError, RESPONSE_TIMEOUT, packet};
use crate::hid::Hidraw;

/// Espera entre o `F3` e o `F4` (`ThreadSleep.Sleep(500)` do `GetMediaList`).
pub const MEDIA_LIST_WAIT: Duration = Duration::from_millis(500);

/// Nomes do `enum DisplayMode` do GCC (`ucCooler.Models.Lcd/DisplayMode.cs`).
const MODE_NAMES: [&str; 12] = [
    "Faith1", "Faith2", "Faith3", "Faith4", "Faith5", "Function", "Text", "Image", "Gif", "Video",
    "Pet", "Carousel",
];

/// Nomes dos bits do `enum Display` do GCC (`ucCooler.Models.Lcd/Display.cs`),
/// na ordem dos bytes `r[2..10]` do `DF`.
pub const DISPLAY_ITEMS: [&str; 9] = [
    "FanRPM",
    "CPUClock",
    "PumpRPM",
    "CPUName",
    "CPUTemperature",
    "CPUThread",
    "LiquidTemperature",
    "RAMTemperature",
    "Fahrenheit",
];

/// Nome do modo pelo `DisplayMode`, ou `None` fora de 0 a 11.
pub fn mode_name(mode: i16) -> Option<&'static str> {
    usize::try_from(mode)
        .ok()
        .and_then(|m| MODE_NAMES.get(m).copied())
}

/// Disco do `FA` (`LcdApi.GetDisk` para o 7A4D): `B` para imagem e GIF, `E`
/// para vídeo.
pub fn disk_for_mode(mode: i16) -> u8 {
    if mode == 9 { b'E' } else { b'B' }
}

/// Um pedido do LCD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcdQuery {
    /// `99 E8` (`GetMode`).
    Mode,
    /// `99 DF` (`GetDisplay`).
    Display,
    /// `99 EA <modo+1>` (`GetZoneColor`).
    ZoneColor { mode: u8 },
    /// `99 E9` (`GetGBTLogo`).
    Logo,
    /// `99 F3 <modo-6>`, sem resposta (`GetMediaList`).
    MediaPrepare { kind: u8 },
    /// `99 F4 <modo-6>` → quantidade em `r[3]`.
    MediaCount { kind: u8 },
    /// `99 F5 <modo-6> <i>` → loop em `r[4]`, nome em `r[6..]`.
    MediaItem { kind: u8, index: u8 },
    /// `99 FD <modo+1>` (`GetMediaLoopIndex`).
    MediaLoop { mode: u8 },
    /// `99 FA <disco>` (`GetDiskFree`).
    DiskFree { disk: u8 },
    /// `99 FC` (`GetLoop`).
    Carousel,
}

impl LcdQuery {
    pub fn payload(self) -> Vec<u8> {
        match self {
            Self::Mode => vec![0xE8],
            Self::Display => vec![0xDF],
            Self::ZoneColor { mode } => vec![0xEA, mode],
            Self::Logo => vec![0xE9],
            Self::MediaPrepare { kind } => vec![0xF3, kind],
            Self::MediaCount { kind } => vec![0xF4, kind],
            Self::MediaItem { kind, index } => vec![0xF5, kind, index],
            Self::MediaLoop { mode } => vec![0xFD, mode],
            Self::DiskFree { disk } => vec![0xFA, disk],
            Self::Carousel => vec![0xFC],
        }
    }

    pub fn packet(self) -> Vec<u8> {
        packet(&self.payload())
    }

    /// Só o `F3` não tem resposta.
    pub fn expects_response(self) -> bool {
        !matches!(self, Self::MediaPrepare { .. })
    }

    /// Bytes que a resposta precisa repetir: o comando, como o `CoolerApi.Receive`
    /// padrão; no `F5`, também o tipo e o índice (`nRecvCheckLength = 3`).
    fn response_prefix(self) -> Vec<u8> {
        let p = self.payload();
        match self {
            Self::MediaItem { .. } => p,
            _ => p[..1].to_vec(),
        }
    }

    /// Decodifica a resposta, com ou sem o report ID. `None` se for de outro
    /// pedido ou curta demais.
    pub fn decode(self, data: &[u8]) -> Option<LcdReading> {
        let body = strip_report_id(data);
        if !body.starts_with(&self.response_prefix()) {
            return None;
        }
        // `r(n)` é o `r[n]` do castor3.md, que conta o report ID como `r[0]`.
        let r = |n: usize| body.get(n - 1).copied();
        // Bytes a partir de `r[from]` até o primeiro zero. `None` se a resposta
        // acaba antes de `r[from]`: um campo ausente não é uma lista vazia
        // (essa tem o zero em `r[from]`).
        let until_zero = |from: usize| -> Option<Vec<u8>> {
            let field = body.get(from - 1..).filter(|f| !f.is_empty())?;
            Some(field.iter().copied().take_while(|&b| b != 0).collect())
        };
        Some(match self {
            Self::Mode => LcdReading::Mode(i16::from(r(2)?) - 1),
            Self::Display => {
                let mut items = [false; 9];
                for (i, item) in items.iter_mut().enumerate() {
                    *item = r(2 + i)? == 1;
                }
                LcdReading::Display {
                    items,
                    byte11: r(11)?,
                }
            }
            Self::ZoneColor { .. } => {
                LcdReading::ZoneColor(Rgb(r(3)?, r(4)?, r(5)?), Rgb(r(6)?, r(7)?, r(8)?))
            }
            Self::Logo => LcdReading::Logo(r(2)?),
            Self::MediaPrepare { .. } => return None,
            Self::MediaCount { .. } => LcdReading::MediaCount(r(3)?),
            Self::MediaItem { .. } => {
                let looped = r(4)? == 1;
                let name = String::from_utf8_lossy(&until_zero(6)?).into_owned();
                LcdReading::MediaItem { looped, name }
            }
            Self::MediaLoop { .. } => LcdReading::MediaLoop {
                interval: r(3)?,
                indices: until_zero(4)?.into_iter().map(|b| b - 1).collect(),
            },
            Self::DiskFree { .. } => {
                let n = usize::from(r(2)?);
                let mut size = 0u64;
                for i in 0..n.min(8) {
                    size |= u64::from(r(3 + i)?) << (8 * i);
                }
                LcdReading::DiskFree(size)
            }
            Self::Carousel => LcdReading::Carousel {
                interval: r(2)?,
                modes: until_zero(3)?
                    .into_iter()
                    .map(|b| i16::from(b) - 1)
                    .collect(),
            },
        })
    }
}

impl fmt::Display for LcdQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex: Vec<String> = self.payload().iter().map(|b| format!("{b:02X}")).collect();
        write!(f, "99 {}", hex.join(" "))
    }
}

fn strip_report_id(data: &[u8]) -> &[u8] {
    match data.first() {
        Some(&b) if b == super::probe::REPORT_ID => &data[1..],
        _ => data,
    }
}

/// Leitura decodificada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LcdReading {
    Mode(i16),
    /// Itens ligados, na ordem de [`DISPLAY_ITEMS`], e o `r[11]` cru.
    Display {
        items: [bool; 9],
        byte11: u8,
    },
    ZoneColor(Rgb, Rgb),
    Logo(u8),
    MediaCount(u8),
    MediaItem {
        looped: bool,
        name: String,
    },
    MediaLoop {
        interval: u8,
        indices: Vec<u8>,
    },
    /// Em KiB, segundo o GCC (que divide por 1024 para mostrar MB).
    DiskFree(u64),
    Carousel {
        interval: u8,
        modes: Vec<i16>,
    },
}

/// Leituras fixas de cada modo, fora da mídia (que depende do `F4`).
pub fn fixed_queries(mode: i16) -> Vec<LcdQuery> {
    let zone = |m: i16| LcdQuery::ZoneColor {
        mode: (m + 1) as u8,
    };
    match mode {
        0..=3 | 5 => vec![LcdQuery::Display, zone(mode)],
        4 => vec![LcdQuery::Logo, zone(mode)],
        6 => vec![zone(mode)],
        11 => vec![LcdQuery::Carousel],
        _ => Vec::new(),
    }
}

fn is_media_mode(mode: i16) -> bool {
    (7..=9).contains(&mode)
}

/// Um pedido e a resposta dele, ou `None` se não chegou no prazo. O `F3`, que
/// não tem resposta, também fica com `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LcdResult {
    pub query: LcdQuery,
    pub response: Option<Vec<u8>>,
}

impl LcdResult {
    pub fn reading(&self) -> Option<LcdReading> {
        self.response.as_deref().and_then(|r| self.query.decode(r))
    }

    /// Faltou uma resposta que era esperada.
    pub fn missing(&self) -> bool {
        self.query.expects_response() && self.response.is_none()
    }
}

/// Espera a resposta de `query`, descartando outros input reports (os `DA` do
/// kernel, ou um `F5` de outro item) até `timeout`.
fn wait_for(
    source: &impl InputReports,
    query: LcdQuery,
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
        if query.decode(&buf[..n]).is_some() {
            return Ok(Some(buf[..n].to_vec()));
        }
    }
}

/// Quanto tempo o probe fica com o aparelho aberto (e com o `flock`), no
/// máximo. O `halod` espera o lock por 5 s (`LOCK_TIMEOUT` do hidraw) e só renova
/// o watchdog de 20 s com envios aceitos: com 10 s, o próximo tick dele ainda
/// cabe com folga, e o `ExecStopPost` também consegue o lock.
pub const PROBE_BUDGET: Duration = Duration::from_secs(10);

/// Resultado do [`read_lcd`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LcdRead {
    pub results: Vec<LcdResult>,
    /// O prazo acabou antes das leituras do modo terminarem.
    pub timed_out: bool,
}

/// Uma leitura em andamento: os pedidos enviados e o prazo total.
struct Session<'a, D: OutputReports + InputReports> {
    device: &'a D,
    deadline: Instant,
    out: Vec<LcdResult>,
    timed_out: bool,
}

impl<D: OutputReports + InputReports> Session<'_, D> {
    fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    /// Envia `query` e espera a resposta pelo que sobrar do prazo, até 2 s.
    /// Sem prazo, não envia nada e marca o fim.
    fn send(&mut self, query: LcdQuery) -> io::Result<Option<LcdReading>> {
        let remaining = self.remaining();
        if self.timed_out || remaining.is_zero() {
            self.timed_out = true;
            return Ok(None);
        }
        self.send_now(query, remaining.min(RESPONSE_TIMEOUT))
    }

    fn send_now(&mut self, query: LcdQuery, timeout: Duration) -> io::Result<Option<LcdReading>> {
        self.device.write_output_report(&query.packet())?;
        let response = if query.expects_response() {
            wait_for(self.device, query, timeout)?
        } else {
            None
        };
        let result = LcdResult { query, response };
        let reading = result.reading();
        self.out.push(result);
        Ok(reading)
    }
}

/// Lê o modo e as leituras dele, um pedido de cada vez, como o `LcdControl`,
/// com o aparelho aberto por no máximo [`PROBE_BUDGET`].
pub fn read_lcd(
    device: &(impl OutputReports + InputReports),
    media_wait: Duration,
) -> io::Result<LcdRead> {
    read_lcd_within(device, media_wait, PROBE_BUDGET)
}

/// [`read_lcd`] com o prazo total `budget`. O `E8` sempre vai, com até 2 s;
/// sem resposta dele, nada mais é enviado. Depois, cada pedido só sai se ainda
/// houver prazo, e a lista de mídia para no primeiro `F5` sem resposta.
/// `media_wait` é a espera entre o `F3` e o `F4` ([`MEDIA_LIST_WAIT`] no
/// aparelho).
pub fn read_lcd_within(
    device: &(impl OutputReports + InputReports),
    media_wait: Duration,
    budget: Duration,
) -> io::Result<LcdRead> {
    let mut s = Session {
        device,
        deadline: Instant::now() + budget,
        out: Vec::new(),
        timed_out: false,
    };
    let mode = s.send_now(LcdQuery::Mode, RESPONSE_TIMEOUT)?;
    if let Some(LcdReading::Mode(mode)) = mode {
        read_mode(&mut s, mode, media_wait)?;
    }
    Ok(LcdRead {
        results: s.out,
        timed_out: s.timed_out,
    })
}

fn read_mode(
    s: &mut Session<'_, impl OutputReports + InputReports>,
    mode: i16,
    media_wait: Duration,
) -> io::Result<()> {
    for query in fixed_queries(mode) {
        s.send(query)?;
    }
    if !is_media_mode(mode) {
        return Ok(());
    }
    let kind = (mode - 6) as u8;
    s.send(LcdQuery::MediaPrepare { kind })?;
    std::thread::sleep(media_wait.min(s.remaining()));
    if let Some(LcdReading::MediaCount(count)) = s.send(LcdQuery::MediaCount { kind })? {
        for index in 0..count {
            if s.send(LcdQuery::MediaItem { kind, index })?.is_none() {
                // Um item mudo: os outros provavelmente também; não esperar
                // 2 s por cada um.
                break;
            }
        }
    }
    s.send(LcdQuery::MediaLoop {
        mode: (mode + 1) as u8,
    })?;
    s.send(LcdQuery::DiskFree {
        disk: disk_for_mode(mode),
    })?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LcdReport {
    pub devnode: PathBuf,
    pub hid_name: Option<String>,
    pub results: Vec<LcdResult>,
    /// O prazo de [`PROBE_BUDGET`] acabou antes das leituras do modo.
    pub timed_out: bool,
}

impl LcdReport {
    /// O modo lido, se o `E8` respondeu.
    pub fn mode(&self) -> Option<i16> {
        self.results.first().and_then(|r| match r.reading() {
            Some(LcdReading::Mode(m)) => Some(m),
            _ => None,
        })
    }

    pub fn complete(&self) -> bool {
        !self.timed_out && !self.results.iter().any(LcdResult::missing)
    }
}

/// Acha o Waterforce, abre o hidraw (com o `flock`) e lê o LCD.
pub fn probe_lcd(sys_root: &Path) -> Result<LcdReport, ProbeError> {
    let info = find_hidraw(sys_root)
        .map_err(ProbeError::Io)?
        .ok_or(ProbeError::NotFound)?;
    let device = Hidraw::open(&info.devnode).map_err(ProbeError::Open)?;
    let read = read_lcd(&device, MEDIA_LIST_WAIT).map_err(ProbeError::Io)?;
    // O `flock` é solto aqui, ao fechar o hidraw.
    drop(device);
    Ok(LcdReport {
        devnode: info.devnode,
        hid_name: info.hid_name,
        results: read.results,
        timed_out: read.timed_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    /// Aparelho falso: grava cada pacote e, a cada escrita, enfileira as
    /// respostas que `respond` der para ele.
    struct Fake<F: Fn(&[u8]) -> Vec<Vec<u8>>> {
        sent: RefCell<Vec<Vec<u8>>>,
        pending: RefCell<VecDeque<Vec<u8>>>,
        respond: F,
    }

    impl<F: Fn(&[u8]) -> Vec<Vec<u8>>> Fake<F> {
        fn new(respond: F) -> Self {
            Self {
                sent: RefCell::new(Vec::new()),
                pending: RefCell::new(VecDeque::new()),
                respond,
            }
        }

        /// Comando e parâmetros de cada pacote enviado, sem o report ID e sem
        /// os zeros (até 3 bytes).
        fn heads(&self) -> Vec<Vec<u8>> {
            self.sent
                .borrow()
                .iter()
                .map(|p| {
                    let mut h = p[1..4].to_vec();
                    while h.len() > 1 && h.last() == Some(&0) {
                        h.pop();
                    }
                    h
                })
                .collect()
        }
    }

    impl<F: Fn(&[u8]) -> Vec<Vec<u8>>> OutputReports for Fake<F> {
        fn write_output_report(&self, data: &[u8]) -> io::Result<()> {
            assert_eq!(data.len(), 6144);
            assert_eq!(data[0], 0x99);
            self.sent.borrow_mut().push(data.to_vec());
            self.pending.borrow_mut().extend((self.respond)(&data[1..]));
            Ok(())
        }
    }

    impl<F: Fn(&[u8]) -> Vec<Vec<u8>>> InputReports for Fake<F> {
        fn read_input_report(&self, buf: &mut [u8], _: Duration) -> io::Result<Option<usize>> {
            Ok(self.pending.borrow_mut().pop_front().map(|r| {
                buf[..r.len()].copy_from_slice(&r);
                r.len()
            }))
        }
    }

    /// Respostas de um aparelho no modo `mode`, com `items` arquivos de mídia.
    fn device(mode: i16, items: u8) -> impl Fn(&[u8]) -> Vec<Vec<u8>> {
        move |p: &[u8]| {
            let cmd = p[0];
            let mut r = vec![0x99, cmd];
            match cmd {
                0xE8 => r.push((mode + 1) as u8),
                0xDF => r.extend([0, 0, 0, 0, 1, 0, 1, 0, 0, 0x05]),
                0xEA => r.extend([p[1], 0xFF, 0, 0, 0, 0, 0xFF]),
                0xE9 => r.push(1),
                0xF3 => return Vec::new(),
                0xF4 => r.extend([p[1], items]),
                0xF5 => {
                    r.extend([p[1], p[2], 1, 0]);
                    r.extend(format!("img{}.jpg", p[2]).bytes());
                }
                0xFD => r.extend([p[1], 3, 1, 2, 0]),
                0xFA => r.extend([3, 0x00, 0x10, 0x00]),
                0xFC => r.extend([10, 4, 8, 0]),
                _ => return Vec::new(),
            }
            vec![r]
        }
    }

    fn run(mode: i16, items: u8) -> (Vec<Vec<u8>>, Vec<LcdResult>) {
        let fake = Fake::new(device(mode, items));
        let results = read_lcd(&fake, Duration::ZERO).unwrap().results;
        (fake.heads(), results)
    }

    #[test]
    fn lcd_pacotes_completados_com_zeros() {
        let p = LcdQuery::ZoneColor { mode: 4 }.packet();
        assert_eq!(p.len(), 6144);
        assert_eq!(&p[..3], &[0x99, 0xEA, 0x04]);
        assert!(p[3..].iter().all(|&b| b == 0));
    }

    #[test]
    fn lcd_modo_e_nomes() {
        assert_eq!(
            LcdQuery::Mode.decode(&[0x99, 0xE8, 0x04]),
            Some(LcdReading::Mode(3))
        );
        assert_eq!(mode_name(3), Some("Faith4"));
        assert_eq!(mode_name(11), Some("Carousel"));
        assert_eq!(
            LcdQuery::Mode.decode(&[0x99, 0xE8, 0x00]),
            Some(LcdReading::Mode(-1))
        );
        assert_eq!(mode_name(-1), None);
        assert_eq!(mode_name(12), None);
        assert_eq!(LcdQuery::Mode.decode(&[0x99, 0xDA, 0x04]), None);
    }

    #[test]
    fn lcd_itens_e_cores() {
        let mut df = vec![0x99, 0xDF, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0x07];
        let Some(LcdReading::Display { items, byte11 }) = LcdQuery::Display.decode(&df) else {
            panic!();
        };
        let on: Vec<&str> = DISPLAY_ITEMS
            .iter()
            .zip(items)
            .filter(|(_, on)| *on)
            .map(|(n, _)| *n)
            .collect();
        assert_eq!(on, ["CPUTemperature", "LiquidTemperature"]);
        assert_eq!(byte11, 0x07);
        df[6] = 0;
        df[8] = 0;
        assert_eq!(
            LcdQuery::Display.decode(&df),
            Some(LcdReading::Display {
                items: [false; 9],
                byte11: 0x07
            })
        );
        assert_eq!(
            LcdQuery::ZoneColor { mode: 4 }.decode(&[0x99, 0xEA, 0x04, 0xFF, 0, 0, 0, 0, 0xFF]),
            Some(LcdReading::ZoneColor(Rgb(0xFF, 0, 0), Rgb(0, 0, 0xFF)))
        );
    }

    #[test]
    fn lcd_midia_espaco_e_carrossel() {
        assert_eq!(
            LcdQuery::DiskFree { disk: b'B' }.decode(&[0x99, 0xFA, 3, 0x01, 0x02, 0x03]),
            Some(LcdReading::DiskFree(0x030201))
        );
        assert_eq!(
            LcdQuery::Carousel.decode(&[0x99, 0xFC, 10, 4, 8, 0, 9]),
            Some(LcdReading::Carousel {
                interval: 10,
                modes: vec![3, 7]
            })
        );
        assert_eq!(
            LcdQuery::MediaLoop { mode: 8 }.decode(&[0x99, 0xFD, 8, 3, 1, 2, 0, 5]),
            Some(LcdReading::MediaLoop {
                interval: 3,
                indices: vec![0, 1]
            })
        );
        let mut f5 = vec![0x99, 0xF5, 1, 1, 1, 0];
        f5.extend(b"foto.jpg\0lixo");
        assert_eq!(
            LcdQuery::MediaItem { kind: 1, index: 1 }.decode(&f5),
            Some(LcdReading::MediaItem {
                looped: true,
                name: "foto.jpg".into()
            })
        );
        assert_eq!(LcdQuery::MediaItem { kind: 1, index: 0 }.decode(&f5), None);
        assert_eq!(disk_for_mode(7), b'B');
        assert_eq!(disk_for_mode(8), b'B');
        assert_eq!(disk_for_mode(9), b'E');
    }

    #[test]
    fn lcd_resposta_truncada_nao_e_lista_vazia() {
        let item = LcdQuery::MediaItem { kind: 1, index: 0 };
        // Termina antes do nome (r[6]).
        assert_eq!(item.decode(&[0x99, 0xF5, 1, 0, 1]), None);
        assert_eq!(item.decode(&[0x99, 0xF5, 1, 0, 1, 0]), None);
        // Com r[6] = 0, o nome é vazio de verdade.
        assert_eq!(
            item.decode(&[0x99, 0xF5, 1, 0, 1, 0, 0]),
            Some(LcdReading::MediaItem {
                looped: true,
                name: String::new()
            })
        );
        let looped = LcdQuery::MediaLoop { mode: 8 };
        assert_eq!(looped.decode(&[0x99, 0xFD, 8, 3]), None);
        assert_eq!(
            looped.decode(&[0x99, 0xFD, 8, 3, 0]),
            Some(LcdReading::MediaLoop {
                interval: 3,
                indices: vec![]
            })
        );
        assert_eq!(LcdQuery::Carousel.decode(&[0x99, 0xFC, 10]), None);
        assert_eq!(
            LcdQuery::Carousel.decode(&[0x99, 0xFC, 10, 0]),
            Some(LcdReading::Carousel {
                interval: 10,
                modes: vec![]
            })
        );
    }

    #[test]
    fn lcd_sequencia_por_modo() {
        let h = |v: &[&[u8]]| -> Vec<Vec<u8>> { v.iter().map(|x| x.to_vec()).collect() };
        assert_eq!(run(3, 0).0, h(&[&[0xE8], &[0xDF], &[0xEA, 0x04]]));
        assert_eq!(run(0, 0).0, h(&[&[0xE8], &[0xDF], &[0xEA, 0x01]]));
        assert_eq!(run(5, 0).0, h(&[&[0xE8], &[0xDF], &[0xEA, 0x06]]));
        assert_eq!(run(4, 0).0, h(&[&[0xE8], &[0xE9], &[0xEA, 0x05]]));
        assert_eq!(run(6, 0).0, h(&[&[0xE8], &[0xEA, 0x07]]));
        assert_eq!(
            run(7, 2).0,
            h(&[
                &[0xE8],
                &[0xF3, 0x01],
                &[0xF4, 0x01],
                // O índice 0 fica como `F5 01` depois de tirar os zeros.
                &[0xF5, 0x01],
                &[0xF5, 0x01, 0x01],
                &[0xFD, 0x08],
                &[0xFA, 0x42],
            ])
        );
        assert_eq!(run(9, 0).0.last(), Some(&vec![0xFA, 0x45]));
        assert_eq!(run(11, 0).0, h(&[&[0xE8], &[0xFC]]));
        assert_eq!(run(10, 0).0, h(&[&[0xE8]]));
        assert_eq!(run(-1, 0).0, h(&[&[0xE8]]));
    }

    #[test]
    fn lcd_respostas_do_modo_3() {
        let (_, results) = run(3, 0);
        let readings: Vec<Option<LcdReading>> = results.iter().map(LcdResult::reading).collect();
        assert_eq!(readings[0], Some(LcdReading::Mode(3)));
        assert!(matches!(readings[1], Some(LcdReading::Display { .. })));
        assert_eq!(
            readings[2],
            Some(LcdReading::ZoneColor(Rgb(0xFF, 0, 0), Rgb(0, 0, 0xFF)))
        );
        assert!(results.iter().all(|r| !r.missing()));
    }

    #[test]
    fn lcd_nenhum_comando_de_escrita() {
        const WRITES: [u8; 10] = [0xE7, 0xE2, 0xE4, 0xFB, 0xF0, 0xF1, 0xF2, 0xF6, 0xFE, 0xB6];
        for mode in 0..=12 {
            let (heads, _) = run(mode, 3);
            for head in &heads {
                assert!(!WRITES.contains(&head[0]), "modo {mode}: {head:02x?}");
            }
            let f3 = heads.iter().any(|h| h[0] == 0xF3);
            assert_eq!(f3, (7..=9).contains(&mode), "modo {mode}");
        }
    }

    #[test]
    fn lcd_sem_resposta_do_modo_para_tudo() {
        let fake = Fake::new(|_: &[u8]| Vec::new());
        let results = read_lcd(&fake, Duration::ZERO).unwrap().results;
        assert_eq!(fake.heads(), [vec![0xE8]]);
        assert_eq!(results.len(), 1);
        assert!(results[0].missing());
    }

    #[test]
    fn lcd_descarta_outras_respostas() {
        // Um DA do kernel antes do DF, e um F5 de outro item antes do certo.
        let inner = device(7, 2);
        let fake = Fake::new(move |p: &[u8]| {
            let mut out = Vec::new();
            match p[0] {
                0xDF => out.push(vec![0x99, 0xDA, 1, 2, 3]),
                0xF5 if p[2] == 1 => out.push(vec![0x99, 0xF5, 1, 0, 1, 0, b'x']),
                _ => {}
            }
            out.extend(inner(p));
            out
        });
        let results = read_lcd(&fake, Duration::ZERO).unwrap().results;
        let item1 = results
            .iter()
            .find(|r| r.query == LcdQuery::MediaItem { kind: 1, index: 1 })
            .unwrap();
        assert_eq!(
            item1.reading(),
            Some(LcdReading::MediaItem {
                looped: true,
                name: "img1.jpg".into()
            })
        );

        let inner = device(3, 0);
        let fake = Fake::new(move |p: &[u8]| {
            let mut out = Vec::new();
            if p[0] == 0xDF {
                out.push(vec![0x99, 0xDA, 1, 2, 3]);
            }
            out.extend(inner(p));
            out
        });
        let results = read_lcd(&fake, Duration::ZERO).unwrap().results;
        assert!(matches!(
            results[1].reading(),
            Some(LcdReading::Display { .. })
        ));
    }

    #[test]
    fn lcd_prazo_total() {
        // Prazo zero: só o E8, e o fim do prazo fica marcado.
        let fake = Fake::new(device(3, 0));
        let read = read_lcd_within(&fake, Duration::ZERO, Duration::ZERO).unwrap();
        assert_eq!(fake.heads(), [vec![0xE8]]);
        assert!(read.timed_out);
        // Com prazo, nada marcado.
        let fake = Fake::new(device(3, 0));
        let read = read_lcd_within(&fake, Duration::ZERO, PROBE_BUDGET).unwrap();
        assert!(!read.timed_out);
        assert_eq!(read.results.len(), 3);
    }

    #[test]
    fn lcd_lista_para_no_primeiro_item_mudo() {
        // F4 informa 30 itens e nenhum F5 responde.
        let inner = device(7, 30);
        let fake = Fake::new(
            move |p: &[u8]| {
                if p[0] == 0xF5 { Vec::new() } else { inner(p) }
            },
        );
        let read = read_lcd(&fake, Duration::ZERO).unwrap();
        let f5 = fake.heads().iter().filter(|h| h[0] == 0xF5).count();
        assert_eq!(f5, 1);
        // Depois da lista, o loop e o espaço livre ainda vão.
        assert_eq!(fake.heads().last(), Some(&vec![0xFA, 0x42]));
        assert!(read.results.iter().any(LcdResult::missing));
    }

    #[test]
    fn lcd_f3_nao_conta_como_falta() {
        let (_, results) = run(7, 0);
        let f3 = results
            .iter()
            .find(|r| matches!(r.query, LcdQuery::MediaPrepare { .. }))
            .unwrap();
        assert!(f3.response.is_none());
        assert!(!f3.missing());
        assert!(results.iter().all(|r| !r.missing()));
    }
}
