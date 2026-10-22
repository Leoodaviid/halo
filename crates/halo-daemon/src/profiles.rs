// SPDX-License-Identifier: GPL-2.0-or-later

//! Perfis: um TOML por perfil em `$XDG_CONFIG_HOME/halo/profiles/`, com as
//! seções opcionais `waterforce.led` e `waterforce.speed`, e o nome do ativo
//! em `$XDG_STATE_HOME/halo/active-profile`.
//!
//! A validação usa os tipos do `halo-core`, então as regras são as da CLI. No
//! D-Bus, o perfil vai como `a{sv}` com as chaves `waterforce.led` e
//! `waterforce.speed`, cada uma um `a{sv}` com os campos do TOML.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use halo_core::waterforce::led::{Effect, LedSetting, Rgb};
use halo_core::waterforce::speed::{DesiredState, FanPreset, FanSetting, PumpPreset, PumpSetting};
use serde::{Deserialize, Serialize};
use zbus::zvariant::{OwnedValue, Value};

use crate::error::Error;

pub const NAME_MAX: usize = 32;

/// De 1 a [`NAME_MAX`] letras minúsculas ASCII, dígitos e hífen, começando
/// por letra ou dígito.
pub fn valid_name(name: &str) -> bool {
    let first_ok = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    first_ok
        && name.len() <= NAME_MAX
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Conteúdo do arquivo, como está no disco.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waterforce: Option<WaterforceSection>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaterforceSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub led: Option<LedSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<SpeedSection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedSection {
    pub effect: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeedSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fan: Option<SideValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pump: Option<SideValue>,
}

/// Um preset pelo nome ou o RPM de uma curva plana.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SideValue {
    Preset(String),
    Rpm(u16),
}

/// Perfil validado, pronto para ativar.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub file: ProfileFile,
    pub led: Option<LedSetting>,
    pub speed: Option<DesiredState>,
}

/// Monta o `LedSetting` da seção. Sem brilho ou velocidade, vale o padrão.
pub fn led_setting(
    effect: &str,
    colors: &[String],
    brightness: Option<u8>,
    speed: Option<u8>,
) -> Result<LedSetting, String> {
    let effect =
        Effect::from_name(effect).ok_or_else(|| format!("efeito desconhecido `{effect}`"))?;
    let colors: Vec<Rgb> = colors
        .iter()
        .map(|c| Rgb::from_str(c).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    LedSetting::new(effect, &colors, brightness, speed).map_err(|e| e.to_string())
}

fn speed_state(section: &SpeedSection) -> Result<DesiredState, String> {
    let fan = match &section.fan {
        None => None,
        Some(SideValue::Rpm(rpm)) => Some(FanSetting::Flat(*rpm)),
        Some(SideValue::Preset(name)) => Some(FanSetting::Preset(
            FanPreset::from_name(name)
                .ok_or_else(|| format!("preset da fan desconhecido `{name}`"))?,
        )),
    };
    let pump = match &section.pump {
        None => None,
        Some(SideValue::Rpm(rpm)) => Some(PumpSetting::Flat(*rpm)),
        Some(SideValue::Preset(name)) => Some(PumpSetting::Preset(
            PumpPreset::from_name(name)
                .ok_or_else(|| format!("preset da bomba desconhecido `{name}`"))?,
        )),
    };
    DesiredState::new(fan, pump).map_err(|e| e.to_string())
}

impl Profile {
    pub fn validate(file: ProfileFile) -> Result<Self, String> {
        let section = file.waterforce.clone().unwrap_or_default();
        let led = section
            .led
            .as_ref()
            .map(|l| led_setting(&l.effect, &l.colors, l.brightness, l.speed))
            .transpose()
            .map_err(|e| format!("waterforce.led: {e}"))?;
        let speed = section
            .speed
            .as_ref()
            .map(speed_state)
            .transpose()
            .map_err(|e| format!("waterforce.speed: {e}"))?;
        Ok(Self { file, led, speed })
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let file: ProfileFile = toml::from_str(text).map_err(|e| e.message().to_owned())?;
        Self::validate(file)
    }
}

/// `$XDG_CONFIG_HOME/halo` ou `~/.config/halo`.
pub fn config_dir_from(
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    let xdg = xdg_config_home
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    let base = match xdg {
        Some(p) => p,
        None => PathBuf::from(home.filter(|h| !h.is_empty())?).join(".config"),
    };
    Some(base.join("halo"))
}

pub fn config_dir() -> Option<PathBuf> {
    config_dir_from(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
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

fn storage(path: &Path, err: io::Error) -> Error {
    Error::Storage(format!("não consegui usar {}: {err}", path.display()))
}

fn check_name(name: &str) -> Result<(), Error> {
    if valid_name(name) {
        Ok(())
    } else {
        Err(Error::InvalidArgument(format!(
            "nome de perfil inválido `{name}`: use de 1 a {NAME_MAX} letras minúsculas, dígitos e hífen, começando por letra ou dígito"
        )))
    }
}

pub struct ProfileStore {
    dir: PathBuf,
    active: PathBuf,
}

impl ProfileStore {
    /// `config_dir` e `state_dir` são os diretórios `halo` de configuração e
    /// de estado.
    pub fn new(config_dir: &Path, state_dir: &Path) -> Self {
        Self {
            dir: config_dir.join("profiles"),
            active: state_dir.join("active-profile"),
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.toml"))
    }

    /// Nomes válidos com arquivo `.toml`, em ordem.
    pub fn list(&self) -> Result<Vec<String>, Error> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(storage(&self.dir, e)),
        };
        let mut names = Vec::new();
        for entry in entries {
            let path = entry.map_err(|e| storage(&self.dir, e))?.path();
            if path.extension().is_some_and(|e| e == "toml")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                && valid_name(stem)
            {
                names.push(stem.to_owned());
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn load(&self, name: &str) -> Result<Profile, Error> {
        check_name(name)?;
        let path = self.path(name);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(Error::ProfileNotFound(format!(
                    "o perfil `{name}` não existe"
                )));
            }
            Err(e) => return Err(storage(&path, e)),
        };
        Profile::parse(&text).map_err(|e| {
            Error::InvalidArgument(format!("perfil inválido em {}: {e}", path.display()))
        })
    }

    /// Valida e grava. Um perfil inválido não altera o arquivo existente.
    pub fn save(&self, name: &str, file: ProfileFile) -> Result<Profile, Error> {
        check_name(name)?;
        let profile = Profile::validate(file)
            .map_err(|e| Error::InvalidArgument(format!("perfil inválido: {e}")))?;
        let text = toml::to_string(&profile.file)
            .map_err(|e| Error::InvalidArgument(format!("perfil inválido: {e}")))?;
        fs::create_dir_all(&self.dir).map_err(|e| storage(&self.dir, e))?;
        let path = self.path(name);
        write_atomic(&path, &text).map_err(|e| storage(&path, e))?;
        Ok(profile)
    }

    pub fn delete(&self, name: &str) -> Result<(), Error> {
        check_name(name)?;
        let path = self.path(name);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(Error::ProfileNotFound(format!(
                "o perfil `{name}` não existe"
            ))),
            Err(e) => Err(storage(&path, e)),
        }
    }

    /// O perfil ativo gravado. Um arquivo ausente ou com nome inválido conta
    /// como nenhum.
    pub fn active(&self) -> Result<Option<String>, Error> {
        match fs::read_to_string(&self.active) {
            Ok(text) => Ok(Some(text.trim().to_owned()).filter(|n| valid_name(n))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(storage(&self.active, e)),
        }
    }

    pub fn set_active(&self, name: Option<&str>) -> Result<(), Error> {
        match name {
            Some(name) => {
                let dir = self.active.parent().unwrap();
                fs::create_dir_all(dir).map_err(|e| storage(dir, e))?;
                write_atomic(&self.active, &format!("{name}\n"))
                    .map_err(|e| storage(&self.active, e))
            }
            None => match fs::remove_file(&self.active) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(storage(&self.active, e)),
            },
        }
    }
}

// a{sv} <-> TOML

fn to_toml(value: &Value<'_>) -> Result<toml::Value, String> {
    let int = |n: i64| Ok(toml::Value::Integer(n));
    match value {
        Value::Bool(b) => Ok(toml::Value::Boolean(*b)),
        Value::U8(n) => int((*n).into()),
        Value::I16(n) => int((*n).into()),
        Value::U16(n) => int((*n).into()),
        Value::I32(n) => int((*n).into()),
        Value::U32(n) => int((*n).into()),
        Value::I64(n) => int(*n),
        Value::U64(n) => i64::try_from(*n)
            .map(toml::Value::Integer)
            .map_err(|_| format!("número grande demais: {n}")),
        Value::Str(s) => Ok(toml::Value::String(s.to_string())),
        Value::Value(inner) => to_toml(inner),
        Value::Array(items) => items
            .iter()
            .map(to_toml)
            .collect::<Result<_, _>>()
            .map(toml::Value::Array),
        Value::Dict(dict) => {
            let mut table = toml::Table::new();
            for (k, v) in dict.iter() {
                let Value::Str(k) = k else {
                    return Err("as chaves dos dicionários precisam ser strings".to_owned());
                };
                table.insert(k.to_string(), to_toml(v)?);
            }
            Ok(toml::Value::Table(table))
        }
        other => Err(format!(
            "tipo D-Bus não aceito no perfil: {}",
            other.value_signature()
        )),
    }
}

/// Lê o `a{sv}` do D-Bus. As chaves de seção (`waterforce.led`) viram tabelas
/// aninhadas, como no TOML.
pub fn from_dbus(map: &HashMap<String, OwnedValue>) -> Result<ProfileFile, Error> {
    let bad = |e: String| Error::InvalidArgument(format!("perfil inválido: {e}"));
    let mut root = toml::Table::new();
    for (key, value) in map {
        let mut table = &mut root;
        let mut parts = key.split('.').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                table.insert(part.to_owned(), to_toml(value).map_err(bad)?);
                break;
            }
            let entry = table
                .entry(part.to_owned())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()));
            let toml::Value::Table(inner) = entry else {
                return Err(bad(format!("chave repetida `{key}`")));
            };
            table = inner;
        }
    }
    toml::Value::Table(root)
        .try_into()
        .map_err(|e: toml::de::Error| bad(e.message().to_owned()))
}

fn owned(value: Value<'_>) -> OwnedValue {
    value
        .try_to_owned()
        .expect("os valores do perfil não têm descritores")
}

/// O `a{sv}` do D-Bus: brilho e velocidade como `y`, RPM como `q`.
pub fn to_dbus(file: &ProfileFile) -> HashMap<String, OwnedValue> {
    let mut map = HashMap::new();
    let Some(section) = &file.waterforce else {
        return map;
    };
    if let Some(led) = &section.led {
        let mut d: HashMap<&str, Value<'_>> = HashMap::new();
        d.insert("effect", Value::from(led.effect.as_str()));
        if !led.colors.is_empty() {
            d.insert("colors", Value::from(led.colors.clone()));
        }
        if let Some(b) = led.brightness {
            d.insert("brightness", Value::from(b));
        }
        if let Some(s) = led.speed {
            d.insert("speed", Value::from(s));
        }
        map.insert("waterforce.led".to_owned(), owned(Value::from(d)));
    }
    if let Some(speed) = &section.speed {
        let side = |v: &SideValue| -> Value<'static> {
            match v {
                SideValue::Preset(name) => Value::from(name.clone()),
                SideValue::Rpm(rpm) => Value::from(*rpm),
            }
        };
        let mut d: HashMap<&str, Value<'_>> = HashMap::new();
        if let Some(fan) = &speed.fan {
            d.insert("fan", side(fan));
        }
        if let Some(pump) = &speed.pump {
            d.insert("pump", side(pump));
        }
        map.insert("waterforce.speed".to_owned(), owned(Value::from(d)));
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    const BOTH: &str = r#"
[waterforce.led]
effect = "static"
colors = ["ff6600"]

[waterforce.speed]
fan = "quiet"
pump = "turbo"
"#;

    fn store() -> (tempfile::TempDir, ProfileStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(
            &dir.path().join("config/halo"),
            &dir.path().join("state/halo"),
        );
        (dir, store)
    }

    fn is_root() -> bool {
        fs::metadata("/proc/self").unwrap().uid() == 0
    }

    #[test]
    fn diretorio_de_configuracao() {
        assert_eq!(
            config_dir_from(Some("/x/cfg".into()), Some("/home/u".into())),
            Some(PathBuf::from("/x/cfg/halo"))
        );
        assert_eq!(
            config_dir_from(None, Some("/home/u".into())),
            Some(PathBuf::from("/home/u/.config/halo"))
        );
        assert_eq!(
            config_dir_from(Some("relativo".into()), Some("/home/u".into())),
            Some(PathBuf::from("/home/u/.config/halo"))
        );
        assert_eq!(config_dir_from(None, None), None);
    }

    #[test]
    fn nomes() {
        for ok in ["noite", "a", "jogo-2", "9", &"a".repeat(32)] {
            assert!(valid_name(ok), "{ok}");
        }
        for bad in [
            "",
            "../x",
            "Noite",
            "-a",
            "a b",
            "a_b",
            "á",
            &"a".repeat(33),
        ] {
            assert!(!valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn perfil_com_as_duas_secoes() {
        let p = Profile::parse(BOTH).unwrap();
        assert_eq!(p.led.unwrap().effect(), Effect::Static);
        assert_eq!(
            p.speed,
            Some(
                DesiredState::new(
                    Some(FanSetting::Preset(FanPreset::Quiet)),
                    Some(PumpSetting::Preset(PumpPreset::Turbo))
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn velocidade_com_rpm() {
        let p = Profile::parse("[waterforce.speed]\nfan = 1500\npump = \"balanced\"\n").unwrap();
        assert_eq!(
            p.speed,
            Some(
                DesiredState::new(
                    Some(FanSetting::Flat(1500)),
                    Some(PumpSetting::Preset(PumpPreset::Balanced))
                )
                .unwrap()
            )
        );
        assert!(p.led.is_none());
    }

    #[test]
    fn perfil_vazio_e_valido() {
        let p = Profile::parse("").unwrap();
        assert!(p.led.is_none() && p.speed.is_none());
    }

    #[test]
    fn recusas() {
        for (text, needle) in [
            ("[waterforce.speed]\npump = 2000\n", "2000 RPM"),
            ("[waterforce.speed]\nfan = \"zero-rpm\"\n", "preset da fan"),
            ("[waterforce.speed]\npump = \"quiet\"\n", "preset da bomba"),
            (
                "[waterforce.led]\neffect = \"strobe\"\n",
                "efeito desconhecido",
            ),
            (
                "[waterforce.led]\neffect = \"static\"\nbrightness = 15\n",
                "brilho 15",
            ),
            (
                "[waterforce.led]\neffect = \"static\"\ncolors = [\"xyz\"]\n",
                "cor inválida",
            ),
            (
                "[waterforce.led]\neffect = \"cycle\"\nbrilho = 5\n",
                "brilho",
            ),
            ("[it5702]\n", "it5702"),
        ] {
            let err = Profile::parse(text).unwrap_err();
            assert!(err.contains(needle), "{text}: {err}");
        }
    }

    #[test]
    fn perfil_salvo() {
        let (dir, store) = store();
        let file = Profile::parse(BOTH).unwrap().file;
        store.save("noite", file.clone()).unwrap();
        let path = dir.path().join("config/halo/profiles/noite.toml");
        assert!(path.exists());
        assert_eq!(store.load("noite").unwrap().file, file);
        assert_eq!(store.list().unwrap(), ["noite"]);
    }

    #[test]
    fn nome_com_barra() {
        let (dir, store) = store();
        let err = store.save("../x", ProfileFile::default()).unwrap_err();
        assert!(matches!(err, Error::InvalidArgument(_)));
        assert!(!dir.path().join("config").exists());
    }

    #[test]
    fn rpm_fora_da_faixa_nao_altera_o_arquivo() {
        let (_dir, store) = store();
        let good = Profile::parse(BOTH).unwrap().file;
        store.save("noite", good.clone()).unwrap();
        let mut bad = good.clone();
        bad.waterforce.as_mut().unwrap().speed = Some(SpeedSection {
            fan: None,
            pump: Some(SideValue::Rpm(2000)),
        });
        let err = store.save("noite", bad).unwrap_err();
        assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
        assert_eq!(store.load("noite").unwrap().file, good);
    }

    #[test]
    fn arquivo_editado_a_mao_com_erro() {
        let (_dir, store) = store();
        fs::create_dir_all(store.path("x").parent().unwrap()).unwrap();
        fs::write(
            store.path("noite"),
            "[waterforce.led]\neffect = \"static\"\nbrightness = 15\n",
        )
        .unwrap();
        let err = store.load("noite").unwrap_err();
        let Error::InvalidArgument(m) = err else {
            panic!("{err:?}")
        };
        assert!(m.contains("noite.toml"), "{m}");
    }

    #[test]
    fn falha_ao_gravar_mantem_o_anterior() {
        if is_root() {
            return;
        }
        let (_dir, store) = store();
        let good = Profile::parse(BOTH).unwrap().file;
        store.save("noite", good.clone()).unwrap();
        let dir = store.path("noite").parent().unwrap().to_owned();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).unwrap();
        let err = store.save("noite", ProfileFile::default());
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(matches!(err, Err(Error::Storage(_))), "{err:?}");
        assert_eq!(store.load("noite").unwrap().file, good);
    }

    #[test]
    fn inexistente_e_ativo() {
        let (_dir, store) = store();
        assert!(matches!(store.load("nada"), Err(Error::ProfileNotFound(_))));
        assert!(matches!(
            store.delete("nada"),
            Err(Error::ProfileNotFound(_))
        ));
        assert_eq!(store.active().unwrap(), None);
        store.set_active(Some("noite")).unwrap();
        assert_eq!(store.active().unwrap().as_deref(), Some("noite"));
        store.set_active(None).unwrap();
        assert_eq!(store.active().unwrap(), None);
    }

    #[test]
    fn ida_e_volta_pelo_dbus() {
        let file = Profile::parse(
            "[waterforce.led]\neffect = \"tri-color\"\ncolors = [\"ff0000\", \"00ff00\"]\nbrightness = 3\nspeed = 2\n\n[waterforce.speed]\nfan = 1500\npump = \"balanced\"\n",
        )
        .unwrap()
        .file;
        let map = to_dbus(&file);
        assert_eq!(map.len(), 2);
        assert_eq!(from_dbus(&map).unwrap(), file);
    }

    #[test]
    fn dbus_com_tipos_inteiros_diferentes() {
        let mut speed: HashMap<&str, Value<'_>> = HashMap::new();
        speed.insert("fan", Value::from(1500u32));
        let mut map = HashMap::new();
        map.insert("waterforce.speed".to_owned(), owned(Value::from(speed)));
        let file = from_dbus(&map).unwrap();
        assert_eq!(
            file.waterforce.unwrap().speed.unwrap().fan,
            Some(SideValue::Rpm(1500))
        );
    }

    #[test]
    fn dbus_com_chave_desconhecida() {
        let mut map = HashMap::new();
        map.insert("waterforce.lcd".to_owned(), owned(Value::from(1u8)));
        assert!(matches!(from_dbus(&map), Err(Error::InvalidArgument(_))));
    }
}
