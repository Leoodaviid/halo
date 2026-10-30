// SPDX-License-Identifier: GPL-2.0-or-later

//! A conexão com o D-Bus da sessão, que pode cair com o daemon vivo: no
//! logout, o GNOME para o `dbus.service` do usuário
//! (`gnome-session-restart-dbus.service`), e o linger mantém só o
//! `systemd --user`. O daemon reconecta no mesmo processo, com os mesmos
//! objetos e o mesmo nome; sem o nome no prazo, o chamador sai pela proteção.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::future::{Either, select};
use zbus::fdo::{RequestNameFlags, RequestNameReply};
use zbus::{Connection, connection};

use crate::bus::{BUS_NAME, Carried};
use crate::worker::Worker;

/// Quanto o daemon espera o nome de volta depois da queda.
pub const DEADLINE: Duration = Duration::from_secs(10);
/// Intervalo entre as tentativas.
pub const EVERY: Duration = Duration::from_secs(1);

/// Monta uma conexão com os objetos publicados e sem o nome.
pub type Connect = Arc<dyn Fn() -> zbus::Result<connection::Builder<'static>> + Send + Sync>;

/// A conexão atual. Os ticks a pegam a cada volta.
pub struct BusLink(Mutex<Connection>);

impl BusLink {
    pub fn new(conn: Connection) -> Self {
        Self(Mutex::new(conn))
    }

    pub fn get(&self) -> Connection {
        self.0.lock().unwrap().clone()
    }

    fn set(&self, conn: Connection) {
        *self.0.lock().unwrap() = conn;
    }
}

/// Espera a conexão atual cair e reconecta, uma tentativa a cada `every`.
/// `Err` com o motivo, sem o nome em `deadline`.
pub async fn reconnect(
    link: &BusLink,
    connect: &Connect,
    worker: &Worker,
    (deadline, every): (Duration, Duration),
) -> Result<(), String> {
    let old = link.get();
    old.closed().await;
    eprintln!("halod: o D-Bus da sessão caiu; reconectando");
    let carried = Carried::take(old.object_server())
        .await
        .map_err(|e| format!("estado dos objetos ilegível depois da queda: {e}"))?;
    drop(old);
    let until = Instant::now() + deadline;
    let mut last = String::from("nenhuma tentativa");
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(format!(
                "sem o nome {BUS_NAME} no D-Bus da sessão em {}s ({last})",
                deadline.as_secs()
            ));
        }
        let try_once = attempt(connect, &carried);
        futures_util::pin_mut!(try_once);
        match select(try_once, async_io::Timer::after(left)).await {
            Either::Left((Ok(conn), _)) => {
                link.set(conn.clone());
                eprintln!("halod: {BUS_NAME} de volta no D-Bus da sessão");
                crate::session::reconcile(&conn, worker).await;
                return Ok(());
            }
            Either::Left((Err(e), _)) => last = e,
            Either::Right(_) => last = "tempo esgotado".into(),
        }
        let left = until.saturating_duration_since(Instant::now());
        async_io::Timer::after(every.min(left)).await;
    }
}

/// Reconecta toda vez que a conexão cair. Só termina sem o nome no prazo,
/// com o motivo.
pub async fn keep_alive(link: &BusLink, connect: &Connect, worker: &Worker) -> String {
    loop {
        if let Err(e) = reconnect(link, connect, worker, (DEADLINE, EVERY)).await {
            return e;
        }
    }
}

/// Uma conexão nova com o estado de antes, e só então o nome, com as regras
/// de [`crate::bus::claim_name`]: sem tomar o nome de outro processo.
async fn attempt(connect: &Connect, carried: &Carried) -> Result<Connection, String> {
    let conn = connect()
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;
    carried
        .restore(conn.object_server())
        .await
        .map_err(|e| e.to_string())?;
    match conn
        .request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into())
        .await
    {
        Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => Ok(conn),
        Ok(reply) => Err(format!("o nome está com outro processo ({reply:?})")),
        Err(zbus::Error::NameTaken) => Err("o nome está com outro processo".into()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::{Telemetry, WATERFORCE_PATH};
    use crate::testbus::Harness;
    use zbus::zvariant::OwnedValue;

    /// Prazo e intervalo curtos, para os testes.
    const FAST: (Duration, Duration) = (Duration::from_secs(2), Duration::from_millis(100));

    fn has_owner(conn: &Connection) -> bool {
        zbus::block_on(conn.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "NameHasOwner",
            &(BUS_NAME,),
        ))
        .unwrap()
        .body()
        .deserialize::<bool>()
        .unwrap()
    }

    fn reconnecting(
        h: &Harness,
        link: &Arc<BusLink>,
    ) -> std::thread::JoinHandle<Result<(), String>> {
        let (link, connect, worker) = (link.clone(), h.connect(), h.worker.clone());
        std::thread::spawn(move || zbus::block_on(reconnect(&link, &connect, &worker, FAST)))
    }

    #[test]
    fn reconecta_no_mesmo_processo_com_o_estado() {
        let mut h = Harness::new();
        // Estado só em memória: uma telemetria que o sysfs falso não dá.
        zbus::block_on(async {
            let t = h
                .server
                .object_server()
                .interface::<_, Telemetry>(WATERFORCE_PATH)
                .await
                .unwrap();
            t.get_mut().await.state.fan_rpm = 1234;
        });
        let before = zbus::block_on(Carried::take(h.server.object_server())).unwrap();
        let link = Arc::new(BusLink::new(h.server.clone()));
        h.kill_bus();
        let task = reconnecting(&h, &link);
        // Algumas tentativas falham antes de o barramento voltar.
        std::thread::sleep(Duration::from_millis(400));
        h.restart_bus();
        task.join().unwrap().unwrap();

        let conn = link.get();
        assert!(!conn.is_closed());
        assert_eq!(
            zbus::block_on(Carried::take(conn.object_server())).unwrap(),
            before
        );
        let client = h.new_client();
        assert!(has_owner(&client));
        let fan: OwnedValue = zbus::block_on(client.call_method(
            Some(BUS_NAME),
            WATERFORCE_PATH,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("io.github.leoodaviid.Halo1.Waterforce.Telemetry", "FanRpm"),
        ))
        .unwrap()
        .body()
        .deserialize()
        .unwrap();
        assert_eq!(u32::try_from(fan).unwrap(), 1234);
    }

    #[test]
    fn nome_com_outro_processo_e_falha_no_prazo() {
        let mut h = Harness::new();
        let link = Arc::new(BusLink::new(h.server.clone()));
        h.kill_bus();
        h.restart_bus();
        let other = h.new_client();
        zbus::block_on(other.request_name(BUS_NAME)).unwrap();
        let started = Instant::now();
        let err = reconnecting(&h, &link).join().unwrap().unwrap_err();
        let took = started.elapsed();
        assert!(err.contains("o nome está com outro processo"), "{err}");
        assert!(
            took >= FAST.0 && took < FAST.0 + Duration::from_secs(1),
            "{took:?}"
        );
    }

    #[test]
    fn barramento_que_nao_volta_e_falha_no_prazo() {
        let mut h = Harness::new();
        let link = Arc::new(BusLink::new(h.server.clone()));
        h.kill_bus();
        let started = Instant::now();
        let err = reconnecting(&h, &link).join().unwrap().unwrap_err();
        let took = started.elapsed();
        assert!(err.contains(&format!("sem o nome {BUS_NAME}")), "{err}");
        assert!(
            took >= FAST.0 && took < FAST.0 + Duration::from_secs(1),
            "{took:?}"
        );
    }
}
