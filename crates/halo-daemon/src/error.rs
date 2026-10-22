// SPDX-License-Identifier: GPL-2.0-or-later

//! Erros D-Bus do daemon, sob `io.github.leoodaviid.Halo1.Error`, e o mapa
//! dos erros do `halo-core` para eles.

use halo_core::hid::OpenError;
use halo_core::waterforce::curve::CurveError;
use halo_core::waterforce::led::{ApplyError, LedError};
use halo_core::waterforce::speed::SpeedError;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "io.github.leoodaviid.Halo1.Error")]
pub enum Error {
    #[zbus(error)]
    ZBus(zbus::Error),
    InvalidArgument(String),
    NotFound(String),
    /// A mensagem do core já aponta a regra udev.
    PermissionDenied(String),
    Busy(String),
    /// Falha de comunicação ou modelo desconhecido.
    Device(String),
    ProfileNotFound(String),
    /// Falha ao ler ou gravar arquivos (snapshots, perfis).
    Storage(String),
}

impl Error {
    /// Mesmo tipo de erro, com `context` depois da mensagem.
    pub fn with_context(self, context: &str) -> Self {
        let add = |m: String| format!("{m}; {context}");
        match self {
            Self::ZBus(e) => Self::Device(add(e.to_string())),
            Self::InvalidArgument(m) => Self::InvalidArgument(add(m)),
            Self::NotFound(m) => Self::NotFound(add(m)),
            Self::PermissionDenied(m) => Self::PermissionDenied(add(m)),
            Self::Busy(m) => Self::Busy(add(m)),
            Self::Device(m) => Self::Device(add(m)),
            Self::ProfileNotFound(m) => Self::ProfileNotFound(add(m)),
            Self::Storage(m) => Self::Storage(add(m)),
        }
    }
}

fn from_open(err: &OpenError, message: String) -> Error {
    match err {
        OpenError::NotFound(_) => Error::NotFound(message),
        OpenError::PermissionDenied(_) => Error::PermissionDenied(message),
        OpenError::Busy(_) => Error::Busy(message),
        OpenError::Other(..) => Error::Device(message),
    }
}

impl From<ApplyError> for Error {
    fn from(err: ApplyError) -> Self {
        let message = err.to_string();
        match &err {
            ApplyError::NotFound => Self::NotFound(message),
            ApplyError::Open(e) => from_open(e, message),
            ApplyError::Io(_) | ApplyError::NoModelResponse | ApplyError::UnknownModel(_) => {
                Self::Device(message)
            }
        }
    }
}

impl From<SpeedError> for Error {
    fn from(err: SpeedError) -> Self {
        let message = err.to_string();
        match &err {
            SpeedError::NotFound => Self::NotFound(message),
            SpeedError::Open(e) => from_open(e, message),
            SpeedError::Io(_) | SpeedError::NoResponse(_) | SpeedError::UnknownModel(_) => {
                Self::Device(message)
            }
            SpeedError::NoSide
            | SpeedError::CurrentModeUnknown { .. }
            | SpeedError::OtherSideUnknown { .. } => Self::InvalidArgument(message),
            SpeedError::NoStateDir
            | SpeedError::State(..)
            | SpeedError::NoSnapshot(_)
            | SpeedError::BadSnapshot(..) => Self::Storage(message),
        }
    }
}

impl From<LedError> for Error {
    fn from(err: LedError) -> Self {
        Self::InvalidArgument(err.to_string())
    }
}

impl From<CurveError> for Error {
    fn from(err: CurveError) -> Self {
        Self::InvalidArgument(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use zbus::DBusError;

    fn path() -> PathBuf {
        PathBuf::from("/dev/hidraw2")
    }

    #[test]
    fn nomes_dos_erros() {
        let cases = [
            (Error::InvalidArgument(String::new()), "InvalidArgument"),
            (Error::NotFound(String::new()), "NotFound"),
            (Error::PermissionDenied(String::new()), "PermissionDenied"),
            (Error::Busy(String::new()), "Busy"),
            (Error::Device(String::new()), "Device"),
            (Error::ProfileNotFound(String::new()), "ProfileNotFound"),
            (Error::Storage(String::new()), "Storage"),
        ];
        for (err, name) in cases {
            assert_eq!(
                err.name().as_str(),
                format!("io.github.leoodaviid.Halo1.Error.{name}")
            );
        }
    }

    #[test]
    fn mapa_do_led() {
        assert!(matches!(
            Error::from(ApplyError::NotFound),
            Error::NotFound(_)
        ));
        let denied = Error::from(ApplyError::Open(OpenError::PermissionDenied(path())));
        let Error::PermissionDenied(m) = denied else {
            panic!("{denied:?}")
        };
        assert!(m.contains("/dev/hidraw2") && m.contains("udev"), "{m}");
        assert!(matches!(
            Error::from(ApplyError::Open(OpenError::Busy(path()))),
            Error::Busy(_)
        ));
        assert!(matches!(
            Error::from(ApplyError::UnknownModel(5)),
            Error::Device(_)
        ));
    }

    #[test]
    fn mapa_da_velocidade() {
        assert!(matches!(
            Error::from(SpeedError::NoSide),
            Error::InvalidArgument(_)
        ));
        assert!(matches!(
            Error::from(SpeedError::NoSnapshot(PathBuf::from("/x"))),
            Error::Storage(_)
        ));
        assert!(matches!(
            Error::from(SpeedError::Open(OpenError::NotFound(path()))),
            Error::NotFound(_)
        ));
        assert!(matches!(
            Error::from(SpeedError::UnknownModel(5)),
            Error::Device(_)
        ));
    }

    #[test]
    fn contexto_mantem_o_tipo() {
        let err = Error::Device("modelo desconhecido".into()).with_context("falharam: x");
        let Error::Device(m) = err else {
            panic!("{err:?}")
        };
        assert_eq!(m, "modelo desconhecido; falharam: x");
    }
}
