// SPDX-License-Identifier: GPL-2.0-or-later

//! Telemetria pelo hwmon `waterforce` do kernel. Só lê arquivos do sysfs.
//!
//! Atributos de `drivers/hwmon/gigabyte_waterforce.c`: `fan1_input` (fan),
//! `fan2_input` (bomba), `pwm1`/`pwm2` (duty) e `temp1_input` (líquido, m°C).

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{PRODUCT_ID, VENDOR_ID, find_hidraw};
use crate::hid::descriptor::parse_hid_id;

/// `name` do hwmon criado pelo driver.
pub const HWMON_NAME: &str = "waterforce";

/// Validade do status no driver (`STATUS_VALIDITY`). Ler mais rápido que
/// isso só devolve o mesmo valor.
pub const MIN_INTERVAL_SECS: u64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Telemetry {
    pub fan_rpm: u32,
    pub pump_rpm: u32,
    /// Duty bruto, 0 a 255.
    pub fan_duty: u8,
    pub pump_duty: u8,
    pub coolant_millicelsius: i32,
}

impl Telemetry {
    pub fn duty_percent(raw: u8) -> u8 {
        ((u32::from(raw) * 100 + 127) / 255) as u8
    }

    pub fn coolant_celsius(&self) -> f64 {
        f64::from(self.coolant_millicelsius) / 1000.0
    }
}

#[derive(Debug)]
pub enum TelemetryError {
    /// Nem hwmon nem dispositivo HID do Waterforce.
    NotFound,
    /// O dispositivo existe, mas o driver `gigabyte_waterforce` não criou o hwmon.
    DriverNotLoaded,
    Io(PathBuf, io::Error),
    Parse(PathBuf),
}

impl fmt::Display for TelemetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(
                f,
                "Waterforce não encontrado ({VENDOR_ID:04x}:{PRODUCT_ID:04x})"
            ),
            Self::DriverNotLoaded => write!(
                f,
                "Waterforce conectado, mas o driver gigabyte_waterforce não está carregado (tente `sudo modprobe gigabyte_waterforce`)"
            ),
            Self::Io(p, e) => write!(f, "erro ao ler {}: {e}", p.display()),
            Self::Parse(p) => write!(f, "valor inválido em {}", p.display()),
        }
    }
}

impl std::error::Error for TelemetryError {}

/// Diretório do hwmon `waterforce` ligado ao HID `1044:7A4D`, sob `sys_root`.
pub fn find_hwmon(sys_root: &Path) -> io::Result<Option<PathBuf>> {
    let class = sys_root.join("class/hwmon");
    let entries = match fs::read_dir(&class) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };

    let mut found: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let dir = entry?.path();
        let Ok(name) = fs::read_to_string(dir.join("name")) else {
            continue;
        };
        if name.trim() != HWMON_NAME {
            continue;
        }
        let Ok(uevent) = fs::read_to_string(dir.join("device/uevent")) else {
            continue;
        };
        if parse_hid_id(&uevent)
            .is_some_and(|id| id.vendor == VENDOR_ID && id.product == PRODUCT_ID)
        {
            found.push(dir);
        }
    }
    found.sort();
    Ok(found.into_iter().next())
}

/// Lê a telemetria. Não abre o hidraw.
pub fn read(sys_root: &Path) -> Result<Telemetry, TelemetryError> {
    let hwmon =
        find_hwmon(sys_root).map_err(|e| TelemetryError::Io(sys_root.join("class/hwmon"), e))?;
    let Some(hwmon) = hwmon else {
        let device = find_hidraw(sys_root)
            .map_err(|e| TelemetryError::Io(sys_root.join("class/hidraw"), e))?;
        return Err(match device {
            Some(_) => TelemetryError::DriverNotLoaded,
            None => TelemetryError::NotFound,
        });
    };

    Ok(Telemetry {
        fan_rpm: read_attr(&hwmon, "fan1_input")?,
        pump_rpm: read_attr(&hwmon, "fan2_input")?,
        fan_duty: read_attr(&hwmon, "pwm1")?,
        pump_duty: read_attr(&hwmon, "pwm2")?,
        coolant_millicelsius: read_attr(&hwmon, "temp1_input")?,
    })
}

fn read_attr<T: std::str::FromStr>(hwmon: &Path, attr: &str) -> Result<T, TelemetryError> {
    let path = hwmon.join(attr);
    let text = fs::read_to_string(&path).map_err(|e| TelemetryError::Io(path.clone(), e))?;
    text.trim().parse().map_err(|_| TelemetryError::Parse(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASTOR3_UEVENT: &str =
        "DRIVER=gigabyte_waterforce\nHID_ID=0003:00001044:00007A4D\nHID_NAME=ITE Tech. Castor3\n";

    fn add_hwmon(sys: &Path, n: &str, name: &str, uevent: &str) -> PathBuf {
        let dir = sys.join("class/hwmon").join(n);
        fs::create_dir_all(dir.join("device")).unwrap();
        fs::write(dir.join("name"), format!("{name}\n")).unwrap();
        fs::write(dir.join("device/uevent"), uevent).unwrap();
        dir
    }

    fn fill(dir: &Path) {
        for (attr, value) in [
            ("fan1_input", "2204"),
            ("fan2_input", "3030"),
            ("pwm1", "224"),
            ("pwm2", "255"),
            ("temp1_input", "29000"),
        ] {
            fs::write(dir.join(attr), format!("{value}\n")).unwrap();
        }
    }

    fn add_hidraw(sys: &Path) {
        // Coleção de topo com usage page 0x0000, como o descritor do Castor3.
        let dir = sys.join("class/hidraw/hidraw2/device");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("uevent"), CASTOR3_UEVENT).unwrap();
        fs::write(
            dir.join("report_descriptor"),
            [0x05, 0x00, 0x09, 0x02, 0xA1, 0x01, 0xC0],
        )
        .unwrap();
    }

    #[test]
    fn le_os_valores() {
        let sys = tempfile::tempdir().unwrap();
        add_hwmon(sys.path(), "hwmon0", "k10temp", "");
        let dir = add_hwmon(sys.path(), "hwmon6", HWMON_NAME, CASTOR3_UEVENT);
        fill(&dir);

        let t = read(sys.path()).unwrap();
        assert_eq!(
            t,
            Telemetry {
                fan_rpm: 2204,
                pump_rpm: 3030,
                fan_duty: 224,
                pump_duty: 255,
                coolant_millicelsius: 29000,
            }
        );
        assert_eq!(t.coolant_celsius(), 29.0);
    }

    #[test]
    fn ignora_hwmon_waterforce_de_outro_dispositivo() {
        let sys = tempfile::tempdir().unwrap();
        let dir = add_hwmon(
            sys.path(),
            "hwmon3",
            HWMON_NAME,
            "HID_ID=0003:00001044:00007A4E\n",
        );
        fill(&dir);
        assert!(matches!(read(sys.path()), Err(TelemetryError::NotFound)));
    }

    #[test]
    fn aparelho_sem_driver() {
        let sys = tempfile::tempdir().unwrap();
        add_hidraw(sys.path());
        assert!(matches!(
            read(sys.path()),
            Err(TelemetryError::DriverNotLoaded)
        ));
    }

    #[test]
    fn sem_aparelho() {
        let sys = tempfile::tempdir().unwrap();
        assert!(matches!(read(sys.path()), Err(TelemetryError::NotFound)));
    }

    #[test]
    fn valor_invalido() {
        let sys = tempfile::tempdir().unwrap();
        let dir = add_hwmon(sys.path(), "hwmon6", HWMON_NAME, CASTOR3_UEVENT);
        fill(&dir);
        fs::write(dir.join("pwm1"), "abc\n").unwrap();
        assert!(matches!(read(sys.path()), Err(TelemetryError::Parse(p)) if p.ends_with("pwm1")));
    }

    #[test]
    fn porcentagem_do_duty() {
        assert_eq!(Telemetry::duty_percent(224), 88);
        assert_eq!(Telemetry::duty_percent(255), 100);
        assert_eq!(Telemetry::duty_percent(0), 0);
    }
}
