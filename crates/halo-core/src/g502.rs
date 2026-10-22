// SPDX-License-Identifier: GPL-2.0-or-later

//! Mouse Logitech G502 SE HERO (`046d:c08b`), só a busca no sysfs.
//!
//! VID:PID e usage page vêm do driver Logitech do OpenRGB
//! (`Controllers/LogitechController/LogitechControllerDetect.cpp`, commit
//! `72262b8`): `LOGITECH_G502_HERO_PID`, registrado com
//! `REGISTER_HID_DETECTOR_IPU` na interface 1, usage page `0xFF00`. Este
//! módulo não abre o hidraw nem envia nada ao mouse.

use crate::hid::DeviceMatch;

pub const VENDOR_ID: u16 = 0x046D;
/// `LOGITECH_G502_HERO_PID`.
pub const PRODUCT_ID: u16 = 0xC08B;
/// Usage page de fabricante da interface 1. A interface 0 é o mouse comum
/// (`0x0001`).
pub const USAGE_PAGE: u16 = 0xFF00;

pub const DEVICE: DeviceMatch = DeviceMatch {
    vendor: VENDOR_ID,
    product: PRODUCT_ID,
    usage_page: USAGE_PAGE,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::find_devices;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn add(root: &Path, name: &str, page: u16) {
        let dir = root.join(name).join("device");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("uevent"), "HID_ID=0003:0000046D:0000C08B\n").unwrap();
        let [hi, lo] = page.to_be_bytes();
        fs::write(
            dir.join("report_descriptor"),
            [0x06, lo, hi, 0x09, 0x01, 0xA1, 0x01, 0xC0],
        )
        .unwrap();
    }

    #[test]
    fn so_a_interface_de_fabricante() {
        let root = tempfile::tempdir().unwrap();
        add(root.path(), "hidraw0", 0x0001);
        add(root.path(), "hidraw1", 0xFF00);
        let found = find_devices(root.path(), DEVICE).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].devnode, PathBuf::from("/dev/hidraw1"));
    }
}
