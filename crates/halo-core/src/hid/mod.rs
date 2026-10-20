// SPDX-License-Identifier: GPL-2.0-or-later

//! Acesso a dispositivos HID pelo hidraw do Linux.
//!
//! A descoberta lê só metadados do sysfs. Nenhum dispositivo é aberto até que
//! uma operação peça um específico.

pub mod descriptor;
pub mod discovery;
pub mod hidraw;

pub use discovery::{DeviceMatch, HidDeviceInfo, SYSFS_HIDRAW, find_devices};
pub use hidraw::{Hidraw, OpenError};
