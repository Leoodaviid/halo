// SPDX-License-Identifier: GPL-2.0-or-later

//! Descoberta de dispositivos hidraw pelo sysfs, sem abrir nenhum deles.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::descriptor::{parse_hid_id, top_level_usage_pages};

/// Diretório do sysfs com uma entrada por hidraw.
pub const SYSFS_HIDRAW: &str = "/sys/class/hidraw";

/// Critério de busca: VID:PID e usage page de alguma coleção de topo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceMatch {
    pub vendor: u16,
    pub product: u16,
    pub usage_page: u16,
}

/// Um hidraw que combina com o critério.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidDeviceInfo {
    /// Nome no sysfs, por exemplo `hidraw5`.
    pub name: String,
    /// Nó de dispositivo, por exemplo `/dev/hidraw5`.
    pub devnode: PathBuf,
    pub vendor: u16,
    pub product: u16,
    pub usage_pages: Vec<u16>,
}

/// Lista os hidraw sob `sysfs_root` que combinam com `wanted`, em ordem de nome.
///
/// Só lê `device/uevent` e `device/report_descriptor` de cada entrada.
/// Entradas ilegíveis são ignoradas. Um `sysfs_root` inexistente devolve
/// lista vazia, como numa máquina sem nenhum hidraw.
pub fn find_devices(sysfs_root: &Path, wanted: DeviceMatch) -> io::Result<Vec<HidDeviceInfo>> {
    let entries = match fs::read_dir(sysfs_root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    let mut found = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let device_dir = entry.path().join("device");

        let Ok(uevent) = fs::read_to_string(device_dir.join("uevent")) else {
            continue;
        };
        let Some(id) = parse_hid_id(&uevent) else {
            continue;
        };
        if id.vendor != wanted.vendor || id.product != wanted.product {
            continue;
        }

        let Ok(descriptor) = fs::read(device_dir.join("report_descriptor")) else {
            continue;
        };
        let usage_pages = top_level_usage_pages(&descriptor);
        if !usage_pages.contains(&wanted.usage_page) {
            continue;
        }

        found.push(HidDeviceInfo {
            devnode: Path::new("/dev").join(&name),
            name,
            vendor: id.vendor,
            product: id.product,
            usage_pages,
        });
    }

    found.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    const IT5702: DeviceMatch = DeviceMatch {
        vendor: 0x048D,
        product: 0x5702,
        usage_page: 0xFF89,
    };

    fn add(root: &Path, name: &str, hid_id: &str, descriptor: &[u8]) {
        let dir = root.join(name).join("device");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("uevent"),
            format!("DRIVER=hid-generic\nHID_ID={hid_id}\n"),
        )
        .unwrap();
        fs::write(dir.join("report_descriptor"), descriptor).unwrap();
    }

    // Coleção de topo com usage page 0xFF89 e com 0xFF00.
    const PAGE_FF89: &[u8] = &[0x06, 0x89, 0xFF, 0x09, 0x10, 0xA1, 0x01, 0xC0];
    const PAGE_FF00: &[u8] = &[0x06, 0x00, 0xFF, 0x09, 0x01, 0xA1, 0x01, 0xC0];

    #[test]
    fn encontra_em_qualquer_hidraw() {
        let root = tempfile::tempdir().unwrap();
        add(root.path(), "hidraw0", "0003:0000046D:0000C08B", PAGE_FF00);
        add(root.path(), "hidraw9", "0003:0000048D:00005702", PAGE_FF89);

        let found = find_devices(root.path(), IT5702).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "hidraw9");
        assert_eq!(found[0].devnode, PathBuf::from("/dev/hidraw9"));
        assert_eq!(found[0].usage_pages, vec![0xFF89]);
    }

    #[test]
    fn ignora_usage_page_diferente() {
        let root = tempfile::tempdir().unwrap();
        add(root.path(), "hidraw5", "0003:0000048D:00005702", PAGE_FF00);
        assert!(find_devices(root.path(), IT5702).unwrap().is_empty());
    }

    #[test]
    fn ignora_entradas_incompletas() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("hidraw1/device")).unwrap();
        let dir = root.path().join("hidraw2/device");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("uevent"), "HID_ID=0003:0000048D:00005702\n").unwrap();
        assert!(find_devices(root.path(), IT5702).unwrap().is_empty());
    }

    #[test]
    fn raiz_inexistente_e_lista_vazia() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("nao-existe");
        assert!(find_devices(&missing, IT5702).unwrap().is_empty());
    }
}
