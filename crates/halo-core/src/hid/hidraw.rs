// SPDX-License-Identifier: GPL-2.0-or-later

//! Feature reports por `/dev/hidrawN`, com os ioctls `HIDIOCSFEATURE` e
//! `HIDIOCGFEATURE` de `linux/hidraw.h`.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

/// Falha ao abrir um hidraw.
#[derive(Debug)]
pub enum OpenError {
    /// O nó de dispositivo não existe.
    NotFound(PathBuf),
    /// O usuário não tem permissão de leitura e escrita no nó.
    PermissionDenied(PathBuf),
    Other(PathBuf, io::Error),
}

impl OpenError {
    /// Classifica o erro de `open(2)` para `path`.
    pub fn from_io(path: &Path, err: io::Error) -> Self {
        match err.raw_os_error() {
            Some(libc::EACCES) | Some(libc::EPERM) => Self::PermissionDenied(path.to_owned()),
            Some(libc::ENOENT) | Some(libc::ENODEV) | Some(libc::ENXIO) => {
                Self::NotFound(path.to_owned())
            }
            _ => Self::Other(path.to_owned(), err),
        }
    }
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(p) => write!(f, "{} não existe", p.display()),
            Self::PermissionDenied(p) => write!(f, "sem permissão para abrir {}", p.display()),
            Self::Other(p, e) => write!(f, "erro ao abrir {}: {e}", p.display()),
        }
    }
}

impl std::error::Error for OpenError {}

/// Um hidraw aberto para leitura e escrita.
pub struct Hidraw {
    file: File,
}

// _IOC(_IOC_WRITE | _IOC_READ, 'H', nr, len) do Linux em x86 e arm.
const fn hid_ioc_rw(nr: u64, len: usize) -> u64 {
    const IOC_READ_WRITE: u64 = 3;
    (IOC_READ_WRITE << 30) | ((len as u64) << 16) | ((b'H' as u64) << 8) | nr
}

const fn hidiocsfeature(len: usize) -> u64 {
    hid_ioc_rw(0x06, len)
}

const fn hidiocgfeature(len: usize) -> u64 {
    hid_ioc_rw(0x07, len)
}

impl Hidraw {
    pub fn open(path: &Path) -> Result<Self, OpenError> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map(|file| Self { file })
            .map_err(|e| OpenError::from_io(path, e))
    }

    /// Envia um feature report. `data[0]` é o report ID.
    /// Devolve quantos bytes o kernel aceitou.
    pub fn send_feature_report(&self, data: &[u8]) -> io::Result<usize> {
        // O ioctl só lê do buffer, mas a assinatura pede ponteiro mutável.
        let mut buf = data.to_vec();
        self.ioctl(hidiocsfeature(buf.len()), &mut buf)
    }

    /// Lê um feature report. `buf[0]` precisa conter o report ID pedido.
    /// Devolve quantos bytes foram lidos, incluindo o report ID.
    pub fn get_feature_report(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.ioctl(hidiocgfeature(buf.len()), buf)
    }

    fn ioctl(&self, request: u64, buf: &mut [u8]) -> io::Result<usize> {
        // SAFETY: `buf` é válido para `buf.len()` bytes, que é o tamanho
        // codificado no número do ioctl.
        let res = unsafe { libc::ioctl(self.file.as_raw_fd(), request as _, buf.as_mut_ptr()) };
        if res < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(res as usize)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeros_dos_ioctls() {
        // Valores de HIDIOCSFEATURE(64) e HIDIOCGFEATURE(64) calculados com
        // os macros de linux/hidraw.h.
        assert_eq!(hidiocsfeature(64), 0xC040_4806);
        assert_eq!(hidiocgfeature(64), 0xC040_4807);
    }

    #[test]
    fn eacces_e_eperm_sao_falta_de_permissao() {
        let path = Path::new("/dev/hidraw5");
        for errno in [libc::EACCES, libc::EPERM] {
            let err = OpenError::from_io(path, io::Error::from_raw_os_error(errno));
            assert!(matches!(err, OpenError::PermissionDenied(ref p) if p == path));
        }
    }

    #[test]
    fn enoent_e_nao_encontrado() {
        let path = Path::new("/dev/hidraw5");
        let err = OpenError::from_io(path, io::Error::from_raw_os_error(libc::ENOENT));
        assert!(matches!(err, OpenError::NotFound(_)));
    }

    #[test]
    fn mensagem_de_permissao_mostra_o_caminho() {
        let err = OpenError::PermissionDenied(PathBuf::from("/dev/hidraw2"));
        assert_eq!(err.to_string(), "sem permissão para abrir /dev/hidraw2");
    }

    #[test]
    fn abrir_caminho_inexistente() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hidraw99");
        assert!(matches!(Hidraw::open(&path), Err(OpenError::NotFound(_))));
    }
}
