// SPDX-License-Identifier: GPL-2.0-or-later

//! Guarda da sessão gráfica (`halod-outside-session`). Com o linger, o
//! `halod` continua vivo depois do logout. Se o acesso ao Castor3 vier só da
//! ACL da sessão, ele some quando o greeter assume o assento, e o aparelho
//! ficaria no último Tctl frio. A unidade `halod-session.service` para antes
//! do `graphical-session.target` e chama [`SessionGuard::end`], que só
//! responde depois de a proteção sair: é a barreira do logout. O
//! [`SessionGuard::begin`] do login retoma o feed.
//!
//! `halod --session begin|end` ([`run_client`]) é o lado da unidade.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::future::{Either, select};
use halo_core::waterforce::cpu_feed::Protected;
use halo_core::waterforce::find_hidraw;
use zbus::interface;
use zbus::proxy::MethodFlags;
use zbus::{Connection, connection};

use crate::access::{self, Access};
use crate::bus::{BUS_NAME, ROOT_PATH};
use crate::error::Error;
use crate::feed::Feed;
use crate::worker::{FeedSlot, Worker};

/// A unidade da guarda.
pub const GUARD_UNIT: &str = "halod-session.service";
/// Prazo da chamada do `halod --session`, dentro do `TimeoutStopSec=35s` da
/// guarda junto com o helper (18 s).
pub const CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// Prazo da leitura do estado da guarda no início.
const GUARD_QUERY_TIMEOUT: Duration = Duration::from_secs(5);

/// Classifica o acesso ao Castor3 agora. Os testes usam uma falsa.
pub type AccessProbe = Arc<dyn Fn() -> Access + Send + Sync>;

/// O acesso ao nó do Castor3 achado no sysfs. Sem o aparelho, não há o que
/// comprovar, e conta como só ACL.
pub fn sysfs_probe(sys_root: PathBuf) -> AccessProbe {
    Arc::new(move || match find_hidraw(&sys_root) {
        Ok(Some(info)) => access::classify(&info.devnode),
        Ok(None) => Access::AclOnly("Castor3 ausente".into()),
        Err(e) => Access::AclOnly(format!("sysfs ilegível: {e}")),
    })
}

/// Com o feed pausado por sessão e o Castor3 presente, os ticks chamam isto
/// a cada volta: o aparelho pode ter aparecido depois do início (boot), ter
/// voltado depois de desconectado, ou a regra udev pode ter mudado. Com o
/// acesso pelo grupo comprovado, retoma como o `Begin()`, com `E1` e 8 s em
/// 80 °C, sem esperar um login. Devolve se retomou.
pub fn resume_if_group(feed: &Feed, access: &AccessProbe) -> bool {
    if !feed.session_paused() {
        return false;
    }
    let access = access();
    if !matches!(access, Access::Group(_)) {
        return false;
    }
    let resumed = feed.resume_session();
    if resumed {
        eprintln!(
            "halod: {}; retomando o feed da CPU sem sessão gráfica, com E1 e 8 s em 80 °C",
            access.describe()
        );
    }
    resumed
}

pub struct SessionGuard {
    worker: Worker,
    access: AccessProbe,
}

impl SessionGuard {
    pub fn new(worker: Worker, access: AccessProbe) -> Self {
        Self { worker, access }
    }
}

#[interface(name = "io.github.leoodaviid.Halo1.Session")]
impl SessionGuard {
    /// Login. Só com o feed pausado por sessão abre uma sessão lógica nova
    /// (`E1` e 8 s em 80 °C); fora dela, só responde.
    async fn begin(&self) {
        let feed = self.worker.feed();
        if feed.resume_session() {
            eprintln!(
                "halod: sessão gráfica de volta; retomando o feed da CPU com E1 e 8 s em 80 °C"
            );
            self.worker.request_feed(FeedSlot::Tick);
        }
    }

    /// Logout. Com acesso só pela ACL, responde depois de o `E0` de 80 °C ser
    /// aceito e o feed pausado; com acesso pelo grupo, na hora.
    async fn end(&self) -> Result<(), Error> {
        if !self.worker.feed().supervised() {
            return Ok(());
        }
        let access = (self.access)();
        eprintln!("halod: fim da sessão gráfica; {}", access.describe());
        if let Access::Group(_) = access {
            return Ok(());
        }
        match self.worker.end_session().await {
            Some(Ok(Protected::Sent(m))) => {
                eprintln!(
                    "halod: E0 de 80 °C aceito pelo modelo {m}; feed da CPU pausado até o próximo login"
                );
                Ok(())
            }
            Some(Ok(_)) | None => {
                eprintln!("halod: feed da CPU pausado até o próximo login");
                Ok(())
            }
            Some(Err(e)) => {
                eprintln!("halod: feed da CPU pausado, mas o aparelho NÃO foi protegido: {e}");
                Err(e.with_context("aparelho não protegido no fim da sessão"))
            }
        }
    }
}

#[zbus::proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1"
)]
trait Systemd1Manager {
    fn get_unit(&self, name: &str) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1"
)]
trait Systemd1Unit {
    #[zbus(property)]
    fn active_state(&self) -> zbus::Result<String>;
}

/// `ActiveState` da guarda no gerenciador do usuário. Uma unidade não
/// carregada (`NoSuchUnit`) está inativa; sem o gerenciador, `Err`.
pub async fn guard_state(conn: &Connection) -> Result<String, String> {
    let query = async {
        let manager = Systemd1ManagerProxy::new(conn).await?;
        let path = match manager.get_unit(GUARD_UNIT).await {
            Ok(path) => path,
            Err(zbus::Error::MethodError(name, _, _))
                if name.as_str() == "org.freedesktop.systemd1.NoSuchUnit" =>
            {
                return Ok("inactive".to_owned());
            }
            Err(e) => return Err(e),
        };
        Systemd1UnitProxy::builder(conn)
            .path(path)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?
            .active_state()
            .await
    };
    futures_util::pin_mut!(query);
    match select(query, async_io::Timer::after(GUARD_QUERY_TIMEOUT)).await {
        Either::Left((result, _)) => result.map_err(|e| e.to_string()),
        Either::Right(_) => Err(format!(
            "sem resposta em {}s",
            GUARD_QUERY_TIMEOUT.as_secs()
        )),
    }
}

/// A guarda conta como ativa também subindo: o `Begin()` dela pode estar
/// esperando este daemon pegar o nome.
fn guard_up(state: &str) -> bool {
    matches!(state, "active" | "activating" | "reloading")
}

/// Estado inicial do feed, lido uma vez antes de pegar o nome: com a guarda
/// inativa (boot pelo linger, antes de qualquer login) e acesso só pela
/// ACL, o feed começa pausado até o `Begin()`. Devolve se pausou.
pub async fn initial_state(conn: &Connection, worker: &Worker, access: &AccessProbe) -> bool {
    if !worker.feed().supervised() {
        return false;
    }
    let guard = guard_state(conn).await;
    let up = match &guard {
        Ok(state) => guard_up(state),
        Err(e) => {
            eprintln!(
                "halod: aviso: estado de {GUARD_UNIT} desconhecido ({e}); tratando a sessão gráfica como ausente"
            );
            false
        }
    };
    let access = access();
    eprintln!("halod: {}", access.describe());
    if up || matches!(access, Access::Group(_)) {
        return false;
    }
    worker.feed().pause_session();
    eprintln!(
        "halod: sem sessão gráfica ({}), e o Castor3 só abre com ela; feed da CPU pausado até o login",
        guard.as_deref().unwrap_or("guarda desconhecida")
    );
    true
}

/// Depois de reconectar ao D-Bus da sessão: com o feed pausado por sessão e
/// a guarda ativa, retoma como o `Begin()`, que pode ter se perdido durante a
/// queda. Devolve `true` se retomou.
pub async fn reconcile(conn: &Connection, worker: &Worker) -> bool {
    if !worker.feed().session_paused() {
        return false;
    }
    match guard_state(conn).await {
        Ok(state) if guard_up(&state) => {}
        Ok(_) => return false,
        Err(e) => {
            eprintln!(
                "halod: aviso: estado de {GUARD_UNIT} desconhecido depois de reconectar ({e})"
            );
            return false;
        }
    }
    if !worker.feed().resume_session() {
        return false;
    }
    eprintln!(
        "halod: {GUARD_UNIT} ativa depois de reconectar; retomando o feed da CPU com E1 e 8 s em 80 °C"
    );
    worker.request_feed(FeedSlot::Tick);
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Step {
    Begin,
    End,
}

/// Chama `Begin` ou `End` no daemon, com [`CALL_TIMEOUT`]. O `End` não sobe
/// o daemon pela ativação do D-Bus.
async fn call(address: Option<&str>, step: Step) -> Result<(), String> {
    let call = async {
        let builder = match address {
            Some(a) => connection::Builder::address(a)?,
            None => connection::Builder::session()?,
        };
        let conn = builder.build().await?;
        let proxy = zbus::Proxy::new(
            &conn,
            BUS_NAME,
            ROOT_PATH,
            "io.github.leoodaviid.Halo1.Session",
        )
        .await?;
        let (method, flags) = match step {
            Step::Begin => ("Begin", Default::default()),
            Step::End => ("End", MethodFlags::NoAutoStart.into()),
        };
        proxy
            .call_with_flags::<_, _, ()>(method, flags, &())
            .await
            .map(|_| ())
    };
    futures_util::pin_mut!(call);
    match select(call, async_io::Timer::after(CALL_TIMEOUT)).await {
        Either::Left((result, _)) => result.map_err(|e| match e {
            zbus::Error::MethodError(name, Some(msg), _) => format!("{name}: {msg}"),
            e => e.to_string(),
        }),
        Either::Right(_) => Err(format!("sem resposta em {}s", CALL_TIMEOUT.as_secs())),
    }
}

/// `halod --session begin|end`. No `end`, sem resposta, com erro ou sem o
/// daemon no barramento, roda a proteção do `ExecStopPost` e sai com falha.
/// No `begin`, uma falha só é registrada e não falha a guarda: com ela
/// parada, o `End()` do logout não rodaria.
pub fn run_client(
    address: Option<&str>,
    step: Step,
    sys_root: &Path,
    proc_root: &Path,
    node: Option<&Path>,
) -> ExitCode {
    let started = Instant::now();
    let result = zbus::block_on(call(address, step));
    match (step, result) {
        (_, Ok(())) => ExitCode::SUCCESS,
        (Step::Begin, Err(e)) => {
            eprintln!("halod: Session.Begin() falhou: {e}; a guarda continua armada para o logout");
            ExitCode::SUCCESS
        }
        (Step::End, Err(e)) => {
            eprintln!(
                "halod: Session.End() falhou em {:.1}s: {e}; rodando a proteção sem o daemon",
                started.elapsed().as_secs_f32()
            );
            crate::safe::run(sys_root, proc_root, node);
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::feed::fake::{FakeClock, FakeSensor};
    use crate::feed::{Feed, PERIOD};
    use crate::testbus::{Harness, TestBus};
    use crate::worker::fake::Call;

    const SESSION: &str = "io.github.leoodaviid.Halo1.Session";

    fn supervised(h_clock: &Arc<FakeClock>) -> Feed {
        Feed::new(true, FakeSensor::new(Some(40_200)), h_clock.clone())
    }

    fn harness(access: Access) -> (Harness, Arc<FakeClock>) {
        let clock = FakeClock::new();
        let h = Harness::with_feed(|_| {}, Some(supervised(&clock)));
        *h.access.lock().unwrap() = access;
        (h, clock)
    }

    fn acl_only() -> Access {
        Access::AclOnly("teste".into())
    }

    /// Um período: o tick marca o envio e a thread o executa.
    fn period(h: &Harness, clock: &FakeClock) {
        h.worker.request_feed(FeedSlot::Tick);
        std::thread::sleep(Duration::from_millis(30));
        clock.advance(PERIOD);
    }

    fn feeds_after(h: &Harness, from: usize) -> Vec<Call> {
        h.backend.calls()[from..].to_vec()
    }

    fn end(h: &Harness) -> Result<(), String> {
        h.call::<_, ()>(ROOT_PATH, SESSION, "End", &())
    }

    fn begin(h: &Harness) -> Result<(), String> {
        h.call::<_, ()>(ROOT_PATH, SESSION, "Begin", &())
    }

    /// Sessão ativa com o Tctl já no aparelho (depois da janela de 8 s).
    fn feeding(h: &Harness, clock: &FakeClock) {
        for _ in 0..5 {
            period(h, clock);
        }
        assert_eq!(h.backend.calls().last(), Some(&Call::Feed(41)));
    }

    #[test]
    fn linger_com_a_regra_antiga() {
        let (h, clock) = harness(acl_only());
        feeding(&h, &clock);
        // O End só responde depois de a proteção ser aceita.
        h.backend.hold(true);
        let before = h.backend.calls().len();
        let client = {
            let address = h.address().to_owned();
            std::thread::spawn(move || {
                zbus::block_on(async {
                    let conn = connection::Builder::address(address.as_str())
                        .unwrap()
                        .build()
                        .await
                        .unwrap();
                    conn.call_method(Some(BUS_NAME), ROOT_PATH, Some(SESSION), "End", &())
                        .await
                        .map(|_| Instant::now())
                })
            })
        };
        std::thread::sleep(Duration::from_millis(300));
        assert!(!client.is_finished(), "End respondeu antes da proteção");
        let released = Instant::now();
        h.backend.hold(false);
        let replied = client.join().unwrap().unwrap();
        assert!(replied >= released);
        assert_eq!(feeds_after(&h, before), [Call::Protect]);
        assert!(h.worker.feed().session_paused());
        // Nenhum E0 com Tctl depois da resposta.
        for _ in 0..15 {
            period(&h, &clock);
        }
        assert_eq!(feeds_after(&h, before), [Call::Protect]);
        assert!(!h.worker.feed().active());
    }

    #[test]
    fn protecao_que_falha_vira_erro_dbus() {
        let (h, clock) = harness(acl_only());
        feeding(&h, &clock);
        h.backend
            .fail_next([Some(Error::PermissionDenied("sem acesso".into()))]);
        assert_eq!(
            end(&h).unwrap_err(),
            "io.github.leoodaviid.Halo1.Error.PermissionDenied"
        );
        // Mesmo assim, pausado: nada frio depois.
        let before = h.backend.calls().len();
        for _ in 0..5 {
            period(&h, &clock);
        }
        assert!(feeds_after(&h, before).is_empty());
    }

    #[test]
    fn volta_a_sessao() {
        let (h, clock) = harness(acl_only());
        feeding(&h, &clock);
        end(&h).unwrap();
        let before = h.backend.calls().len();
        begin(&h).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        clock.advance(PERIOD);
        for _ in 0..4 {
            period(&h, &clock);
        }
        // Sessão nova: reset, E1 com 80 °C (o Feed(80) que abre a sessão),
        // 8 s em 80 °C e o Tctl.
        assert_eq!(
            feeds_after(&h, before),
            [
                Call::ResetCpu,
                Call::Feed(80),
                Call::Feed(80),
                Call::Feed(80),
                Call::Feed(80),
                Call::Feed(41),
            ]
        );
        assert!(!h.worker.feed().session_paused());
    }

    #[test]
    fn acesso_pelo_grupo_e_login() {
        let (h, clock) = harness(Access::Group(46));
        feeding(&h, &clock);
        let before = h.backend.calls().len();
        // End seguido de Begin: nada de E1 nem 80 °C, e o Tctl a cada 2 s.
        end(&h).unwrap();
        for _ in 0..3 {
            period(&h, &clock);
        }
        begin(&h).unwrap();
        for _ in 0..3 {
            period(&h, &clock);
        }
        assert_eq!(feeds_after(&h, before), vec![Call::Feed(41); 6]);
        assert!(h.worker.feed().active());
    }

    #[test]
    fn mascara_da_acl_nao_e_o_grupo() {
        // O nó real do achado: a sonda classifica pelo xattr, e o End protege.
        let x = {
            let mut x = 2u32.to_le_bytes().to_vec();
            for (tag, perm, id) in [
                (1u16, 6u16, u32::MAX),
                (2, 6, 1000),
                (4, 4, u32::MAX),
                (0x10, 6, u32::MAX),
                (0x20, 0, u32::MAX),
            ] {
                x.extend(tag.to_le_bytes());
                x.extend(perm.to_le_bytes());
                x.extend(id.to_le_bytes());
            }
            x
        };
        let access = access::decide(0o20660, 46, Ok(Some(x)), &[1000, 46]);
        let (h, clock) = harness(access);
        feeding(&h, &clock);
        let before = h.backend.calls().len();
        end(&h).unwrap();
        assert_eq!(feeds_after(&h, before), [Call::Protect]);
        assert!(h.worker.feed().session_paused());
    }

    #[test]
    fn tela_bloqueada_nao_muda_nada() {
        // Sem End nem Begin, o feed segue; um Begin solto não reinicia nada.
        let (h, clock) = harness(acl_only());
        feeding(&h, &clock);
        let before = h.backend.calls().len();
        begin(&h).unwrap();
        for _ in 0..3 {
            period(&h, &clock);
        }
        assert_eq!(feeds_after(&h, before), vec![Call::Feed(41); 3]);
    }

    #[test]
    fn feed_pausado_por_sessao_renova_o_watchdog() {
        let (h, clock) = harness(acl_only());
        feeding(&h, &clock);
        let feed = h.worker.feed().clone();
        while feed.progressed(true) {}
        end(&h).unwrap();
        // Mais de 20 s pausado com o Waterforce presente: cada scan renova.
        for _ in 0..12 {
            period(&h, &clock);
            assert!(feed.progressed(true));
        }
        // Depois do Begin, sem envio novo (o primeiro está preso no
        // transporte), não renova; com ele aceito, renova.
        h.backend.hold(true);
        begin(&h).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        assert!(!feed.progressed(true));
        h.backend.hold(false);
        std::thread::sleep(Duration::from_millis(30));
        assert!(feed.progressed(true));
        assert!(!feed.progressed(true));
    }

    #[test]
    fn pausa_sem_protecao_aceita_nao_renova_o_watchdog() {
        // Achado do cross-review: com o último E0 num Tctl frio, a pausa só
        // conta como progresso depois de a proteção do End() ser aceita.
        let (h, clock) = harness(acl_only());
        feeding(&h, &clock);
        let feed = h.worker.feed().clone();
        while feed.progressed(true) {}
        // Worker preso: a proteção fica na fila, e o End() não responde.
        h.backend.hold(true);
        let address = h.address().to_owned();
        let pending = std::thread::spawn(move || {
            let conn = zbus::blocking::connection::Builder::address(address.as_str())
                .unwrap()
                .build()
                .unwrap();
            conn.call_method(Some(BUS_NAME), ROOT_PATH, Some(SESSION), "End", &())
                .map(|_| ())
        });
        std::thread::sleep(Duration::from_millis(50));
        assert!(feed.session_paused());
        for _ in 0..12 {
            period(&h, &clock);
            assert!(
                !feed.progressed(true),
                "pausa sem proteção renovou o watchdog"
            );
        }
        // Proteção aceita: a pausa volta a renovar.
        h.backend.hold(false);
        pending.join().unwrap().unwrap();
        assert!(feed.progressed(true));
        period(&h, &clock);
        assert!(feed.progressed(true));

        // Proteção que falha: pausado e frio, sem renovar.
        let (h, clock) = harness(acl_only());
        feeding(&h, &clock);
        let feed = h.worker.feed().clone();
        while feed.progressed(true) {}
        h.backend
            .fail_next([Some(Error::Device("transporte".into()))]);
        assert!(end(&h).is_err());
        for _ in 0..3 {
            period(&h, &clock);
            assert!(!feed.progressed(true));
        }
    }

    #[test]
    fn sem_supervisao_end_nao_envia() {
        let h = Harness::new();
        *h.access.lock().unwrap() = acl_only();
        end(&h).unwrap();
        begin(&h).unwrap();
        assert!(h.backend.calls().is_empty());
    }

    /// O feed pausado sem sessão, com os ticks do daemon e a sonda do harness.
    fn paused_with_ticks(
        castor: bool,
        access: Access,
    ) -> (Harness, Arc<FakeClock>, crate::ticks::Ticks) {
        let clock = FakeClock::new();
        let h = Harness::with_feed(
            |sys| {
                if castor {
                    crate::testbus::add_hidraw(sys, "hidraw99", 0x1044, 0x7A4D, 0x0000);
                }
            },
            Some(supervised(&clock)),
        );
        *h.access.lock().unwrap() = access;
        h.worker.feed().pause_session();
        let probe: AccessProbe = {
            let access = h.access.clone();
            Arc::new(move || access.lock().unwrap().clone())
        };
        let ticks = crate::ticks::Ticks::with_worker(
            h.sys.clone(),
            h.model.clone(),
            h.worker.clone(),
            None,
        )
        .with_access(probe);
        (h, clock, ticks)
    }

    fn tick(h: &Harness, clock: &FakeClock, ticks: &crate::ticks::Ticks) {
        zbus::block_on(ticks.run_once(&h.server)).unwrap();
        std::thread::sleep(Duration::from_millis(30));
        clock.advance(PERIOD);
    }

    #[test]
    fn aparelho_que_aparece_depois_do_boot_retoma_pelo_grupo() {
        // Boot pelo linger: o Castor3 ainda não está no sysfs, o feed começou
        // pausado. O grupo dá acesso, mas sem o aparelho nada muda.
        let (h, clock, ticks) = paused_with_ticks(false, Access::Group(46));
        for _ in 0..3 {
            tick(&h, &clock, &ticks);
        }
        assert!(h.worker.feed().session_paused());
        assert!(h.backend.calls().is_empty());
        // Enumerado: retoma sem Begin, com a sequência segura.
        crate::testbus::add_hidraw(&h.sys, "hidraw99", 0x1044, 0x7A4D, 0x0000);
        for _ in 0..5 {
            tick(&h, &clock, &ticks);
        }
        assert!(!h.worker.feed().session_paused());
        assert_eq!(
            h.backend.calls(),
            [
                Call::ResetCpu,
                Call::Feed(80),
                Call::Feed(80),
                Call::Feed(80),
                Call::Feed(80),
                Call::Feed(41),
            ]
        );
    }

    #[test]
    fn pausado_so_pela_acl_continua_pausado() {
        // Presente, mas sem o grupo: nada sai até o Begin; trocar a regra
        // (o grupo passa a valer) retoma no tick seguinte.
        let (h, clock, ticks) = paused_with_ticks(true, acl_only());
        for _ in 0..5 {
            tick(&h, &clock, &ticks);
        }
        assert!(h.backend.calls().is_empty());
        *h.access.lock().unwrap() = Access::Group(46);
        tick(&h, &clock, &ticks);
        assert_eq!(h.backend.calls(), [Call::ResetCpu, Call::Feed(80)]);
    }

    // 1.3

    const UNIT_PATH: &str = "/org/freedesktop/systemd1/unit/halod_2dsession_2eservice";

    /// `org.freedesktop.systemd1` falso, com a guarda carregada.
    struct FakeManager;

    #[interface(name = "org.freedesktop.systemd1.Manager")]
    impl FakeManager {
        fn get_unit(&self, name: &str) -> zbus::zvariant::OwnedObjectPath {
            assert_eq!(name, GUARD_UNIT);
            zbus::zvariant::OwnedObjectPath::try_from(UNIT_PATH).unwrap()
        }
    }

    struct FakeUnit(String);

    #[interface(name = "org.freedesktop.systemd1.Unit")]
    impl FakeUnit {
        #[zbus(property)]
        fn active_state(&self) -> String {
            self.0.clone()
        }
    }

    fn fake_systemd(bus: &TestBus, state: &str) -> Connection {
        zbus::block_on(async {
            connection::Builder::address(bus.address.as_str())
                .unwrap()
                .name("org.freedesktop.systemd1")
                .unwrap()
                .serve_at("/org/freedesktop/systemd1", FakeManager)
                .unwrap()
                .serve_at(UNIT_PATH, FakeUnit(state.to_owned()))
                .unwrap()
                .build()
                .await
                .unwrap()
        })
    }

    fn client(bus: &TestBus) -> Connection {
        zbus::block_on(
            connection::Builder::address(bus.address.as_str())
                .unwrap()
                .build(),
        )
        .unwrap()
    }

    fn worker(clock: &Arc<FakeClock>) -> (Worker, crate::worker::fake::FakeBackend) {
        let backend = crate::worker::fake::FakeBackend::default();
        let (refresh, _wake) = async_channel::bounded(1);
        let w = Worker::spawn_with_feed(
            backend.clone(),
            crate::worker::KnownModel::default(),
            refresh,
            supervised(clock),
        );
        (w, backend)
    }

    fn probe(access: Access) -> AccessProbe {
        Arc::new(move || access.clone())
    }

    #[test]
    fn estado_da_guarda_pelo_gerenciador() {
        let bus = TestBus::start();
        let _systemd = fake_systemd(&bus, "deactivating");
        let conn = client(&bus);
        assert_eq!(zbus::block_on(guard_state(&conn)).unwrap(), "deactivating");
        for (state, up) in [
            ("active", true),
            ("activating", true),
            ("reloading", true),
            ("inactive", false),
            ("failed", false),
            ("deactivating", false),
        ] {
            assert_eq!(guard_up(state), up, "{state}");
        }
    }

    #[test]
    fn sem_acesso_no_boot() {
        let bus = TestBus::start();
        let _systemd = fake_systemd(&bus, "inactive");
        let conn = client(&bus);
        let clock = FakeClock::new();
        let (w, backend) = worker(&clock);
        assert!(zbus::block_on(initial_state(&conn, &w, &probe(acl_only()))));
        // Nada é enviado até o Begin.
        for _ in 0..6 {
            w.request_feed(FeedSlot::Tick);
            std::thread::sleep(Duration::from_millis(20));
            clock.advance(PERIOD);
        }
        assert!(backend.calls().is_empty());
        assert!(w.feed().resume_session());
        w.request_feed(FeedSlot::Tick);
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(backend.calls(), [Call::ResetCpu, Call::Feed(80)]);
    }

    #[test]
    fn guarda_ativa_ou_grupo_nao_pausam() {
        let bus = TestBus::start();
        let _systemd = fake_systemd(&bus, "activating");
        let conn = client(&bus);
        let clock = FakeClock::new();
        let (w, _) = worker(&clock);
        assert!(!zbus::block_on(initial_state(
            &conn,
            &w,
            &probe(acl_only())
        )));
        assert!(!w.feed().session_paused());

        let bus = TestBus::start();
        let _systemd = fake_systemd(&bus, "inactive");
        let conn = client(&bus);
        let (w, _) = worker(&clock);
        assert!(!zbus::block_on(initial_state(
            &conn,
            &w,
            &probe(Access::Group(46))
        )));
    }

    #[test]
    fn begin_perdido_na_reconexao() {
        // O feed pausado por sessão e a guarda ativa no barramento novo: a
        // reconciliação retoma como o Begin(), com E1 e 8 s em 80 °C.
        let bus = TestBus::start();
        let _systemd = fake_systemd(&bus, "active");
        let conn = client(&bus);
        let clock = FakeClock::new();
        let (w, backend) = worker(&clock);
        w.feed().pause_session();
        assert!(zbus::block_on(reconcile(&conn, &w)));
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(backend.calls(), [Call::ResetCpu, Call::Feed(80)]);
        assert!(!w.feed().session_paused());
        // Sem pausa, nada a fazer.
        assert!(!zbus::block_on(reconcile(&conn, &w)));

        // Guarda inativa ou desconhecida: continua pausado.
        for state in [Some("inactive"), None] {
            let bus = TestBus::start();
            let _systemd = state.map(|s| fake_systemd(&bus, s));
            let conn = client(&bus);
            let (w, backend) = worker(&clock);
            w.feed().pause_session();
            assert!(!zbus::block_on(reconcile(&conn, &w)), "{state:?}");
            assert!(w.feed().session_paused());
            std::thread::sleep(Duration::from_millis(30));
            assert!(backend.calls().is_empty());
        }
    }

    #[test]
    fn sem_o_gerenciador_no_barramento() {
        let bus = TestBus::start();
        let conn = client(&bus);
        let clock = FakeClock::new();
        let (w, _) = worker(&clock);
        assert!(zbus::block_on(guard_state(&conn)).is_err());
        // Só ACL: pausado; pelo grupo: não.
        assert!(zbus::block_on(initial_state(&conn, &w, &probe(acl_only()))));
        let (w, _) = worker(&clock);
        assert!(!zbus::block_on(initial_state(
            &conn,
            &w,
            &probe(Access::Group(46))
        )));
    }

    #[test]
    fn unidade_nao_carregada_esta_inativa() {
        let bus = TestBus::start();
        let _systemd = zbus::block_on(async {
            connection::Builder::address(bus.address.as_str())
                .unwrap()
                .name("org.freedesktop.systemd1")
                .unwrap()
                .serve_at("/org/freedesktop/systemd1", NoSuchUnit)
                .unwrap()
                .build()
                .await
                .unwrap()
        });
        assert_eq!(
            zbus::block_on(guard_state(&client(&bus))).unwrap(),
            "inactive"
        );
    }

    #[derive(zbus::DBusError, Debug)]
    #[zbus(prefix = "org.freedesktop.systemd1")]
    enum UnitError {
        #[zbus(error)]
        ZBus(zbus::Error),
        NoSuchUnit(String),
    }

    struct NoSuchUnit;

    #[interface(name = "org.freedesktop.systemd1.Manager")]
    impl NoSuchUnit {
        fn get_unit(&self, name: &str) -> Result<zbus::zvariant::OwnedObjectPath, UnitError> {
            Err(UnitError::NoSuchUnit(format!("Unit {name} not loaded.")))
        }
    }
}
