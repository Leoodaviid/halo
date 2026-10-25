// SPDX-License-Identifier: GPL-2.0-or-later

//! Último LED aplicado pelo halod; carregar não acessa o aparelho.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use halo_core::waterforce::led::{Effect, LedSetting};

use crate::profiles::{LedSection, led_setting, write_atomic};

pub struct LedState {
    path: PathBuf,
}

impl LedState {
    pub fn new(state_dir: &Path) -> Self {
        Self {
            path: state_dir.join("waterforce-led.last"),
        }
    }

    pub fn load(&self) -> Option<LedSetting> {
        let result = (|| -> Result<LedSetting, String> {
            let text = match fs::read_to_string(&self.path) {
                Ok(text) => text,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(String::new()),
                Err(e) => return Err(e.to_string()),
            };
            let section: LedSection = toml::from_str(&text).map_err(|e| e.to_string())?;
            led_setting(
                &section.effect,
                &section.colors,
                section.brightness,
                section.speed,
            )
        })();
        match result {
            Ok(setting) => Some(setting),
            Err(e) => {
                if !e.is_empty() {
                    eprintln!("halod: não consegui ler {}: {e}", self.path.display());
                }
                None
            }
        }
    }

    pub fn save(&self, setting: &LedSetting) -> io::Result<()> {
        let section = LedSection {
            effect: setting.effect().name().to_owned(),
            colors: setting.colors().iter().map(ToString::to_string).collect(),
            brightness: (setting.effect() != Effect::Off).then_some(setting.brightness().get()),
            speed: Some(setting.speed().get()),
        };
        let text = toml::to_string(&section).map_err(io::Error::other)?;
        fs::create_dir_all(self.path.parent().unwrap())?;
        write_atomic(&self.path, &text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::led_setting;
    use std::fs;

    #[test]
    fn ida_e_volta() {
        let dir = tempfile::tempdir().unwrap();
        let store = LedState::new(&dir.path().join("state/halo"));
        assert_eq!(store.load(), None);
        for (effect, colors, brightness) in [
            ("static", vec!["ff0000".to_owned()], Some(10)),
            ("off", vec![], None),
            ("cycle", vec![], Some(3)),
            (
                "tri-color",
                vec![
                    "ff0000".to_owned(),
                    "00ff00".to_owned(),
                    "0000ff".to_owned(),
                    "ffffff".to_owned(),
                    "010203".to_owned(),
                    "ff6600".to_owned(),
                    "123456".to_owned(),
                    "abcdef".to_owned(),
                ],
                Some(6),
            ),
        ] {
            let setting = led_setting(effect, &colors, brightness, Some(2)).unwrap();
            store.save(&setting).unwrap();
            assert_eq!(store.load(), Some(setting));
            let text = fs::read_to_string(&store.path).unwrap();
            assert_eq!(text.contains("brightness"), effect != "off");
            assert_eq!(text.contains("colors"), !colors.is_empty());
            assert!(
                !store
                    .path
                    .with_file_name(".waterforce-led.last.tmp")
                    .exists()
            );
        }
    }

    #[test]
    fn invalido_ou_ilegivel_nao_impede_inicio() {
        let dir = tempfile::tempdir().unwrap();
        let store = LedState::new(dir.path());
        for text in [
            "effect = \"cycle\"\nextra = 1",
            "effect = \"unknown\"",
            "effect = \"static\"\ncolors = [\"zzzzzz\"]",
            "effect = \"cycle\"\nbrightness = 11",
            "effect = \"cycle\"\nspeed = 6",
            "effect = [",
        ] {
            fs::write(&store.path, text).unwrap();
            assert_eq!(store.load(), None, "{text}");
        }
        fs::remove_file(&store.path).unwrap();
        fs::create_dir(&store.path).unwrap();
        assert_eq!(store.load(), None);
    }
}
