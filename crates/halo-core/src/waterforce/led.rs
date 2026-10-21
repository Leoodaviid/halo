// SPDX-License-Identifier: GPL-2.0-or-later

//! Anel e fans RGB do Castor3, no layout da `GcLedLib.dll` do GCC 26.09.10.01.
//!
//! Só estes pacotes existem aqui (`docs/protocols/castor3.md`, "Anel e RGB das
//! fans"):
//! - `99 C9 <efeito> <brilho×10> <velocidade×2> <índice> <área>`: `FUN_180040fc0`;
//! - `99 CD R G B`: `FUN_180041470`;
//! - `99 B0`–`B3 <efeito> RGB RGB`: `FUN_1800410d0`;
//! - `99 B6`: `FUN_180040780`.
//!
//! A sequência de cada efeito segue o `switch` de `FUN_1800402a0`, com
//! `nGlobalClr = 1` e `nArea = 1`, como o C# do GCC manda. O OpenRGB troca o
//! brilho e a velocidade no `C9` e não é usado aqui.
//!
//! Antes de escrever, o envio confere o modelo com `99 DE 00`.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use super::probe::{
    InputReports, RESPONSE_TIMEOUT, ReadQuery, Reading, model_name, packet, wait_response,
};
use super::{PRODUCT_ID, UDEV_RULE, VENDOR_ID, find_hidraw};
use crate::hid::{Hidraw, OpenError};

/// Efeitos da interface do X 240 (`GLed_0x4016_240.LedEffectId`), com o
/// número de fio do `LedEffectConvert`, e o desligado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Static,
    Pulse,
    Cycle,
    Flash,
    DoubleFlash,
    Gradient,
    ColorShift,
    Wave,
    Rainbow,
    TriColor,
    Spin,
    Switch,
    Lego,
    Aurora,
    /// Estático com brilho 0, como o GCC manda com `On = false`.
    Off,
}

/// Que cores um efeito aceita.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorUse {
    /// Nenhuma (só `C9`).
    None,
    /// Uma cor, no `CD`.
    Single,
    /// Paleta de até 8 cores, no `B0`–`B3`.
    Palette,
}

impl Effect {
    pub const ALL: [Self; 15] = [
        Self::Static,
        Self::Pulse,
        Self::Cycle,
        Self::Flash,
        Self::DoubleFlash,
        Self::Gradient,
        Self::ColorShift,
        Self::Wave,
        Self::Rainbow,
        Self::TriColor,
        Self::Spin,
        Self::Switch,
        Self::Lego,
        Self::Aurora,
        Self::Off,
    ];

    pub const NAMES: [&'static str; 15] = [
        "static",
        "pulse",
        "cycle",
        "flash",
        "double-flash",
        "gradient",
        "color-shift",
        "wave",
        "rainbow",
        "tri-color",
        "spin",
        "switch",
        "lego",
        "aurora",
        "off",
    ];

    pub fn name(self) -> &'static str {
        let i = Self::ALL.iter().position(|&e| e == self).unwrap();
        Self::NAMES[i]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let i = Self::NAMES.iter().position(|&n| n == name)?;
        Some(Self::ALL[i])
    }

    /// `nType` no fio.
    pub fn wire(self) -> u8 {
        match self {
            Self::Static | Self::Off => 0x01,
            Self::Pulse => 0x02,
            Self::Cycle => 0x03,
            Self::Flash => 0x04,
            Self::DoubleFlash => 0x05,
            Self::Gradient => 0x06,
            Self::ColorShift => 0x07,
            Self::Wave => 0x08,
            Self::Rainbow => 0x0A,
            Self::TriColor => 0x0B,
            Self::Spin => 0x0C,
            Self::Switch => 0x0D,
            Self::Lego => 0x0E,
            Self::Aurora => 0x0F,
        }
    }

    pub fn color_use(self) -> ColorUse {
        match self {
            Self::Static | Self::Pulse | Self::Flash | Self::DoubleFlash | Self::Gradient => {
                ColorUse::Single
            }
            Self::Cycle | Self::Wave | Self::Rainbow | Self::Off => ColorUse::None,
            Self::ColorShift
            | Self::TriColor
            | Self::Spin
            | Self::Switch
            | Self::Lego
            | Self::Aurora => ColorUse::Palette,
        }
    }

    /// Cores padrão do GCC (`CoolerLedProperty.InitLedProperty`). No
    /// gradiente, a primeira da lista do seletor (`uc_GradientColor`). Nos
    /// efeitos sem cor, a lista é vazia porque nada é enviado.
    pub fn default_colors(self) -> &'static [Rgb] {
        const ORANGE: &[Rgb] = &[Rgb(0xFF, 0x66, 0x00)];
        const RAINBOW8: &[Rgb] = &[
            Rgb(0xFF, 0x00, 0x00),
            Rgb(0xFF, 0x72, 0x00),
            Rgb(0xFF, 0xFF, 0x00),
            Rgb(0x00, 0xFF, 0x00),
            Rgb(0x00, 0xFF, 0xFF),
            Rgb(0x00, 0x00, 0xFF),
            Rgb(0xFF, 0x00, 0xFF),
            Rgb(0xFF, 0x80, 0x80),
        ];
        const WARM3: &[Rgb] = &[
            Rgb(0xFF, 0x00, 0x00),
            Rgb(0xFF, 0x72, 0x00),
            Rgb(0xFF, 0xFF, 0x00),
        ];
        match self {
            Self::Static | Self::Pulse | Self::Flash | Self::DoubleFlash => ORANGE,
            Self::Gradient => &[Rgb(0xFF, 0x00, 0x00)],
            Self::ColorShift => RAINBOW8,
            // Primeiro grupo do seletor de três cores.
            Self::TriColor => &[
                Rgb(0x00, 0x00, 0xFF),
                Rgb(0x7D, 0x00, 0xFF),
                Rgb(0xFF, 0x00, 0xFF),
            ],
            Self::Spin | Self::Lego | Self::Aurora => WARM3,
            Self::Switch => &[Rgb(0xFF, 0x00, 0x00), Rgb(0xFF, 0x72, 0x00)],
            Self::Cycle | Self::Wave | Self::Rainbow | Self::Off => &[],
        }
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl FromStr for Rgb {
    type Err = LedError;

    /// `RRGGBB`, com ou sem `#` na frente.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        let bad = || LedError::BadColor(s.to_owned());
        if hex.len() != 6 || !hex.is_ascii() {
            return Err(bad());
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| bad());
        Ok(Self(byte(0)?, byte(2)?, byte(4)?))
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// Brilho de 1 a 10, como o `uc_Brightness` do GCC. No fio vai `× 10`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Brightness(u8);

impl Brightness {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 10;

    pub fn new(value: u8) -> Result<Self, LedError> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(LedError::BrightnessOutOfRange(value))
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

impl Default for Brightness {
    fn default() -> Self {
        Self(Self::MAX)
    }
}

/// Velocidade de 1 a 5, como o `uc_Speed` do GCC. No fio vai `× 2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Speed(u8);

impl Speed {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 5;

    pub fn new(value: u8) -> Result<Self, LedError> {
        if (Self::MIN..=Self::MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(LedError::SpeedOutOfRange(value))
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

impl Default for Speed {
    fn default() -> Self {
        Self(Self::MAX)
    }
}

/// Máximo de cores da paleta (`lFan1Clr[8]`).
pub const PALETTE_SIZE: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedError {
    BadColor(String),
    BrightnessOutOfRange(u8),
    SpeedOutOfRange(u8),
    TooManyColors(usize),
    ColorNotAccepted(Effect),
    SingleColorOnly(Effect, usize),
    BrightnessNotAccepted(Effect),
}

impl fmt::Display for LedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadColor(s) => write!(f, "cor inválida `{s}`: use RRGGBB em hexadecimal"),
            Self::BrightnessOutOfRange(v) => write!(
                f,
                "brilho {v} fora da faixa: use de {} a {}",
                Brightness::MIN,
                Brightness::MAX
            ),
            Self::SpeedOutOfRange(v) => write!(
                f,
                "velocidade {v} fora da faixa: use de {} a {}",
                Speed::MIN,
                Speed::MAX
            ),
            Self::TooManyColors(n) => {
                write!(f, "{n} cores: a paleta aceita no máximo {PALETTE_SIZE}")
            }
            Self::ColorNotAccepted(e) => write!(f, "o efeito {e} não aceita cor"),
            Self::SingleColorOnly(e, n) => {
                write!(f, "o efeito {e} aceita uma cor só, e foram informadas {n}")
            }
            Self::BrightnessNotAccepted(e) => write!(f, "o efeito {e} não aceita brilho"),
        }
    }
}

impl std::error::Error for LedError {}

/// Efeito validado, pronto para virar pacotes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedSetting {
    effect: Effect,
    brightness: Brightness,
    speed: Speed,
    /// Vazia nos efeitos sem cor; uma cor nos de cor única; 8 cores nos de
    /// paleta.
    colors: Vec<Rgb>,
}

impl LedSetting {
    /// Valida as entradas. Sem `colors`, usa as cores padrão do GCC. Uma
    /// paleta curta é completada repetindo a última cor.
    pub fn new(
        effect: Effect,
        colors: &[Rgb],
        brightness: Option<u8>,
        speed: Option<u8>,
    ) -> Result<Self, LedError> {
        if effect == Effect::Off && brightness.is_some() {
            return Err(LedError::BrightnessNotAccepted(effect));
        }
        let brightness = brightness
            .map(Brightness::new)
            .transpose()?
            .unwrap_or_default();
        let speed = speed.map(Speed::new).transpose()?.unwrap_or_default();

        let given = if colors.is_empty() {
            effect.default_colors()
        } else {
            colors
        };
        let colors = match effect.color_use() {
            ColorUse::None if !colors.is_empty() => {
                return Err(LedError::ColorNotAccepted(effect));
            }
            ColorUse::None => Vec::new(),
            ColorUse::Single if given.len() > 1 => {
                return Err(LedError::SingleColorOnly(effect, given.len()));
            }
            ColorUse::Single => given.to_vec(),
            ColorUse::Palette if given.len() > PALETTE_SIZE => {
                return Err(LedError::TooManyColors(given.len()));
            }
            ColorUse::Palette => {
                let mut palette = given.to_vec();
                let last = *palette.last().unwrap();
                palette.resize(PALETTE_SIZE, last);
                palette
            }
        };

        Ok(Self {
            effect,
            brightness,
            speed,
            colors,
        })
    }

    pub fn effect(&self) -> Effect {
        self.effect
    }

    pub fn colors(&self) -> &[Rgb] {
        &self.colors
    }

    /// Sequência do efeito, cada pacote com o report ID `0x99` e sem o
    /// preenchimento até 6144 bytes. Termina sempre com um único `B6`.
    pub fn packets(&self) -> Vec<Vec<u8>> {
        let t = self.effect.wire();
        let b = match self.effect {
            Effect::Off => 0,
            _ => self.brightness.get() * 10,
        };
        let s = self.speed.get() * 2;
        let c9 = |index: u8, area: u8| vec![0x99, 0xC9, t, b, s, index, area];
        let cd = |c: Rgb| vec![0x99, 0xCD, c.0, c.1, c.2];
        let off = [Rgb(0, 0, 0)];

        let mut out = match self.effect {
            Effect::Off => vec![c9(0x02, 0x01), cd(off[0])],
            Effect::Gradient => vec![c9(0x02, 0x01), cd(self.colors[0]), c9(0x0A, 0x01)],
            Effect::ColorShift => {
                let mut v = vec![c9(0x08, 0x02)];
                v.extend(self.palette_packets());
                v
            }
            _ => match self.effect.color_use() {
                ColorUse::None => vec![c9(0x02, 0x01)],
                ColorUse::Single => vec![c9(0x02, 0x01), cd(self.colors[0])],
                ColorUse::Palette => {
                    let mut v = vec![c9(0x02, 0x01)];
                    v.extend(self.palette_packets());
                    v
                }
            },
        };
        out.push(vec![0x99, 0xB6]);
        out
    }

    /// `B0`–`B3`, duas cores em cada.
    fn palette_packets(&self) -> Vec<Vec<u8>> {
        self.colors
            .chunks(2)
            .enumerate()
            .map(|(n, pair)| {
                let mut p = vec![0x99, 0xB0 + n as u8, self.effect.wire()];
                for c in pair {
                    p.extend([c.0, c.1, c.2]);
                }
                p
            })
            .collect()
    }
}

/// Destino de output reports. Existe para testar a sequência sem hardware.
pub trait OutputReports {
    fn write_output_report(&self, data: &[u8]) -> io::Result<()>;
}

impl OutputReports for Hidraw {
    fn write_output_report(&self, data: &[u8]) -> io::Result<()> {
        Hidraw::write_output_report(self, data)
    }
}

/// Intervalo entre os pacotes, o `Sleep(1)` da `GcLedLib.dll`.
pub const PACKET_INTERVAL: Duration = Duration::from_millis(1);

#[derive(Debug)]
pub enum ApplyError {
    NotFound,
    Open(OpenError),
    Io(io::Error),
    /// Nenhuma resposta `99 DE` no prazo.
    NoModelResponse,
    /// `r[2]` do `DE` fora de 0, 1 e 2.
    UnknownModel(u8),
}

impl fmt::Display for ApplyError {
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
                "o aparelho não respondeu ao pedido de modelo (DE 00) em 2 s; nada foi enviado ao LED"
            ),
            Self::UnknownModel(v) => write!(
                f,
                "modelo desconhecido no DE ({v}); nada foi enviado ao LED"
            ),
        }
    }
}

impl std::error::Error for ApplyError {}

/// Confere o modelo com `DE 00` e, se for um Waterforce X conhecido, envia a
/// sequência de `setting`, cada pacote completado até 6144 bytes, com
/// [`PACKET_INTERVAL`] entre eles. As respostas aos pacotes de LED não são
/// lidas, como no GCC. Devolve o `r[2]` do `DE`.
pub fn apply_to(
    device: &(impl InputReports + OutputReports),
    setting: &LedSetting,
) -> Result<u8, ApplyError> {
    let query = ReadQuery::Model;
    device
        .write_output_report(&query.packet())
        .map_err(ApplyError::Io)?;
    let response = wait_response(device, query.command(), RESPONSE_TIMEOUT)
        .map_err(ApplyError::Io)?
        .ok_or(ApplyError::NoModelResponse)?;
    let model = match query.decode(&response) {
        Some(Reading::Model(m)) => m,
        _ => return Err(ApplyError::NoModelResponse),
    };
    if model_name(model).is_none() {
        return Err(ApplyError::UnknownModel(model));
    }

    for (i, payload) in setting.packets().iter().enumerate() {
        if i > 0 {
            std::thread::sleep(PACKET_INTERVAL);
        }
        device
            .write_output_report(&packet(&payload[1..]))
            .map_err(ApplyError::Io)?;
    }
    Ok(model)
}

/// Resultado de [`apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub devnode: PathBuf,
    pub model: u8,
}

/// Abre o hidraw do Waterforce sob `sys_root` e chama [`apply_to`].
pub fn apply(sys_root: &Path, setting: &LedSetting) -> Result<Applied, ApplyError> {
    let info = find_hidraw(sys_root)
        .map_err(ApplyError::Io)?
        .ok_or(ApplyError::NotFound)?;
    let device = Hidraw::open(&info.devnode).map_err(ApplyError::Open)?;
    let model = apply_to(&device, setting)?;
    Ok(Applied {
        devnode: info.devnode,
        model,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::waterforce::probe::OUT_SIZE;
    use crate::waterforce::probe::tests::Fake;
    use std::cell::RefCell;

    /// Input reports de um [`Fake`] e registro dos output reports.
    pub(crate) struct FakeDevice {
        input: Fake,
        pub(crate) written: RefCell<Vec<Vec<u8>>>,
    }

    impl FakeDevice {
        pub(crate) fn new(reports: &[&[u8]]) -> Self {
            Self {
                input: Fake::new(reports),
                written: RefCell::new(Vec::new()),
            }
        }

        /// Confere que os pacotes escritos são `expected` (com o report ID,
        /// sem o preenchimento), cada um completado até 6144 bytes.
        pub(crate) fn assert_written(&self, expected: &[&[u8]]) {
            let written = self.written.borrow();
            assert!(written.iter().all(|p| p.len() == OUT_SIZE));
            let expected: Vec<Vec<u8>> = expected.iter().map(|p| packet(&p[1..])).collect();
            assert_eq!(*written, expected);
        }
    }

    impl InputReports for FakeDevice {
        fn read_input_report(&self, buf: &mut [u8], t: Duration) -> io::Result<Option<usize>> {
            self.input.read_input_report(buf, t)
        }
    }

    impl OutputReports for FakeDevice {
        fn write_output_report(&self, data: &[u8]) -> io::Result<()> {
            self.written.borrow_mut().push(data.to_vec());
            Ok(())
        }
    }

    #[test]
    fn modelo_valido_envia_a_sequencia_exata() {
        let dev = FakeDevice::new(&[&[0x99, 0xDA, 0x00], &[0x99, 0xDE, 0x00]]);
        let setting =
            LedSetting::new(Effect::Static, &[Rgb(0xFF, 0x66, 0x00)], None, None).unwrap();
        assert_eq!(apply_to(&dev, &setting).unwrap(), 0);
        dev.assert_written(&[
            &[0x99, 0xDE, 0x00],
            &[0x99, 0xC9, 0x01, 0x64, 0x0A, 0x02, 0x01],
            &[0x99, 0xCD, 0xFF, 0x66, 0x00],
            &[0x99, 0xB6],
        ]);
    }

    #[test]
    fn modelo_desconhecido_nao_envia_led() {
        let dev = FakeDevice::new(&[&[0x99, 0xDE, 0x05]]);
        let setting = LedSetting::new(Effect::Cycle, &[], None, None).unwrap();
        let err = apply_to(&dev, &setting).unwrap_err();
        assert!(matches!(err, ApplyError::UnknownModel(5)));
        assert!(err.to_string().contains("(5)"));
        dev.assert_written(&[&[0x99, 0xDE, 0x00]]);
    }

    #[test]
    fn sem_resposta_ao_de_nao_envia_led() {
        let dev = FakeDevice::new(&[&[0x99, 0xDA, 0x00]]);
        let setting = LedSetting::new(Effect::Cycle, &[], None, None).unwrap();
        assert!(matches!(
            apply_to(&dev, &setting),
            Err(ApplyError::NoModelResponse)
        ));
        dev.assert_written(&[&[0x99, 0xDE, 0x00]]);
    }

    #[test]
    fn envio_nao_encontrado_num_sysfs_vazio() {
        let sys = tempfile::tempdir().unwrap();
        let setting = LedSetting::new(Effect::Cycle, &[], None, None).unwrap();
        assert!(matches!(
            apply(sys.path(), &setting),
            Err(ApplyError::NotFound)
        ));
    }

    #[test]
    fn mensagem_de_permissao_aponta_a_regra() {
        let err = ApplyError::Open(OpenError::PermissionDenied(PathBuf::from("/dev/hidraw2")));
        let msg = err.to_string();
        assert!(msg.contains("/dev/hidraw2"));
        assert!(msg.contains("packaging/udev/70-halo.rules"));
    }

    fn rgb(s: &str) -> Rgb {
        s.parse().unwrap()
    }

    fn seq(effect: Effect, colors: &[&str], b: Option<u8>, s: Option<u8>) -> Vec<Vec<u8>> {
        let colors: Vec<Rgb> = colors.iter().map(|c| rgb(c)).collect();
        LedSetting::new(effect, &colors, b, s).unwrap().packets()
    }

    const B6: &[u8] = &[0x99, 0xB6];

    #[test]
    fn cor_estatica() {
        assert_eq!(
            seq(Effect::Static, &["ff6600"], Some(10), None),
            [
                &[0x99, 0xC9, 0x01, 0x64, 0x0A, 0x02, 0x01][..],
                &[0x99, 0xCD, 0xFF, 0x66, 0x00],
                B6,
            ]
        );
    }

    #[test]
    fn brilho_no_byte_2_velocidade_no_byte_3() {
        let p = seq(Effect::Pulse, &[], Some(3), Some(2));
        assert_eq!(p[0], [0x99, 0xC9, 0x02, 0x1E, 0x04, 0x02, 0x01]);
    }

    #[test]
    fn valores_padrao() {
        let p = seq(Effect::Cycle, &[], None, None);
        assert_eq!(&p[0][3..5], &[0x64, 0x0A]);
    }

    #[test]
    fn efeitos_de_cor_unica() {
        for (effect, t) in [
            (Effect::Static, 0x01),
            (Effect::Pulse, 0x02),
            (Effect::Flash, 0x04),
            (Effect::DoubleFlash, 0x05),
        ] {
            let p = seq(effect, &[], None, None);
            assert_eq!(
                p,
                [
                    &[0x99, 0xC9, t, 0x64, 0x0A, 0x02, 0x01][..],
                    &[0x99, 0xCD, 0xFF, 0x66, 0x00],
                    B6,
                ],
                "{effect}"
            );
        }
    }

    #[test]
    fn efeitos_sem_cor() {
        for (effect, t) in [
            (Effect::Cycle, 0x03),
            (Effect::Wave, 0x08),
            (Effect::Rainbow, 0x0A),
        ] {
            let p = seq(effect, &[], None, None);
            assert_eq!(
                p,
                [&[0x99, 0xC9, t, 0x64, 0x0A, 0x02, 0x01][..], B6],
                "{effect}"
            );
        }
    }

    #[test]
    fn gradiente() {
        assert_eq!(
            seq(Effect::Gradient, &["00ff00"], None, None),
            [
                &[0x99, 0xC9, 0x06, 0x64, 0x0A, 0x02, 0x01][..],
                &[0x99, 0xCD, 0x00, 0xFF, 0x00],
                &[0x99, 0xC9, 0x06, 0x64, 0x0A, 0x0A, 0x01],
                B6,
            ]
        );
    }

    #[test]
    fn transicao_de_cores_com_paleta_padrao() {
        let p = seq(Effect::ColorShift, &[], None, None);
        assert_eq!(
            p,
            [
                &[0x99, 0xC9, 0x07, 0x64, 0x0A, 0x08, 0x02][..],
                &[0x99, 0xB0, 0x07, 0xFF, 0x00, 0x00, 0xFF, 0x72, 0x00],
                &[0x99, 0xB1, 0x07, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0x00],
                &[0x99, 0xB2, 0x07, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF],
                &[0x99, 0xB3, 0x07, 0xFF, 0x00, 0xFF, 0xFF, 0x80, 0x80],
                B6,
            ]
        );
    }

    #[test]
    fn efeitos_de_paleta() {
        for (effect, t) in [
            (Effect::TriColor, 0x0B),
            (Effect::Spin, 0x0C),
            (Effect::Switch, 0x0D),
            (Effect::Lego, 0x0E),
            (Effect::Aurora, 0x0F),
        ] {
            let p = seq(effect, &[], None, None);
            assert_eq!(p.len(), 6, "{effect}");
            assert_eq!(p[0], [0x99, 0xC9, t, 0x64, 0x0A, 0x02, 0x01], "{effect}");
            for (n, packet) in p[1..5].iter().enumerate() {
                assert_eq!(&packet[..3], &[0x99, 0xB0 + n as u8, t], "{effect}");
                assert_eq!(packet.len(), 9, "{effect}");
            }
            assert_eq!(p[5], B6, "{effect}");
        }
    }

    #[test]
    fn tres_cores_completadas_com_a_ultima() {
        let p = seq(
            Effect::TriColor,
            &["ff0000", "00ff00", "0000ff"],
            None,
            None,
        );
        assert_eq!(p[1], [0x99, 0xB0, 0x0B, 0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00]);
        for n in 1..4u8 {
            assert_eq!(
                p[1 + n as usize],
                [0x99, 0xB0 + n, 0x0B, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF]
            );
        }
    }

    #[test]
    fn desligar() {
        assert_eq!(
            seq(Effect::Off, &[], None, None),
            [
                &[0x99, 0xC9, 0x01, 0x00, 0x0A, 0x02, 0x01][..],
                &[0x99, 0xCD, 0x00, 0x00, 0x00],
                B6,
            ]
        );
    }

    #[test]
    fn um_b6_so_no_fim() {
        for effect in Effect::ALL {
            let p = seq(effect, &[], None, None);
            assert_eq!(p.iter().filter(|p| p[1] == 0xB6).count(), 1, "{effect}");
            assert_eq!(p.last().unwrap().as_slice(), B6, "{effect}");
            assert!(p.iter().all(|p| p[0] == 0x99), "{effect}");
        }
    }

    #[test]
    fn nomes_ida_e_volta() {
        for effect in Effect::ALL {
            assert_eq!(Effect::from_name(effect.name()), Some(effect));
        }
        assert_eq!(Effect::from_name("nada"), None);
    }

    #[test]
    fn cores_lidas() {
        assert_eq!(rgb("#FF6600"), Rgb(0xFF, 0x66, 0x00));
        assert_eq!(rgb("ff6600").to_string(), "ff6600");
        for bad in ["ff66", "gg6600", "ff66001", "#ff66é"] {
            assert!(bad.parse::<Rgb>().is_err(), "{bad}");
        }
    }

    #[test]
    fn brilho_e_velocidade_fora_da_faixa() {
        let err = |b, s| LedSetting::new(Effect::Pulse, &[], b, s).unwrap_err();
        assert_eq!(err(Some(0), None), LedError::BrightnessOutOfRange(0));
        assert_eq!(err(Some(11), None), LedError::BrightnessOutOfRange(11));
        assert_eq!(err(None, Some(0)), LedError::SpeedOutOfRange(0));
        assert_eq!(err(None, Some(6)), LedError::SpeedOutOfRange(6));
        assert!(err(None, Some(6)).to_string().contains("de 1 a 5"));
        assert!(LedSetting::new(Effect::Pulse, &[], Some(1), Some(1)).is_ok());
        assert!(LedSetting::new(Effect::Pulse, &[], Some(10), Some(5)).is_ok());
    }

    #[test]
    fn paleta_grande_demais() {
        let nine = vec![Rgb(1, 2, 3); 9];
        assert_eq!(
            LedSetting::new(Effect::Aurora, &nine, None, None),
            Err(LedError::TooManyColors(9))
        );
        assert!(LedSetting::new(Effect::Aurora, &nine[..8], None, None).is_ok());
    }

    #[test]
    fn cor_recusada_nos_efeitos_sem_cor() {
        for effect in [Effect::Cycle, Effect::Wave, Effect::Rainbow, Effect::Off] {
            assert_eq!(
                LedSetting::new(effect, &[Rgb(1, 2, 3)], None, None),
                Err(LedError::ColorNotAccepted(effect))
            );
        }
    }

    #[test]
    fn uma_cor_so_nos_de_cor_unica() {
        let two = [Rgb(1, 2, 3), Rgb(4, 5, 6)];
        for effect in [
            Effect::Static,
            Effect::Pulse,
            Effect::Flash,
            Effect::DoubleFlash,
            Effect::Gradient,
        ] {
            assert_eq!(
                LedSetting::new(effect, &two, None, None),
                Err(LedError::SingleColorOnly(effect, 2))
            );
        }
    }

    #[test]
    fn desligar_recusa_brilho() {
        assert_eq!(
            LedSetting::new(Effect::Off, &[], Some(5), None),
            Err(LedError::BrightnessNotAccepted(Effect::Off))
        );
    }
}
