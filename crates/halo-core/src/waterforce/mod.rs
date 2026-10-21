// SPDX-License-Identifier: GPL-2.0-or-later

//! AORUS Waterforce com controlador Castor3 (`1044:7a4d`).
//!
//! A telemetria vem do hwmon do driver `gigabyte_waterforce` do Linux, sem
//! nenhum comando do halo. O probe usa o hidraw só com pedidos de leitura, e o
//! `led` escreve só os comandos de LED da `GcLedLib.dll` do GCC.

pub mod led;
pub mod probe;
pub mod telemetry;

use std::path::Path;

use crate::hid::{DeviceMatch, HidDeviceInfo, find_devices};

pub const VENDOR_ID: u16 = 0x1044;
pub const PRODUCT_ID: u16 = 0x7A4D;
/// O descritor do Castor3 declara a usage page `0x0000`.
pub const USAGE_PAGE: u16 = 0x0000;

pub const DEVICE: DeviceMatch = DeviceMatch {
    vendor: VENDOR_ID,
    product: PRODUCT_ID,
    usage_page: USAGE_PAGE,
};

/// Onde está a regra udev que libera o hidraw ao usuário.
pub const UDEV_RULE: &str = "packaging/udev/70-halo.rules";

/// Primeiro hidraw do Castor3 sob `/sys/class/hidraw` de `sys_root`.
pub fn find_hidraw(sys_root: &Path) -> std::io::Result<Option<HidDeviceInfo>> {
    Ok(find_devices(&sys_root.join("class/hidraw"), DEVICE)?
        .into_iter()
        .next())
}
