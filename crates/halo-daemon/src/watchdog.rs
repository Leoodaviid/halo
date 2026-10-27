// SPDX-License-Identifier: GPL-2.0-or-later

//! Watchdog do systemd (`WatchdogSec` na unidade). O `WATCHDOG=1` sai pelo
//! protocolo do `sd_notify(3)` só depois de um ciclo com progresso real dos
//! ticks e do feed ([`crate::feed::Feed::progressed`]); nenhum timer
//! independente renova a saúde.

use std::io;
use std::os::unix::net::UnixDatagram;
use std::time::Duration;

/// Menor `WATCHDOG_USEC` aceito: os pings saem a cada ~2–4 s e precisam
/// ficar abaixo da metade do prazo.
pub const MIN_WATCHDOG: Duration = Duration::from_secs(10);

/// Destino das mensagens do `sd_notify`. Os testes usam um falso.
pub trait Notifier: Send + Sync {
    fn notify(&self, state: &str) -> io::Result<()>;
}

/// Datagrama para o `NOTIFY_SOCKET`, que pode ser um caminho ou um nome
/// abstrato (com `@`).
pub struct SdNotify {
    socket: String,
}

impl Notifier for SdNotify {
    fn notify(&self, state: &str) -> io::Result<()> {
        let sock = UnixDatagram::unbound()?;
        match self.socket.strip_prefix('@') {
            Some(name) => {
                use std::os::linux::net::SocketAddrExt;
                let addr = std::os::unix::net::SocketAddr::from_abstract_name(name)?;
                sock.send_to_addr(state.as_bytes(), &addr)?;
            }
            None => {
                sock.send_to(state.as_bytes(), &self.socket)?;
            }
        }
        Ok(())
    }
}

/// Supervisão efetiva do processo.
pub struct Supervision {
    pub timeout: Duration,
    pub notifier: SdNotify,
}

/// Lê `NOTIFY_SOCKET`, `WATCHDOG_USEC` e `WATCHDOG_PID` com `get`.
/// `Ok(None)` sem watchdog (execução manual); `Err` com um ambiente que não
/// permite supervisão confiável.
pub fn from_env(
    get: impl Fn(&str) -> Option<String>,
    pid: u32,
) -> Result<Option<Supervision>, String> {
    let Some(usec) = get("WATCHDOG_USEC") else {
        return Ok(None);
    };
    let usec: u64 = usec
        .parse()
        .map_err(|_| format!("WATCHDOG_USEC inválido ({usec})"))?;
    let timeout = Duration::from_micros(usec);
    if timeout < MIN_WATCHDOG {
        return Err(format!(
            "WatchdogSec de {timeout:?} abaixo de {MIN_WATCHDOG:?}"
        ));
    }
    if let Some(owner) = get("WATCHDOG_PID")
        && owner.parse::<u32>().ok() != Some(pid)
    {
        return Err(format!("WATCHDOG_PID {owner} não é este processo ({pid})"));
    }
    let socket = get("NOTIFY_SOCKET")
        .filter(|s| !s.is_empty())
        .ok_or("WATCHDOG_USEC sem NOTIFY_SOCKET")?;
    Ok(Some(Supervision {
        timeout,
        notifier: SdNotify { socket },
    }))
}

#[cfg(test)]
pub(crate) mod fake {
    use super::*;
    use std::sync::Mutex;

    /// Guarda as mensagens enviadas.
    #[derive(Default)]
    pub struct FakeNotifier(pub Mutex<Vec<String>>);

    impl FakeNotifier {
        pub fn pings(&self) -> usize {
            self.0
                .lock()
                .unwrap()
                .iter()
                .filter(|m| *m == "WATCHDOG=1")
                .count()
        }
    }

    impl Notifier for FakeNotifier {
        fn notify(&self, state: &str) -> io::Result<()> {
            self.0.lock().unwrap().push(state.to_owned());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn cpu_feed_watchdog_ambiente_valido() {
        let s = from_env(
            env(&[
                ("WATCHDOG_USEC", "20000000"),
                ("WATCHDOG_PID", "42"),
                ("NOTIFY_SOCKET", "/run/user/1000/systemd/notify"),
            ]),
            42,
        )
        .unwrap()
        .unwrap();
        assert_eq!(s.timeout, Duration::from_secs(20));
        // Sem WATCHDOG_PID também vale.
        let s = from_env(
            env(&[("WATCHDOG_USEC", "20000000"), ("NOTIFY_SOCKET", "@x")]),
            42,
        );
        assert!(s.unwrap().is_some());
    }

    #[test]
    fn cpu_feed_watchdog_sem_watchdog() {
        assert!(from_env(env(&[]), 1).unwrap().is_none());
        assert!(
            from_env(env(&[("NOTIFY_SOCKET", "/x")]), 1)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn cpu_feed_watchdog_ambiente_incompativel() {
        let cases: [&[(&str, &str)]; 4] = [
            &[("WATCHDOG_USEC", "abc"), ("NOTIFY_SOCKET", "/x")],
            &[("WATCHDOG_USEC", "5000000"), ("NOTIFY_SOCKET", "/x")],
            &[
                ("WATCHDOG_USEC", "20000000"),
                ("WATCHDOG_PID", "7"),
                ("NOTIFY_SOCKET", "/x"),
            ],
            &[("WATCHDOG_USEC", "20000000")],
        ];
        for pairs in cases {
            assert!(from_env(env(pairs), 42).is_err(), "{pairs:?}");
        }
    }

    #[test]
    fn cpu_feed_watchdog_datagrama_no_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notify");
        let server = UnixDatagram::bind(&path).unwrap();
        let n = SdNotify {
            socket: path.to_string_lossy().into_owned(),
        };
        n.notify("WATCHDOG=1").unwrap();
        let mut buf = [0u8; 32];
        let len = server.recv(&mut buf).unwrap();
        assert_eq!(&buf[..len], b"WATCHDOG=1");
    }
}
