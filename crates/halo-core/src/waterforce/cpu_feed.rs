// SPDX-License-Identifier: GPL-2.0-or-later

//! Alimentação da CPU no Castor3: o nome (`E1`) e os dados (`E0`) que o
//! `AorusLcdService` do GCC envia (`CoolerDataApi.SendCpuName` e
//! `SendCoolerData`), para a curva Customized seguir o Tctl.
//!
//! O `E0` é a referência de temperatura da curva (`docs/hardware.md`, "E0
//! como referência"), e o firmware retém o último valor por pelo menos 70 s
//! sem fallback. Por isso toda falha de leitura vira [`PROTECT_CELSIUS`], e
//! o início de uma sessão sempre manda esse valor. Nenhum pacote daqui é
//! `B6`, `E5` ou `E6`, e nada toca nos snapshots.
//!
//! Bytes e fontes na matriz de `docs/protocols/castor3.md`, "Matriz
//! fonte/byte do E0 e do E1".

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::led::{OutputReports, PACKET_INTERVAL};
use super::probe::{
    InputReports, RESPONSE_TIMEOUT, ReadQuery, Reading, model_name, packet, wait_response,
};
use super::{PRODUCT_ID, UDEV_RULE, VENDOR_ID, find_hidraw};
use crate::hid::hidraw::LOCK_TIMEOUT;
use crate::hid::{HidDeviceInfo, Hidraw, OpenError};

/// Temperatura de proteção: o maior ponto de controle medido (duty 100 na
/// curva ensaiada). É uma referência alta, não um comando de duty.
pub const PROTECT_CELSIUS: u8 = 80;
/// Maior Tctl aceito, em m°C. Acima disso a leitura é tratada como falha.
pub const TCTL_MAX_MILLICELSIUS: i32 = 125_000;
/// Limite do nome no `E1`: o do `GetCpuShortName` do GCC, o maior nome que o
/// firmware já recebeu dele.
pub const NAME_MAX_BYTES: usize = 30;
/// Nome enviado quando o `/proc/cpuinfo` não informa um.
pub const FALLBACK_NAME: &str = "CPU";
/// Único modelo com a alimentação validada no aparelho: o X 240.
pub const FED_MODEL: u8 = 0;
/// `name` do hwmon do sensor de CPU AMD.
pub const K10TEMP: &str = "k10temp";
/// Rótulo do canal usado como temperatura da CPU.
pub const TCTL_LABEL: &str = "Tctl";

/// Dados fixos da CPU no `E0` e no `E1`. Campo indisponível vai como zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuInfo {
    /// `GenuineIntel` no `vendor_id`: byte 2 = 1.
    pub intel: bool,
    /// Nome já no formato do fio (ASCII, até [`NAME_MAX_BYTES`]).
    pub name: String,
    /// Threads online, até 255.
    pub threads: u8,
    /// Núcleos físicos online, até 255.
    pub cores: u8,
}

impl CpuInfo {
    /// Sem nenhum metadado: fabricante 0, nome [`FALLBACK_NAME`], zeros.
    pub fn unknown() -> Self {
        Self {
            intel: false,
            name: FALLBACK_NAME.to_owned(),
            threads: 0,
            cores: 0,
        }
    }

    /// Lê `cpuinfo` sob `proc_root` e a topologia online sob `sys_root`.
    /// Nunca falha: o que não puder ser lido vira o valor de [`unknown`].
    ///
    /// [`unknown`]: CpuInfo::unknown
    pub fn read(sys_root: &Path, proc_root: &Path) -> Self {
        let cpuinfo = fs::read_to_string(proc_root.join("cpuinfo")).ok();
        let (intel, name) = cpuinfo.as_deref().map_or((false, None), parse_cpuinfo);
        let cpu_dir = sys_root.join("devices/system/cpu");
        let online = fs::read_to_string(cpu_dir.join("online"))
            .ok()
            .and_then(|t| parse_cpu_list(t.trim()));
        let threads = online.as_ref().map_or(0, |l| saturate(l.len()));
        let cores = online
            .as_ref()
            .and_then(|l| count_cores(&cpu_dir, l))
            .map_or(0, saturate);
        Self {
            intel,
            name: wire_name(name.unwrap_or("")),
            threads,
            cores,
        }
    }
}

fn saturate(n: usize) -> u8 {
    u8::try_from(n).unwrap_or(u8::MAX)
}

/// Fabricante e nome da primeira CPU do `/proc/cpuinfo`.
fn parse_cpuinfo(text: &str) -> (bool, Option<&str>) {
    let field = |key: &str| {
        text.lines().find_map(|line| {
            let (k, v) = line.split_once(':')?;
            (k.trim() == key).then(|| v.trim())
        })
    };
    (
        field("vendor_id") == Some("GenuineIntel"),
        field("model name"),
    )
}

/// Lista do sysfs como `0-5,8,10-11`. `None` se vazia ou malformada.
pub fn parse_cpu_list(text: &str) -> Option<Vec<u32>> {
    let mut cpus = Vec::new();
    for part in text.split(',') {
        let (first, last) = match part.split_once('-') {
            Some((a, b)) => (a.parse::<u32>().ok()?, b.parse::<u32>().ok()?),
            None => {
                let n = part.parse::<u32>().ok()?;
                (n, n)
            }
        };
        // Faixa invertida ou absurda é lista malformada.
        if first > last || last - first > u32::from(u16::MAX) {
            return None;
        }
        cpus.extend(first..=last);
    }
    (!cpus.is_empty()).then_some(cpus)
}

/// Pares únicos de `physical_package_id`/`core_id` das CPUs `online`.
/// `None` se a topologia de alguma faltar, para não contar a menos.
fn count_cores(cpu_dir: &Path, online: &[u32]) -> Option<usize> {
    let read = |cpu: u32, attr: &str| -> Option<i64> {
        fs::read_to_string(cpu_dir.join(format!("cpu{cpu}/topology/{attr}")))
            .ok()?
            .trim()
            .parse()
            .ok()
    };
    let mut cores = BTreeSet::new();
    for &cpu in online {
        cores.insert((read(cpu, "physical_package_id")?, read(cpu, "core_id")?));
    }
    Some(cores.len())
}

/// Nome no formato do fio: sem espaços nas pontas, caractere fora do ASCII
/// visível vira `?`, até [`NAME_MAX_BYTES`]; vazio vira [`FALLBACK_NAME`].
pub fn wire_name(raw: &str) -> String {
    let mut name: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c == ' ' || c.is_ascii_graphic() {
                c
            } else {
                '?'
            }
        })
        .take(NAME_MAX_BYTES)
        .collect();
    name.truncate(name.trim_end().len());
    if name.is_empty() {
        FALLBACK_NAME.to_owned()
    } else {
        name
    }
}

/// `99 E1 <len> <nome>`, com `len` em bytes (`SendCpuName`).
pub fn name_payload(name: &str) -> Vec<u8> {
    let name = wire_name(name);
    let mut payload = vec![0x99, 0xE1, name.len() as u8];
    payload.extend(name.bytes());
    payload
}

/// `99 E0` com a temperatura `celsius` (limitada a [`PROTECT_CELSIUS`]) e os
/// campos de `info`. GHz, VRAM/VRM, líquido, uso e W vão zerados
/// (`SendCoolerData`; matriz em `docs/protocols/castor3.md`).
pub fn data_payload(info: &CpuInfo, celsius: u8) -> Vec<u8> {
    vec![
        0x99,
        0xE0,
        u8::from(info.intel),
        celsius.min(PROTECT_CELSIUS),
        info.threads,
        0,
        0,
        info.cores,
        0,
        0,
        0,
        0,
        0,
    ]
}

/// Temperatura do fio para uma amostra em m°C: arredondada para cima e
/// limitada a [`PROTECT_CELSIUS`]. Sem amostra ou fora de 0 a
/// [`TCTL_MAX_MILLICELSIUS`], a proteção.
pub fn wire_celsius(sample: Option<i32>) -> u8 {
    match sample {
        Some(mc) if (0..=TCTL_MAX_MILLICELSIUS).contains(&mc) => {
            ((mc + 999) / 1000).min(i32::from(PROTECT_CELSIUS)) as u8
        }
        _ => PROTECT_CELSIUS,
    }
}

/// Falha ao achar ou ler o Tctl.
#[derive(Debug)]
pub enum TctlError {
    Io(PathBuf, io::Error),
    /// Nenhum hwmon `k10temp` com canal rotulado `Tctl`.
    NotFound,
    /// Mais de um canal `Tctl`; não há como escolher.
    Ambiguous(Vec<PathBuf>),
    Parse(PathBuf),
    /// Valor fora de 0 a [`TCTL_MAX_MILLICELSIUS`].
    OutOfRange(PathBuf, i64),
}

impl fmt::Display for TctlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(p, e) => write!(f, "erro ao ler {}: {e}", p.display()),
            Self::NotFound => write!(f, "nenhum hwmon {K10TEMP} com canal {TCTL_LABEL}"),
            Self::Ambiguous(paths) => {
                write!(f, "{} canais {TCTL_LABEL} em {K10TEMP}", paths.len())
            }
            Self::Parse(p) => write!(f, "{} não é um número", p.display()),
            Self::OutOfRange(p, v) => write!(
                f,
                "{} informa {v} m°C, fora de 0 a {TCTL_MAX_MILLICELSIUS}",
                p.display()
            ),
        }
    }
}

impl std::error::Error for TctlError {}

/// O `tempN_input` do único canal `Tctl` dos hwmon `k10temp` sob
/// `sys_root`, achado pelo `name` e pelo `tempN_label`, sem índice fixo.
pub fn find_tctl(sys_root: &Path) -> Result<PathBuf, TctlError> {
    let class = sys_root.join("class/hwmon");
    let entries = match fs::read_dir(&class) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(TctlError::NotFound),
        Err(e) => return Err(TctlError::Io(class, e)),
    };
    let mut found = Vec::new();
    for entry in entries {
        let dir = entry.map_err(|e| TctlError::Io(class.clone(), e))?.path();
        let Ok(name) = fs::read_to_string(dir.join("name")) else {
            continue;
        };
        if name.trim() != K10TEMP {
            continue;
        }
        let Ok(attrs) = fs::read_dir(&dir) else {
            continue;
        };
        for attr in attrs.flatten() {
            let file = attr.file_name();
            let Some(index) = file
                .to_str()
                .and_then(|f| f.strip_prefix("temp")?.strip_suffix("_label"))
            else {
                continue;
            };
            let label = fs::read_to_string(attr.path()).unwrap_or_default();
            if label.trim() == TCTL_LABEL {
                found.push(dir.join(format!("temp{index}_input")));
            }
        }
    }
    found.sort();
    match found.len() {
        0 => Err(TctlError::NotFound),
        1 => Ok(found.remove(0)),
        _ => Err(TctlError::Ambiguous(found)),
    }
}

/// Lê um `tempN_input` em m°C e confere a faixa.
pub fn read_tctl_at(path: &Path) -> Result<i32, TctlError> {
    let text = fs::read_to_string(path).map_err(|e| TctlError::Io(path.to_owned(), e))?;
    let value: i64 = text
        .trim()
        .parse()
        .map_err(|_| TctlError::Parse(path.to_owned()))?;
    match i32::try_from(value) {
        Ok(v) if (0..=TCTL_MAX_MILLICELSIUS).contains(&v) => Ok(v),
        _ => Err(TctlError::OutOfRange(path.to_owned(), value)),
    }
}

/// Acha e lê o Tctl. A busca é refeita a cada leitura: o índice do hwmon
/// pode mudar quando um driver é recarregado.
pub fn read_tctl(sys_root: &Path) -> Result<i32, TctlError> {
    read_tctl_at(&find_tctl(sys_root)?)
}

/// Falha de uma operação do feed ou da proteção.
#[derive(Debug)]
pub enum FeedError {
    NotFound,
    Open(OpenError),
    Io(io::Error),
    /// Nenhuma resposta `99 DE` no prazo.
    NoModelResponse,
    /// Modelo conhecido sem alimentação validada (1 = X 280, 2 = X 360).
    NotFed(u8),
    /// `r[2]` do `DE` fora da tabela.
    UnknownModel(u8),
    /// O prazo da operação venceu antes do próximo pacote.
    Deadline,
}

impl fmt::Display for FeedError {
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
            Self::NoModelResponse => write!(
                f,
                "o aparelho não respondeu ao pedido de modelo (DE 00) em 2 s; nenhum E1/E0 enviado"
            ),
            Self::NotFed(m) => write!(
                f,
                "modelo {m} sem alimentação de CPU validada; nenhum E1/E0 enviado"
            ),
            Self::UnknownModel(m) => {
                write!(f, "modelo desconhecido no DE ({m}); nenhum E1/E0 enviado")
            }
            Self::Deadline => write!(f, "prazo da operação vencido antes de concluir o envio"),
        }
    }
}

impl std::error::Error for FeedError {}

fn remaining(deadline: Instant) -> Result<Duration, FeedError> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        Err(FeedError::Deadline)
    } else {
        Ok(left)
    }
}

/// Envia `payload` (com o `0x99`) se o prazo ainda não venceu. Um `write`
/// em andamento não é interrompido; o prazo impede só o próximo pacote.
fn send(device: &impl OutputReports, payload: &[u8], deadline: Instant) -> Result<(), FeedError> {
    remaining(deadline)?;
    device
        .write_output_report(&packet(&payload[1..]))
        .map_err(FeedError::Io)
}

/// `DE 00`, com a resposta esperada até o menor entre 2 s e o prazo.
/// Só o [`FED_MODEL`] segue.
pub fn check_model(
    device: &(impl InputReports + OutputReports),
    deadline: Instant,
) -> Result<u8, FeedError> {
    let query = ReadQuery::Model;
    remaining(deadline)?;
    device
        .write_output_report(&query.packet())
        .map_err(FeedError::Io)?;
    let wait = remaining(deadline)?.min(RESPONSE_TIMEOUT);
    let response = wait_response(device, query.command(), wait)
        .map_err(FeedError::Io)?
        .ok_or_else(|| match remaining(deadline) {
            Ok(_) => FeedError::NoModelResponse,
            Err(e) => e,
        })?;
    let Some(Reading::Model(model)) = query.decode(&response) else {
        return Err(FeedError::NoModelResponse);
    };
    match model {
        FED_MODEL => Ok(model),
        m if model_name(m).is_some() => Err(FeedError::NotFed(m)),
        m => Err(FeedError::UnknownModel(m)),
    }
}

/// Início de sessão e proteção: `DE 00`, `E1` e `E0` de [`PROTECT_CELSIUS`].
pub fn start_session_to(
    device: &(impl InputReports + OutputReports),
    info: &CpuInfo,
    deadline: Instant,
) -> Result<u8, FeedError> {
    let model = check_model(device, deadline)?;
    send(device, &name_payload(&info.name), deadline)?;
    std::thread::sleep(PACKET_INTERVAL);
    send(device, &data_payload(info, PROTECT_CELSIUS), deadline)?;
    Ok(model)
}

/// Um `E0` dentro de uma sessão já iniciada.
pub fn send_data_to(
    device: &impl OutputReports,
    info: &CpuInfo,
    celsius: u8,
    deadline: Instant,
) -> Result<(), FeedError> {
    send(device, &data_payload(info, celsius), deadline)
}

/// Sessão lógica do feed: o aparelho que recebeu `DE`/`E1`. Fechar o hidraw
/// entre envios não a encerra; outra identidade no sysfs, sim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub identity: String,
    pub model: u8,
}

impl Session {
    /// `false` em modelo sem alimentação validada: nada é enviado até a
    /// próxima sessão.
    pub fn fed(&self) -> bool {
        self.model == FED_MODEL
    }
}

/// Identidade do aparelho por trás do hidraw: o nome do nó mais o
/// dispositivo HID do sysfs (como `0003:1044:7A4D.0007`), cujo número muda a
/// cada reconexão mesmo quando o `hidrawN` se repete.
pub fn identity(sys_root: &Path, info: &HidDeviceInfo) -> String {
    let device = sys_root
        .join("class/hidraw")
        .join(&info.name)
        .join("device");
    match fs::canonicalize(&device)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
    {
        Some(hid) => format!("{}:{hid}", info.name),
        None => info.name.clone(),
    }
}

/// Resultado de um envio do feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fed {
    /// Sessão nova: `DE`, `E1` e `E0` de proteção. Quem chama começa a
    /// janela segura.
    Started(u8),
    /// Um `E0` com a temperatura pedida.
    Data,
    /// Modelo sem alimentação: nada enviado nesta sessão.
    Disabled(u8),
}

/// Um envio na sessão `session`, num aparelho já aberto. Sem sessão, inicia
/// uma; com sessão, manda o `E0` de `celsius`. Erro de transporte encerra a
/// sessão, e o próximo envio recomeça pelo `DE`.
pub fn feed_to(
    device: &(impl InputReports + OutputReports),
    identity: &str,
    session: &mut Option<Session>,
    info: &CpuInfo,
    celsius: u8,
    deadline: Instant,
) -> Result<Fed, FeedError> {
    if session.as_ref().is_some_and(|s| s.identity != identity) {
        *session = None;
    }
    if let Some(s) = session.as_ref() {
        if !s.fed() {
            return Ok(Fed::Disabled(s.model));
        }
        let sent = send_data_to(device, info, celsius, deadline);
        if sent.is_err() {
            *session = None;
        }
        return sent.map(|()| Fed::Data);
    }
    let (model, fed) = match start_session_to(device, info, deadline) {
        Ok(m) => (m, Fed::Started(m)),
        Err(FeedError::NotFed(m) | FeedError::UnknownModel(m)) => (m, Fed::Disabled(m)),
        Err(e) => return Err(e),
    };
    *session = Some(Session {
        identity: identity.to_owned(),
        model,
    });
    Ok(fed)
}

/// Acha o Waterforce sob `sys_root`, abre (esperando o `flock` até o menor
/// entre 5 s e o prazo), faz o [`feed_to`] e fecha, soltando o lock. Numa
/// sessão sem alimentação da mesma identidade, nem abre o nó.
pub fn feed(
    sys_root: &Path,
    session: &mut Option<Session>,
    info: &CpuInfo,
    celsius: u8,
    deadline: Instant,
) -> Result<Fed, FeedError> {
    let dev = find_hidraw(sys_root)
        .map_err(FeedError::Io)?
        .ok_or(FeedError::NotFound)?;
    let id = identity(sys_root, &dev);
    if let Some(s) = session.as_ref().filter(|s| s.identity == id && !s.fed()) {
        return Ok(Fed::Disabled(s.model));
    }
    let wait = remaining(deadline)?.min(LOCK_TIMEOUT);
    let device = Hidraw::open_with_timeout(&dev.devnode, wait).map_err(FeedError::Open)?;
    feed_to(&device, &id, session, info, celsius, deadline)
}

/// Resultado da proteção.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protected {
    /// `DE`, `E1` e `E0` de 80 °C enviados ao modelo.
    Sent(u8),
    /// Nenhum Waterforce.
    Absent,
    /// Modelo 1 ou 2: o feed nunca roda nele, nada a proteger.
    NotFed(u8),
}

/// Proteção num aparelho aberto: o início de sessão, sem sessão anterior.
/// Repetir manda os mesmos pacotes.
pub fn protect_to(
    device: &(impl InputReports + OutputReports),
    info: &CpuInfo,
    deadline: Instant,
) -> Result<Protected, FeedError> {
    match start_session_to(device, info, deadline) {
        Ok(m) => Ok(Protected::Sent(m)),
        Err(FeedError::NotFed(m)) => Ok(Protected::NotFed(m)),
        Err(e) => Err(e),
    }
}

/// Acha, abre e protege. Sem aparelho (ou nó sumido antes do `open`),
/// [`Protected::Absent`].
pub fn protect(sys_root: &Path, info: &CpuInfo, deadline: Instant) -> Result<Protected, FeedError> {
    let Some(dev) = find_hidraw(sys_root).map_err(FeedError::Io)? else {
        return Ok(Protected::Absent);
    };
    let wait = remaining(deadline)?.min(LOCK_TIMEOUT);
    let device = match Hidraw::open_with_timeout(&dev.devnode, wait) {
        Ok(d) => d,
        Err(OpenError::NotFound(_)) => return Ok(Protected::Absent),
        Err(e) => return Err(FeedError::Open(e)),
    };
    protect_to(&device, info, deadline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::waterforce::led::tests::FakeDevice;
    use crate::waterforce::probe::OUT_SIZE;
    use std::cell::Cell;

    const DE_X240: &[u8] = &[0x99, 0xDE, 0x00];

    fn ryzen() -> CpuInfo {
        CpuInfo {
            intel: false,
            name: "Ryzen 5 5600G".into(),
            threads: 12,
            cores: 6,
        }
    }

    fn later(ms: u64) -> Instant {
        Instant::now() + Duration::from_millis(ms)
    }

    #[test]
    fn e0_byte_a_byte_como_no_ensaio() {
        // experiment1.jsonl: `99 e0 00 50 0c 00 00 06 00 00 00 00 00`.
        assert_eq!(
            data_payload(&ryzen(), 80),
            [
                0x99, 0xE0, 0x00, 0x50, 0x0C, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00
            ]
        );
        assert_eq!(data_payload(&ryzen(), 30)[3], 0x1E);
    }

    #[test]
    fn e0_intel_e_saturacao() {
        let info = CpuInfo {
            intel: true,
            name: "x".into(),
            threads: 255,
            cores: 255,
        };
        let p = data_payload(&info, 200);
        assert_eq!(p[2], 1);
        assert_eq!(p[3], 80);
        assert_eq!((p[4], p[7]), (255, 255));
        // GHz, VRAM/VRM, líquido, uso e W zerados.
        assert!([5, 6, 8, 9, 10, 11, 12].iter().all(|&i| p[i] == 0));
    }

    #[test]
    fn e0_sem_metadados() {
        assert_eq!(
            data_payload(&CpuInfo::unknown(), 80),
            [0x99, 0xE0, 0, 80, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn e1_byte_a_byte_como_no_ensaio() {
        let mut expected = vec![0x99, 0xE1, 0x0D];
        expected.extend(b"Ryzen 5 5600G");
        assert_eq!(name_payload("Ryzen 5 5600G"), expected);
    }

    #[test]
    fn e1_nome_nao_ascii_truncado_e_vazio() {
        assert_eq!(wire_name("  Ryzen™ 5\t"), "Ryzen? 5");
        let long = "A".repeat(100);
        let p = name_payload(&long);
        assert_eq!(p[2] as usize, NAME_MAX_BYTES);
        assert_eq!(p.len(), 3 + NAME_MAX_BYTES);
        // Comprimento em bytes, mesmo com caractere de vários bytes trocado.
        assert_eq!(name_payload("é")[2..], [1, b'?']);
        assert_eq!(name_payload("   ")[2..], [3, b'C', b'P', b'U']);
        assert_eq!(wire_name(&format!("{} B", "A".repeat(29))), "A".repeat(29));
    }

    #[test]
    fn temperatura_do_fio() {
        assert_eq!(wire_celsius(Some(0)), 0);
        assert_eq!(wire_celsius(Some(40_001)), 41);
        assert_eq!(wire_celsius(Some(40_000)), 40);
        assert_eq!(wire_celsius(Some(79_001)), 80);
        assert_eq!(wire_celsius(Some(85_000)), 80);
        assert_eq!(wire_celsius(Some(125_000)), 80);
        assert_eq!(wire_celsius(Some(125_001)), 80);
        assert_eq!(wire_celsius(Some(-1)), 80);
        assert_eq!(wire_celsius(None), 80);
    }

    /// sysfs falso com hwmons.
    struct Sys(tempfile::TempDir);

    impl Sys {
        fn new() -> Self {
            Self(tempfile::tempdir().unwrap())
        }

        fn hwmon(&self, n: u32, name: &str, attrs: &[(&str, &str)]) {
            let dir = self.0.path().join(format!("class/hwmon/hwmon{n}"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("name"), format!("{name}\n")).unwrap();
            for (attr, value) in attrs {
                fs::write(dir.join(attr), format!("{value}\n")).unwrap();
            }
        }

        fn path(&self) -> &Path {
            self.0.path()
        }
    }

    #[test]
    fn tctl_por_nome_e_rotulo_em_qualquer_indice() {
        let sys = Sys::new();
        sys.hwmon(
            0,
            "waterforce",
            &[("temp1_label", "Tctl"), ("temp1_input", "1")],
        );
        sys.hwmon(
            7,
            "k10temp",
            &[
                ("temp1_label", "Tdie"),
                ("temp1_input", "99000"),
                ("temp3_label", "Tctl"),
                ("temp3_input", "41250"),
            ],
        );
        let path = find_tctl(sys.path()).unwrap();
        assert!(path.ends_with("hwmon7/temp3_input"), "{path:?}");
        assert_eq!(read_tctl(sys.path()).unwrap(), 41_250);
    }

    #[test]
    fn tctl_zero_e_valido_e_limites() {
        let sys = Sys::new();
        sys.hwmon(
            2,
            "k10temp",
            &[("temp1_label", "Tctl"), ("temp1_input", "0")],
        );
        assert_eq!(read_tctl(sys.path()).unwrap(), 0);
        let input = sys.path().join("class/hwmon/hwmon2/temp1_input");
        fs::write(&input, "125000").unwrap();
        assert_eq!(read_tctl(sys.path()).unwrap(), 125_000);
        fs::write(&input, "125001").unwrap();
        assert!(matches!(
            read_tctl(sys.path()),
            Err(TctlError::OutOfRange(_, 125_001))
        ));
        fs::write(&input, "-5").unwrap();
        assert!(matches!(
            read_tctl(sys.path()),
            Err(TctlError::OutOfRange(_, -5))
        ));
        fs::write(&input, "quente").unwrap();
        assert!(matches!(read_tctl(sys.path()), Err(TctlError::Parse(_))));
        fs::remove_file(&input).unwrap();
        assert!(matches!(read_tctl(sys.path()), Err(TctlError::Io(..))));
        // A falha vira proteção no fio.
        assert_eq!(wire_celsius(read_tctl(sys.path()).ok()), 80);
    }

    #[test]
    fn tctl_ausente_sem_fallback() {
        let sys = Sys::new();
        assert!(matches!(read_tctl(sys.path()), Err(TctlError::NotFound)));
        // Só Tdie, líquido ou outro sensor: nada serve.
        sys.hwmon(
            1,
            "k10temp",
            &[("temp2_label", "Tdie"), ("temp2_input", "40000")],
        );
        sys.hwmon(3, "waterforce", &[("temp1_input", "30000")]);
        sys.hwmon(4, "gigabyte_wmi", &[("temp1_input", "35000")]);
        assert!(matches!(read_tctl(sys.path()), Err(TctlError::NotFound)));
    }

    #[test]
    fn tctl_ambiguo() {
        let sys = Sys::new();
        sys.hwmon(
            1,
            "k10temp",
            &[("temp1_label", "Tctl"), ("temp1_input", "40000")],
        );
        sys.hwmon(
            2,
            "k10temp",
            &[("temp1_label", "Tctl"), ("temp1_input", "50000")],
        );
        assert!(matches!(
            read_tctl(sys.path()),
            Err(TctlError::Ambiguous(ref p)) if p.len() == 2
        ));
    }

    fn cpu_sys(online: &str, topo: &[(u32, i64, i64)]) -> Sys {
        let sys = Sys::new();
        let cpu = sys.path().join("devices/system/cpu");
        fs::create_dir_all(&cpu).unwrap();
        fs::write(cpu.join("online"), format!("{online}\n")).unwrap();
        for &(n, pkg, core) in topo {
            let t = cpu.join(format!("cpu{n}/topology"));
            fs::create_dir_all(&t).unwrap();
            fs::write(t.join("physical_package_id"), format!("{pkg}\n")).unwrap();
            fs::write(t.join("core_id"), format!("{core}\n")).unwrap();
        }
        sys
    }

    #[test]
    fn metadados_do_cpuinfo_e_topologia() {
        // 12 threads em 6 núcleos, SMT nos pares (n, n+6).
        let topo: Vec<_> = (0..12).map(|n| (n, 0, i64::from(n % 6))).collect();
        let sys = cpu_sys("0-11", &topo);
        let proc = tempfile::tempdir().unwrap();
        fs::write(
            proc.path().join("cpuinfo"),
            "processor\t: 0\nvendor_id\t: AuthenticAMD\nmodel name\t: AMD Ryzen 5 5600G with Radeon Graphics\n\nprocessor\t: 1\nvendor_id\t: AuthenticAMD\n",
        )
        .unwrap();
        let info = CpuInfo::read(sys.path(), proc.path());
        assert_eq!(
            info,
            CpuInfo {
                intel: false,
                // 38 caracteres no cpuinfo; o fio leva 30, sem o espaço final.
                name: "AMD Ryzen 5 5600G with Radeon".into(),
                threads: 12,
                cores: 6,
            }
        );
    }

    #[test]
    fn metadados_intel_lista_com_buracos() {
        let sys = cpu_sys("0,2-3", &[(0, 0, 0), (2, 0, 2), (3, 1, 0)]);
        let proc = tempfile::tempdir().unwrap();
        fs::write(
            proc.path().join("cpuinfo"),
            "vendor_id : GenuineIntel\nmodel name : Intel(R) Core(TM) i5\n",
        )
        .unwrap();
        let info = CpuInfo::read(sys.path(), proc.path());
        assert!(info.intel);
        assert_eq!((info.threads, info.cores), (3, 3));
    }

    #[test]
    fn metadados_ausentes_viram_zeros() {
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(
            CpuInfo::read(empty.path(), empty.path()),
            CpuInfo::unknown()
        );
        // Topologia faltando numa CPU: núcleos indisponíveis, threads não.
        let sys = cpu_sys("0-1", &[(0, 0, 0)]);
        let info = CpuInfo::read(sys.path(), empty.path());
        assert_eq!((info.threads, info.cores), (2, 0));
    }

    #[test]
    fn lista_de_cpus() {
        assert_eq!(parse_cpu_list("0-3,8"), Some(vec![0, 1, 2, 3, 8]));
        assert_eq!(parse_cpu_list("5"), Some(vec![5]));
        assert_eq!(parse_cpu_list(""), None);
        assert_eq!(parse_cpu_list("3-1"), None);
        assert_eq!(parse_cpu_list("0-x"), None);
        assert_eq!(parse_cpu_list("0-4294967295"), None);
        let many = format!("0-{}", 299);
        let sys = cpu_sys(&many, &[]);
        let info = CpuInfo::read(sys.path(), sys.path());
        assert_eq!(info.threads, 255);
    }

    fn assert_no_config(dev: &FakeDevice) {
        for p in dev.written.borrow().iter() {
            assert_eq!(p.len(), OUT_SIZE);
            assert!(
                matches!(p[1], 0xDE | 0xE0 | 0xE1),
                "pacote fora do feed: {:02X}",
                p[1]
            );
        }
    }

    fn e1_ryzen() -> Vec<u8> {
        name_payload("Ryzen 5 5600G")
    }

    #[test]
    fn inicio_de_sessao_de_e1_e0_alto() {
        let dev = FakeDevice::new(&[&[0x99, 0xDA, 0x00], DE_X240]);
        let mut session = None;
        let fed = feed_to(&dev, "hidraw3:A", &mut session, &ryzen(), 35, later(5000)).unwrap();
        assert_eq!(fed, Fed::Started(0));
        // A temperatura pedida não vale no início: sempre 80.
        dev.assert_written(&[DE_X240, &e1_ryzen(), &data_payload(&ryzen(), 80)]);
        assert_no_config(&dev);
        assert_eq!(
            session,
            Some(Session {
                identity: "hidraw3:A".into(),
                model: 0
            })
        );
    }

    #[test]
    fn sessao_preservada_ao_reabrir() {
        let mut session = Some(Session {
            identity: "hidraw3:A".into(),
            model: 0,
        });
        // Um fd novo, sem DE pendente: só o E0.
        let dev = FakeDevice::new(&[]);
        let fed = feed_to(&dev, "hidraw3:A", &mut session, &ryzen(), 41, later(5000)).unwrap();
        assert_eq!(fed, Fed::Data);
        dev.assert_written(&[&data_payload(&ryzen(), 41)]);
    }

    #[test]
    fn identidade_nova_reinicia_a_sessao() {
        let mut session = Some(Session {
            identity: "hidraw3:A".into(),
            model: 0,
        });
        let dev = FakeDevice::new(&[DE_X240]);
        let fed = feed_to(&dev, "hidraw3:B", &mut session, &ryzen(), 41, later(5000)).unwrap();
        assert_eq!(fed, Fed::Started(0));
        assert_eq!(dev.written.borrow().len(), 3);
        assert_eq!(session.unwrap().identity, "hidraw3:B");
    }

    #[test]
    fn modelos_sem_feed_nao_recebem_e1_e0() {
        for (model, expected) in [
            (1u8, Fed::Disabled(1)),
            (2, Fed::Disabled(2)),
            (5, Fed::Disabled(5)),
        ] {
            let dev = FakeDevice::new(&[&[0x99, 0xDE, model]]);
            let mut session = None;
            let fed = feed_to(&dev, "id", &mut session, &ryzen(), 40, later(5000)).unwrap();
            assert_eq!(fed, expected);
            dev.assert_written(&[DE_X240]);
            // Na mesma sessão, nada é enviado.
            let again = FakeDevice::new(&[]);
            let fed = feed_to(&again, "id", &mut session, &ryzen(), 40, later(5000)).unwrap();
            assert_eq!(fed, expected);
            assert!(again.written.borrow().is_empty());
        }
    }

    #[test]
    fn sem_resposta_ao_de() {
        let dev = FakeDevice::new(&[]);
        let mut session = None;
        let err = feed_to(&dev, "id", &mut session, &ryzen(), 40, later(5000)).unwrap_err();
        assert!(matches!(err, FeedError::NoModelResponse), "{err:?}");
        dev.assert_written(&[DE_X240]);
        assert_eq!(session, None);
    }

    /// Dispositivo cujas escritas demoram ou falham.
    struct Slow {
        inner: FakeDevice,
        delay: Duration,
        fail_at: Option<usize>,
        writes: Cell<usize>,
    }

    impl InputReports for Slow {
        fn read_input_report(&self, buf: &mut [u8], t: Duration) -> io::Result<Option<usize>> {
            self.inner.read_input_report(buf, t)
        }
    }

    impl OutputReports for Slow {
        fn write_output_report(&self, data: &[u8]) -> io::Result<()> {
            let n = self.writes.get();
            self.writes.set(n + 1);
            std::thread::sleep(self.delay);
            if self.fail_at == Some(n) {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "output report curto: 100 de 6144 bytes",
                ));
            }
            self.inner.write_output_report(data)
        }
    }

    #[test]
    fn write_parcial_encerra_a_sessao() {
        let dev = Slow {
            inner: FakeDevice::new(&[]),
            delay: Duration::ZERO,
            fail_at: Some(0),
            writes: Cell::new(0),
        };
        let mut session = Some(Session {
            identity: "id".into(),
            model: 0,
        });
        let err = feed_to(&dev, "id", &mut session, &ryzen(), 30, later(5000)).unwrap_err();
        assert!(matches!(err, FeedError::Io(_)));
        // O próximo envio recomeça pelo DE, com E0 alto.
        assert_eq!(session, None);
    }

    #[test]
    fn prazo_para_antes_do_proximo_pacote() {
        // Cada write leva 80 ms e o prazo é de 120 ms: DE e E1 saem, o E0
        // não, e a operação termina sem esperar o resto da sequência.
        let dev = Slow {
            inner: FakeDevice::new(&[DE_X240]),
            delay: Duration::from_millis(80),
            fail_at: None,
            writes: Cell::new(0),
        };
        let started = Instant::now();
        let err = protect_to(&dev, &ryzen(), later(120)).unwrap_err();
        assert!(matches!(err, FeedError::Deadline), "{err:?}");
        assert!(started.elapsed() < Duration::from_millis(400));
        let written = dev.inner.written.borrow();
        assert!(written.iter().all(|p| p[1] != 0xE0));
    }

    #[test]
    fn prazo_vencido_nao_envia_nada() {
        let dev = FakeDevice::new(&[DE_X240]);
        let err = protect_to(&dev, &ryzen(), Instant::now()).unwrap_err();
        assert!(matches!(err, FeedError::Deadline));
        assert!(dev.written.borrow().is_empty());
    }

    #[test]
    fn protecao_repetida_e_identica() {
        let first = FakeDevice::new(&[DE_X240]);
        let second = FakeDevice::new(&[DE_X240]);
        assert_eq!(
            protect_to(&first, &CpuInfo::unknown(), later(5000)).unwrap(),
            Protected::Sent(0)
        );
        assert_eq!(
            protect_to(&second, &CpuInfo::unknown(), later(5000)).unwrap(),
            Protected::Sent(0)
        );
        assert_eq!(*first.written.borrow(), *second.written.borrow());
        let e0 = &first.written.borrow()[2];
        assert_eq!(e0[1..4], [0xE0, 0, 80]);
        assert_no_config(&first);
    }

    #[test]
    fn protecao_por_modelo() {
        let x280 = FakeDevice::new(&[&[0x99, 0xDE, 0x01]]);
        assert_eq!(
            protect_to(&x280, &ryzen(), later(5000)).unwrap(),
            Protected::NotFed(1)
        );
        x280.assert_written(&[DE_X240]);
        let odd = FakeDevice::new(&[&[0x99, 0xDE, 0x07]]);
        assert!(matches!(
            protect_to(&odd, &ryzen(), later(5000)),
            Err(FeedError::UnknownModel(7))
        ));
    }

    /// sysfs com um Castor3 em `hidraw{n}`, ligado ao HID `hid`.
    fn castor(sys: &Path, n: u32, hid: &str) {
        let real = sys.join("devices").join(hid);
        fs::create_dir_all(&real).unwrap();
        fs::write(
            real.join("uevent"),
            "DRIVER=hid-generic\nHID_ID=0003:00001044:00007A4D\n",
        )
        .unwrap();
        // Usage page 0x0000 numa coleção de topo.
        fs::write(
            real.join("report_descriptor"),
            [0x05, 0x00, 0x09, 0x01, 0xA1, 0x01, 0xC0],
        )
        .unwrap();
        let class = sys.join(format!("class/hidraw/hidraw{n}"));
        let _ = fs::remove_dir_all(&class);
        fs::create_dir_all(&class).unwrap();
        std::os::unix::fs::symlink(&real, class.join("device")).unwrap();
    }

    #[test]
    fn identidade_muda_na_reconexao_com_o_mesmo_hidraw() {
        let sys = tempfile::tempdir().unwrap();
        castor(sys.path(), 250, "0003:1044:7A4D.0005");
        let info = find_hidraw(sys.path()).unwrap().unwrap();
        let first = identity(sys.path(), &info);
        assert_eq!(first, "hidraw250:0003:1044:7A4D.0005");
        castor(sys.path(), 250, "0003:1044:7A4D.0009");
        assert_ne!(identity(sys.path(), &info), first);
    }

    #[test]
    fn protecao_sem_aparelho() {
        let sys = tempfile::tempdir().unwrap();
        assert_eq!(
            protect(sys.path(), &ryzen(), later(5000)).unwrap(),
            Protected::Absent
        );
    }

    #[test]
    fn feed_sem_aparelho() {
        let sys = tempfile::tempdir().unwrap();
        let mut session = None;
        assert!(matches!(
            feed(sys.path(), &mut session, &ryzen(), 40, later(5000)),
            Err(FeedError::NotFound)
        ));
    }

    #[test]
    fn sessao_desabilitada_nem_abre_o_no() {
        // O devnode é /dev/hidraw250, que não existe: chegar ao open daria
        // erro. Número alto de propósito, para nunca abrir um aparelho real.
        let sys = tempfile::tempdir().unwrap();
        castor(sys.path(), 250, "0003:1044:7A4D.0005");
        let info = find_hidraw(sys.path()).unwrap().unwrap();
        let mut session = Some(Session {
            identity: identity(sys.path(), &info),
            model: 2,
        });
        assert_eq!(
            feed(sys.path(), &mut session, &ryzen(), 40, later(5000)).unwrap(),
            Fed::Disabled(2)
        );
    }

    #[test]
    fn permissao_e_ocupado_no_open() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let node = dir.path().join("hidraw250");
        fs::write(&node, b"").unwrap();
        let _held = Hidraw::open(&node).unwrap();
        let Err(busy) = Hidraw::open_with_timeout(&node, Duration::from_millis(60)) else {
            panic!("segundo open devia ficar ocupado");
        };
        assert!(matches!(busy, OpenError::Busy(_)));
        assert!(FeedError::Open(busy).to_string().contains("ocupado"));

        let closed = dir.path().join("hidraw251");
        fs::write(&closed, b"").unwrap();
        fs::set_permissions(&closed, fs::Permissions::from_mode(0o000)).unwrap();
        // Como root o modo não impede o open; o teste vale para o usuário.
        if let Err(e) = Hidraw::open(&closed) {
            assert!(matches!(e, OpenError::PermissionDenied(_)));
            assert!(FeedError::Open(e).to_string().contains("regra udev"));
        }
    }

    #[test]
    fn lock_liberado_entre_transacoes() {
        // Uma transação do feed num nó de verdade (arquivo comum) segura o
        // flock só enquanto envia; a CLI abre logo depois, sem esperar.
        let dir = tempfile::tempdir().unwrap();
        let node = dir.path().join("hidraw250");
        fs::write(&node, b"").unwrap();
        let mut session = Some(Session {
            identity: "id".into(),
            model: 0,
        });
        {
            let device = Hidraw::open(&node).unwrap();
            let fed = feed_to(&device, "id", &mut session, &ryzen(), 40, later(5000)).unwrap();
            assert_eq!(fed, Fed::Data);
        }
        let started = Instant::now();
        let cli = Hidraw::open_with_timeout(&node, Duration::from_secs(1));
        assert!(cli.is_ok());
        assert!(started.elapsed() < Duration::from_millis(100));
        // Com a CLI segurando o nó, o feed espera e desiste com Busy.
        let held = cli.unwrap();
        let Err(busy) = Hidraw::open_with_timeout(&node, Duration::from_millis(80)) else {
            panic!("o feed devia achar o nó ocupado");
        };
        assert!(matches!(busy, OpenError::Busy(_)));
        drop(held);
        assert_eq!(fs::read(&node).unwrap().len(), OUT_SIZE);
    }

    #[test]
    fn mensagens() {
        assert_eq!(
            FeedError::NotFed(1).to_string(),
            "modelo 1 sem alimentação de CPU validada; nenhum E1/E0 enviado"
        );
    }
}
