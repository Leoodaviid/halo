// SPDX-License-Identifier: GPL-2.0-or-later

//! Estado salvo da RAM: o último efeito aplicado e se os pentes estão ligados.
//!
//! O pente não guarda o que recebe depois de um corte de energia, e numa
//! suspensão S3 fica aceso. O FURY CTRL 2.0.66.0 grava o perfil a cada
//! aplicação e o reaplica na partida (`OnStart` → `KeepDramLEDEffectFeature`) e
//! ao retomar (`OnPowerEvent` → `applyDRAMLEDmode(bResume: true)`), e apaga os
//! pentes ao suspender (`setDRAMDDR5_AllOff`). O halo faz o mesmo com este
//! arquivo, que as unidades de sistema leem como root. Por isso a leitura é
//! estrita e recusa um arquivo que outro usuário possa ter alterado.
//!
//! O efeito fica separado de ligado/desligado: o `on` só escreve índice e
//! brilho, e depois de um corte de energia o pente está no rainbow de fábrica.
//! Guardar só o último comando faria `static` → `off` → `on` voltar em rainbow.

use std::fmt;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use super::controller::{RamBrightness, RamCommand};
use crate::waterforce::led::Rgb;

/// Diretório padrão do estado.
pub const DEFAULT_DIR: &str = "/var/lib/halo";
/// Nome do arquivo de estado.
pub const FILE_NAME: &str = "ram-led";
const TMP_NAME: &str = "ram-led.tmp";
const VERSION: &str = "1";

/// Efeito salvo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// O halo só apagou ou reacendeu; o efeito é o de fábrica.
    None,
    /// `setDRAMDDR5_DefaultRainbow`.
    Default,
    Static {
        color: Rgb,
        brightness: RamBrightness,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    On,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedState {
    pub effect: Effect,
    pub power: Power,
    /// Instâncias do `ee1004` dos pentes, por exemplo `0-0052`.
    pub spd: Vec<String>,
}

impl SavedState {
    /// Estado depois de aplicar `command` sobre `previous` nos pentes `spd`.
    /// `off` e `on` mantêm o efeito; o `on` sobre um estático grava o brilho
    /// que ele escreve (80).
    pub fn after(previous: Option<&SavedState>, command: RamCommand, spd: Vec<String>) -> Self {
        let kept = previous.map_or(Effect::None, |p| p.effect);
        let (effect, power) = match command {
            RamCommand::Off => (kept, Power::Off),
            RamCommand::On => {
                let effect = match kept {
                    Effect::Static { color, .. } => Effect::Static {
                        color,
                        brightness: RamBrightness::default(),
                    },
                    other => other,
                };
                (effect, Power::On)
            }
            RamCommand::Default => (Effect::Default, Power::On),
            RamCommand::Static { color, brightness } => {
                (Effect::Static { color, brightness }, Power::On)
            }
        };
        Self { effect, power, spd }
    }

    /// Comandos que reaplicam este estado, em ordem: o efeito (nada com
    /// `none`) e, desligado, o `off`; ligado sem efeito, o `on`.
    pub fn restore_commands(&self) -> Vec<RamCommand> {
        let mut out = Vec::new();
        match self.effect {
            Effect::None => {}
            Effect::Default => out.push(RamCommand::Default),
            Effect::Static { color, brightness } => {
                out.push(RamCommand::Static { color, brightness });
            }
        }
        match (self.power, self.effect) {
            (Power::Off, _) => out.push(RamCommand::Off),
            (Power::On, Effect::None) => out.push(RamCommand::On),
            (Power::On, _) => {}
        }
        out
    }

    /// Resumo para as mensagens, por exemplo `static ffffff, brilho 80, ligado`.
    pub fn label(&self) -> String {
        let effect = match self.effect {
            Effect::None => "efeito de fábrica".to_owned(),
            Effect::Default => "default".to_owned(),
            Effect::Static { color, brightness } => {
                format!("static {color}, brilho {}", brightness.value())
            }
        };
        let power = match self.power {
            Power::On => "ligado",
            Power::Off => "desligado",
        };
        format!("{effect}, {power}")
    }

    pub fn render(&self) -> String {
        let mut out = String::from(
            "# halo: estado da RAM, gravado pelo halo led ram. Não edite: o\n\
             # restore recusa o arquivo inteiro se uma linha for inválida.\n",
        );
        out.push_str(&format!("version={VERSION}\n"));
        match self.effect {
            Effect::None => out.push_str("effect=none\n"),
            Effect::Default => out.push_str("effect=default\n"),
            Effect::Static { color, brightness } => {
                out.push_str(&format!(
                    "effect=static\ncolor={color}\nbrightness={}\n",
                    brightness.value()
                ));
            }
        }
        out.push_str(match self.power {
            Power::On => "power=on\n",
            Power::Off => "power=off\n",
        });
        out.push_str(&format!("spd={}\n", self.spd.join(",")));
        out
    }

    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let mut version = None;
        let mut effect = None;
        let mut power = None;
        let mut spd = None;
        let mut color = None;
        let mut brightness = None;
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line_no = n + 1;
            let (key, value) = line
                .split_once('=')
                .ok_or(ParseError::Malformed { line: line_no })?;
            let (key, value) = (key.trim(), value.trim());
            let slot = match key {
                "version" => &mut version,
                "effect" => &mut effect,
                "power" => &mut power,
                "spd" => &mut spd,
                "color" => &mut color,
                "brightness" => &mut brightness,
                _ => return Err(ParseError::UnknownKey(key.to_owned())),
            };
            if slot.replace(value.to_owned()).is_some() {
                return Err(ParseError::Repeated(key.to_owned()));
            }
        }
        let need = |v: Option<String>, k: &'static str| v.ok_or(ParseError::Missing(k));
        let version = need(version, "version")?;
        if version != VERSION {
            return Err(ParseError::Version(version));
        }
        let effect = match need(effect, "effect")?.as_str() {
            "none" | "default" if color.is_some() || brightness.is_some() => {
                return Err(ParseError::ColorWithoutStatic);
            }
            "none" => Effect::None,
            "default" => Effect::Default,
            "static" => Effect::Static {
                color: parse_color(&need(color, "color")?)?,
                brightness: parse_brightness(&need(brightness, "brightness")?)?,
            },
            other => return Err(ParseError::Value("effect", other.to_owned())),
        };
        let power = match need(power, "power")?.as_str() {
            "on" => Power::On,
            "off" => Power::Off,
            other => return Err(ParseError::Value("power", other.to_owned())),
        };
        let spd = parse_spd(&need(spd, "spd")?)?;
        Ok(Self { effect, power, spd })
    }
}

fn parse_color(v: &str) -> Result<Rgb, ParseError> {
    let bad = || ParseError::Value("color", v.to_owned());
    if v.len() != 6 || !v.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(bad());
    }
    v.parse().map_err(|_| bad())
}

fn parse_brightness(v: &str) -> Result<RamBrightness, ParseError> {
    let bad = || ParseError::Value("brightness", v.to_owned());
    if v.is_empty() || v.len() > 3 || !v.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    v.parse::<u8>()
        .ok()
        .and_then(RamBrightness::new)
        .ok_or_else(bad)
}

/// `B-00AA`: o número do barramento, um hífen e 4 dígitos hexadecimais.
fn valid_instance(s: &str) -> bool {
    let Some((bus, addr)) = s.split_once('-') else {
        return false;
    };
    !bus.is_empty()
        && bus.bytes().all(|b| b.is_ascii_digit())
        && addr.len() == 4
        && addr.bytes().all(|b| b.is_ascii_hexdigit())
}

fn parse_spd(v: &str) -> Result<Vec<String>, ParseError> {
    let bad = || ParseError::Value("spd", v.to_owned());
    let list: Vec<String> = v.split(',').map(|s| s.trim().to_owned()).collect();
    if list.iter().any(|s| !valid_instance(s)) {
        return Err(bad());
    }
    let mut sorted = list.clone();
    sorted.sort();
    sorted.dedup();
    if sorted.len() != list.len() {
        return Err(bad());
    }
    Ok(list)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Malformed { line: usize },
    UnknownKey(String),
    Repeated(String),
    Missing(&'static str),
    Version(String),
    Value(&'static str, String),
    ColorWithoutStatic,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { line } => write!(f, "linha {line} não é chave=valor"),
            Self::UnknownKey(k) => write!(f, "chave desconhecida: {k}"),
            Self::Repeated(k) => write!(f, "chave repetida: {k}"),
            Self::Missing(k) => write!(f, "falta a chave {k}"),
            Self::Version(v) => write!(f, "versão {v} não suportada (esta lê a {VERSION})"),
            Self::Value(k, v) => write!(f, "valor inválido em {k}: {v:?}"),
            Self::ColorWithoutStatic => write!(f, "color e brightness só valem com effect=static"),
        }
    }
}

/// Falha ao ler o estado. Toda variante significa "não escrever nada".
#[derive(Debug)]
pub enum LoadError {
    Io(PathBuf, io::Error),
    NotRegular(PathBuf),
    /// Dono diferente do usuário efetivo.
    Owner {
        path: PathBuf,
        uid: u32,
    },
    /// O grupo ou outros podem gravar.
    Writable(PathBuf),
    Parse(PathBuf, ParseError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(p, e) => write!(f, "não consegui ler {}: {e}", p.display()),
            Self::NotRegular(p) => write!(f, "{} não é um arquivo comum; recusado", p.display()),
            Self::Owner { path, uid } => write!(
                f,
                "{} pertence ao uid {uid}, e não a quem roda o halo; recusado",
                path.display()
            ),
            Self::Writable(p) => write!(
                f,
                "{} pode ser alterado por outros usuários (grupo ou outros com escrita); recusado",
                p.display()
            ),
            Self::Parse(p, e) => write!(f, "{} é inválido ({e}); recusado", p.display()),
        }
    }
}

impl std::error::Error for LoadError {}

fn euid() -> u32 {
    // SAFETY: geteuid não tem pré-condições e não falha.
    unsafe { libc::geteuid() }
}

fn check_meta(path: &Path, meta: &fs::Metadata) -> Result<(), LoadError> {
    if meta.uid() != euid() {
        return Err(LoadError::Owner {
            path: path.to_owned(),
            uid: meta.uid(),
        });
    }
    if meta.mode() & 0o022 != 0 {
        return Err(LoadError::Writable(path.to_owned()));
    }
    Ok(())
}

/// Lê o estado de `dir`. Sem o arquivo, `Ok(None)`. Recusa um arquivo que não
/// seja comum, que não seja do usuário efetivo ou que o grupo ou outros possam
/// gravar, e o mesmo para o diretório.
pub fn load(dir: &Path) -> Result<Option<SavedState>, LoadError> {
    let path = dir.join(FILE_NAME);
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) if e.raw_os_error() == Some(libc::ELOOP) => return Err(LoadError::NotRegular(path)),
        Err(e) => return Err(LoadError::Io(path, e)),
    };
    let meta = file
        .metadata()
        .map_err(|e| LoadError::Io(path.clone(), e))?;
    if !meta.is_file() {
        return Err(LoadError::NotRegular(path));
    }
    check_meta(&path, &meta)?;
    let dir_meta = fs::metadata(dir).map_err(|e| LoadError::Io(dir.to_owned(), e))?;
    check_meta(dir, &dir_meta)?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|e| LoadError::Io(path.clone(), e))?;
    SavedState::parse(&text)
        .map(Some)
        .map_err(|e| LoadError::Parse(path, e))
}

/// Grava o estado em `dir`, de forma atômica: `ram-led.tmp` no mesmo
/// diretório, `fsync`, `rename` e `fsync` do diretório. O arquivo fica com
/// `0644`; o diretório, se faltar, é criado com `0755`.
///
/// Uma falha até o `rename` deixa o arquivo antigo inteiro
/// ([`SaveError::NotSaved`]). Uma falha no `fsync` do diretório, depois do
/// `rename`, deixa o arquivo novo no lugar, mas sem garantia de que ele
/// sobreviva a um corte de energia ([`SaveError::NotDurable`]).
pub fn save(dir: &Path, state: &SavedState) -> Result<(), SaveError> {
    if !dir.is_dir() {
        DirBuilder::new()
            .recursive(true)
            .mode(0o755)
            .create(dir)
            .map_err(SaveError::NotSaved)?;
    }
    let tmp = dir.join(TMP_NAME);
    let written = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o644)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&tmp)?;
        f.set_permissions(fs::Permissions::from_mode(0o644))?;
        f.write_all(state.render().as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, dir.join(FILE_NAME))
    })();
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(SaveError::NotSaved(e));
    }
    File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(SaveError::NotDurable)
}

/// Falha do [`save`].
#[derive(Debug)]
pub enum SaveError {
    /// Nada foi trocado: o arquivo antigo continua inteiro.
    NotSaved(io::Error),
    /// O arquivo novo está no lugar, mas o `fsync` do diretório falhou.
    NotDurable(io::Error),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSaved(e) => write!(f, "{e}"),
            Self::NotDurable(e) => write!(f, "fsync do diretório: {e}"),
        }
    }
}

impl std::error::Error for SaveError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn white(b: u8) -> Effect {
        Effect::Static {
            color: Rgb(0xFF, 0xFF, 0xFF),
            brightness: RamBrightness::new(b).unwrap(),
        }
    }

    fn pair() -> Vec<String> {
        vec!["0-0052".to_owned(), "0-0053".to_owned()]
    }

    fn state(effect: Effect, power: Power) -> SavedState {
        SavedState {
            effect,
            power,
            spd: pair(),
        }
    }

    const VALID: &str =
        "version=1\neffect=static\ncolor=ffffff\nbrightness=80\npower=on\nspd=0-0052,0-0053\n";

    #[test]
    fn ram_estado_ida_e_volta() {
        for effect in [Effect::None, Effect::Default, white(80), white(0)] {
            for power in [Power::On, Power::Off] {
                let s = state(effect, power);
                assert_eq!(SavedState::parse(&s.render()), Ok(s.clone()), "{s:?}");
            }
        }
        let s = state(white(80), Power::On);
        assert!(
            s.render().contains(
                "effect=static\ncolor=ffffff\nbrightness=80\npower=on\nspd=0-0052,0-0053\n"
            )
        );
        assert_eq!(SavedState::parse(VALID), Ok(s));
    }

    #[test]
    fn ram_estado_depois_de_cada_comando() {
        let red20 = RamCommand::Static {
            color: Rgb(0xFF, 0, 0),
            brightness: RamBrightness::new(20).unwrap(),
        };
        let s1 = SavedState::after(None, red20, pair());
        let s2 = SavedState::after(Some(&s1), RamCommand::Off, pair());
        assert_eq!(s2.power, Power::Off);
        assert_eq!(s2.effect, s1.effect);
        let s3 = SavedState::after(Some(&s2), RamCommand::On, pair());
        assert_eq!(
            s3.effect,
            Effect::Static {
                color: Rgb(0xFF, 0, 0),
                brightness: RamBrightness::new(80).unwrap()
            }
        );
        assert_eq!(s3.power, Power::On);

        let off = SavedState::after(None, RamCommand::Off, pair());
        assert_eq!((off.effect, off.power), (Effect::None, Power::Off));
        let def = SavedState::after(Some(&s2), RamCommand::Default, pair());
        assert_eq!((def.effect, def.power), (Effect::Default, Power::On));
    }

    #[test]
    fn ram_estado_comandos_do_restore() {
        let white80 = RamCommand::Static {
            color: Rgb(0xFF, 0xFF, 0xFF),
            brightness: RamBrightness::new(80).unwrap(),
        };
        assert_eq!(state(white(80), Power::On).restore_commands(), [white80]);
        assert_eq!(
            state(white(80), Power::Off).restore_commands(),
            [white80, RamCommand::Off]
        );
        assert_eq!(
            state(Effect::Default, Power::On).restore_commands(),
            [RamCommand::Default]
        );
        assert_eq!(
            state(Effect::None, Power::On).restore_commands(),
            [RamCommand::On]
        );
        assert_eq!(
            state(Effect::None, Power::Off).restore_commands(),
            [RamCommand::Off]
        );
    }

    #[test]
    fn ram_estado_invalido() {
        let cases = [
            ("chave desconhecida", format!("{VALID}command=static\n")),
            ("repetida", format!("{VALID}power=off\n")),
            ("faltando", VALID.replace("power=on\n", "")),
            ("versão", VALID.replace("version=1", "version=2")),
            (
                "cor sem static",
                "version=1\neffect=default\ncolor=ffffff\npower=on\nspd=0-0052\n".to_owned(),
            ),
            (
                "brilho 150",
                VALID.replace("brightness=80", "brightness=150"),
            ),
            ("brilho +5", VALID.replace("brightness=80", "brightness=+5")),
            ("cor com #", VALID.replace("color=ffffff", "color=#ffffff")),
            ("cor curta", VALID.replace("color=ffffff", "color=fffff")),
            ("static sem cor", VALID.replace("color=ffffff\n", "")),
            ("spd vazio", VALID.replace("spd=0-0052,0-0053", "spd=")),
            (
                "spd repetido",
                VALID.replace("spd=0-0052,0-0053", "spd=0-0052,0-0052"),
            ),
            (
                "spd fora do formato",
                VALID.replace("spd=0-0052,0-0053", "spd=../x"),
            ),
            ("efeito", VALID.replace("effect=static", "effect=rainbow")),
            ("power", VALID.replace("power=on", "power=maybe")),
            ("sem =", format!("{VALID}lixo\n")),
        ];
        for (name, text) in cases {
            assert!(SavedState::parse(&text).is_err(), "{name}: {text}");
        }
        assert!(SavedState::parse(&format!("# comentário\n\n{VALID}")).is_ok());
    }

    #[test]
    fn ram_estado_gravado_e_lido() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("halo");
        assert!(load(&sub).unwrap().is_none(), "sem arquivo");
        let s = state(white(80), Power::On);
        save(&sub, &s).unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&sub.join(FILE_NAME)), 0o644);
        assert_eq!(mode(&sub), 0o755);
        assert!(!sub.join(TMP_NAME).exists());
        assert_eq!(load(&sub).unwrap(), Some(s));
    }

    #[test]
    fn ram_estado_recusado_por_permissao() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        fs::set_permissions(d, fs::Permissions::from_mode(0o755)).unwrap();
        save(d, &state(white(80), Power::On)).unwrap();
        let file = d.join(FILE_NAME);

        fs::set_permissions(&file, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(matches!(load(d), Err(LoadError::Writable(p)) if p == file));
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();

        fs::set_permissions(d, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(matches!(load(d), Err(LoadError::Writable(p)) if p == d));
        fs::set_permissions(d, fs::Permissions::from_mode(0o755)).unwrap();

        fs::write(&file, "version=2\n").unwrap();
        assert!(matches!(load(d), Err(LoadError::Parse(..))));

        fs::remove_file(&file).unwrap();
        std::os::unix::fs::symlink("/etc/hostname", &file).unwrap();
        assert!(matches!(load(d), Err(LoadError::NotRegular(_))));
    }

    #[test]
    fn ram_estado_falha_ao_gravar_preserva_o_antigo() {
        if euid() == 0 {
            eprintln!("aviso: como root o diretório só de leitura aceita escrita; teste pulado");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let old = state(Effect::Default, Power::On);
        save(d, &old).unwrap();
        fs::set_permissions(d, fs::Permissions::from_mode(0o555)).unwrap();
        let err = save(d, &state(white(80), Power::Off));
        fs::set_permissions(d, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(err, Err(SaveError::NotSaved(_))), "{err:?}");
        assert_eq!(load(d).unwrap(), Some(old));
    }
}
