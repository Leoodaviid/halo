// SPDX-License-Identifier: GPL-2.0-or-later

//! Acesso a `/dev/hidrawN`: feature reports com os ioctls `HIDIOCSFEATURE` e
//! `HIDIOCGFEATURE` de `linux/hidraw.h`, output reports com `write(2)` e
//! input reports com `read(2)`.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

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

    /// Envia um output report. `data[0]` é o report ID.
    /// Um `write` que não aceita o pacote inteiro vira erro.
    pub fn write_output_report(&self, data: &[u8]) -> io::Result<()> {
        let n = (&self.file).write(data)?;
        if n != data.len() {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                format!("output report curto: {n} de {} bytes", data.len()),
            ));
        }
        Ok(())
    }

    /// Espera até `timeout` por um input report e o lê em `buf`.
    /// Devolve `None` se o prazo vencer sem dados.
    pub fn read_input_report(
        &self,
        buf: &mut [u8],
        timeout: Duration,
    ) -> io::Result<Option<usize>> {
        let mut pfd = libc::pollfd {
            fd: self.file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ms = libc::c_int::try_from(timeout.as_millis()).unwrap_or(libc::c_int::MAX);
        // SAFETY: `pfd` é um único pollfd válido durante a chamada.
        let res = unsafe { libc::poll(&mut pfd, 1, ms) };
        match res {
            r if r < 0 => Err(io::Error::last_os_error()),
            0 => Ok(None),
            _ => (&self.file).read(buf).map(Some),
        }
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
    use std::os::fd::FromRawFd;

    /// Pipe com a ponta de leitura num `Hidraw`, para testar input reports.
    fn pipe() -> (Hidraw, File) {
        let mut fds = [0; 2];
        // SAFETY: `fds` tem espaço para os dois descritores.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        // SAFETY: os descritores acabaram de ser criados e têm dono único.
        let (read, write) = unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) };
        (Hidraw { file: read }, write)
    }

    #[test]
    fn input_report_com_prazo_vencido() {
        let (dev, _write) = pipe();
        let mut buf = [0u8; 8];
        let res = dev
            .read_input_report(&mut buf, Duration::from_millis(20))
            .unwrap();
        assert_eq!(res, None);
    }

    #[test]
    fn input_report_disponivel() {
        let (dev, mut write) = pipe();
        write.write_all(&[0x99, 0xD6, 0x02, 0x05]).unwrap();
        let mut buf = [0u8; 8];
        let n = dev
            .read_input_report(&mut buf, Duration::from_secs(1))
            .unwrap();
        assert_eq!(n, Some(4));
        assert_eq!(&buf[..4], &[0x99, 0xD6, 0x02, 0x05]);
    }

    #[test]
    fn output_report_inteiro() {
        let mut fds = [0; 2];
        // SAFETY: como em `pipe()`, mas com o `Hidraw` na ponta de escrita.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        let (mut read, write) = unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) };
        let dev = Hidraw { file: write };
        dev.write_output_report(&[0x99, 0xD6, 0x00]).unwrap();
        let mut buf = [0u8; 3];
        read.read_exact(&mut buf).unwrap();
        assert_eq!(buf, [0x99, 0xD6, 0x00]);
    }

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
