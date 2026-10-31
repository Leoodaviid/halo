// SPDX-License-Identifier: GPL-2.0-or-later

//! Fila de um aparelho: uma thread dona das chamadas bloqueantes ao
//! `halo-core`, que atende um pedido de cada vez, na ordem de chegada. Os
//! métodos D-Bus esperam a resposta sem bloquear o executor do zbus.
//!
//! O feed da CPU não entra na fila de pedidos: os ticks marcam um slot único
//! ([`Worker::request_feed`]), que a thread atende antes do próximo pedido.
//! Assim cabe no máximo um envio vencido entre dois pedidos, nenhum pedido
//! é intercalado e atrasos não acumulam envios.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use async_channel::Sender;
use halo_core::waterforce::cpu_feed::{self, CpuInfo, Fed, Protected, Session};
use halo_core::waterforce::led::{self, LedSetting};
use halo_core::waterforce::probe::model_name;
use halo_core::waterforce::speed::{
    self, CurveRequest, DesiredState, SetRequest, SnapshotStore, SpeedError, SpeedStatus,
    StateOutcome, Which,
};

use crate::error::Error;
use crate::feed::{FEED_BUDGET, Feed};

/// Modelo lido no `DE 00` (`r[2]`; ver [`model_name`]).
pub type Model = u8;

/// O que o daemon faz no Waterforce. A implementação real é o
/// [`CoreBackend`]; os testes usam um falso. Toda operação começa pelo
/// `DE 00` e devolve o modelo lido.
pub trait WaterforceBackend: Send {
    fn apply_led(&mut self, setting: &LedSetting) -> Result<Model, Error>;
    fn speed_status(&mut self) -> Result<SpeedStatus, Error>;
    fn set_presets(&mut self, request: &SetRequest) -> Result<Model, Error>;
    fn set_curve(&mut self, request: &CurveRequest) -> Result<Model, Error>;
    fn restore(&mut self, which: Which) -> Result<Model, Error>;
    /// `true` se escreveu, `false` se o aparelho já estava em `desired`.
    fn apply_state(&mut self, desired: &DesiredState) -> Result<(bool, Model), Error>;
    /// Um envio do feed da CPU (sessão nova ou `E0` de `celsius`).
    fn feed_cpu(&mut self, celsius: u8) -> Result<Fed, Error>;
    /// `DE`/`E1`/`E0` de 80 °C numa transação nova; a sessão recomeça.
    fn protect_cpu(&mut self) -> Result<Protected, Error>;
    /// Esquece a sessão do feed: o próximo envio recomeça pelo `DE`.
    fn reset_cpu(&mut self);
}

/// Backend sobre o `halo-core`, com o mesmo `SnapshotStore` da CLI.
pub struct CoreBackend {
    sys_root: PathBuf,
    store: Option<SnapshotStore>,
    cpu: CpuInfo,
    session: Option<Session>,
}

impl CoreBackend {
    /// Sem `store` (nem `XDG_STATE_HOME` nem `HOME`), as escritas de
    /// velocidade são recusadas antes de abrir o aparelho.
    #[cfg(test)]
    pub fn new(sys_root: PathBuf, store: Option<SnapshotStore>) -> Self {
        Self::with_cpu(sys_root, store, CpuInfo::unknown())
    }

    /// Com os dados fixos da CPU que vão no `E1` e no `E0`.
    pub fn with_cpu(sys_root: PathBuf, store: Option<SnapshotStore>, cpu: CpuInfo) -> Self {
        Self {
            sys_root,
            store,
            cpu,
            session: None,
        }
    }

    fn store(&self) -> Result<&SnapshotStore, Error> {
        self.store
            .as_ref()
            .ok_or_else(|| SpeedError::NoStateDir.into())
    }
}

impl WaterforceBackend for CoreBackend {
    fn apply_led(&mut self, setting: &LedSetting) -> Result<Model, Error> {
        Ok(led::apply(&self.sys_root, setting)?.model)
    }

    fn speed_status(&mut self) -> Result<SpeedStatus, Error> {
        Ok(speed::status(&self.sys_root)?)
    }

    fn set_presets(&mut self, request: &SetRequest) -> Result<Model, Error> {
        let applied = speed::set(&self.sys_root, self.store()?, request)?;
        Ok(applied.written.before.model)
    }

    fn set_curve(&mut self, request: &CurveRequest) -> Result<Model, Error> {
        let applied = speed::curve(&self.sys_root, self.store()?, request)?;
        Ok(applied.written.before.model)
    }

    fn restore(&mut self, which: Which) -> Result<Model, Error> {
        let applied = speed::restore(&self.sys_root, self.store()?, which)?;
        Ok(applied.written.before.model)
    }

    fn apply_state(&mut self, desired: &DesiredState) -> Result<(bool, Model), Error> {
        let applied = speed::apply_state(&self.sys_root, self.store()?, desired)?;
        Ok(match applied.outcome {
            StateOutcome::Written(w) => (true, w.before.model),
            StateOutcome::Unchanged(s) => (false, s.model),
        })
    }

    fn feed_cpu(&mut self, celsius: u8) -> Result<Fed, Error> {
        let deadline = Instant::now() + FEED_BUDGET;
        Ok(cpu_feed::feed(
            &self.sys_root,
            &mut self.session,
            &self.cpu,
            celsius,
            deadline,
        )?)
    }

    fn protect_cpu(&mut self) -> Result<Protected, Error> {
        self.session = None;
        let deadline = Instant::now() + FEED_BUDGET;
        Ok(cpu_feed::protect(&self.sys_root, &self.cpu, deadline)?)
    }

    fn reset_cpu(&mut self) {
        self.session = None;
    }
}

/// Modelo do Waterforce que as operações do worker já leram, compartilhado
/// com os ticks. `None` até a primeira operação que deu certo; não é guardado
/// entre execuções do daemon.
#[derive(Clone, Default)]
pub struct KnownModel(Arc<Mutex<Option<Model>>>);

impl KnownModel {
    pub fn get(&self) -> Option<Model> {
        *self.0.lock().unwrap()
    }

    /// Grava `model` e diz se ele mudou.
    fn set(&self, model: Model) -> bool {
        self.0.lock().unwrap().replace(model) != Some(model)
    }
}

/// Envolve o backend e, depois de cada operação que deu certo com um modelo
/// conhecido, grava o modelo e pede uma volta dos ticks em `refresh`, que
/// montam o nome novo e emitem o `DevicesChanged`.
struct Observed<B> {
    inner: B,
    known: KnownModel,
    refresh: Sender<()>,
}

impl<B> Observed<B> {
    fn saw<T>(&self, result: Result<T, Error>, model: impl Fn(&T) -> Model) -> Result<T, Error> {
        if let Ok(value) = &result {
            let model = model(value);
            if model_name(model).is_some() && self.known.set(model) {
                // Canal cheio: os ticks já vão rodar.
                let _ = self.refresh.try_send(());
            }
        }
        result
    }
}

impl<B: WaterforceBackend> WaterforceBackend for Observed<B> {
    fn apply_led(&mut self, setting: &LedSetting) -> Result<Model, Error> {
        let r = self.inner.apply_led(setting);
        self.saw(r, |m| *m)
    }

    fn speed_status(&mut self) -> Result<SpeedStatus, Error> {
        let r = self.inner.speed_status();
        self.saw(r, |s| s.state.model)
    }

    fn set_presets(&mut self, request: &SetRequest) -> Result<Model, Error> {
        let r = self.inner.set_presets(request);
        self.saw(r, |m| *m)
    }

    fn set_curve(&mut self, request: &CurveRequest) -> Result<Model, Error> {
        let r = self.inner.set_curve(request);
        self.saw(r, |m| *m)
    }

    fn restore(&mut self, which: Which) -> Result<Model, Error> {
        let r = self.inner.restore(which);
        self.saw(r, |m| *m)
    }

    fn apply_state(&mut self, desired: &DesiredState) -> Result<(bool, Model), Error> {
        let r = self.inner.apply_state(desired);
        self.saw(r, |(_, m)| *m)
    }

    fn feed_cpu(&mut self, celsius: u8) -> Result<Fed, Error> {
        let r = self.inner.feed_cpu(celsius);
        if let Ok(Fed::Started(m) | Fed::Disabled(m)) = &r {
            let _ = self.saw(Ok(*m), |m| *m);
        }
        r
    }

    fn protect_cpu(&mut self) -> Result<Protected, Error> {
        let r = self.inner.protect_cpu();
        if let Ok(Protected::Sent(m) | Protected::NotFed(m)) = &r {
            let _ = self.saw(Ok(*m), |m| *m);
        }
        r
    }

    fn reset_cpu(&mut self) {
        self.inner.reset_cpu();
    }
}

type Job = Box<dyn FnOnce(&mut dyn WaterforceBackend) + Send>;

/// Pedido do feed no slot único da fila.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedSlot {
    /// Envio vencido do período.
    Tick,
    /// Proteção antes da suspensão; vence um `Tick` pendente.
    SleepProtect,
}

/// Resultado da última proteção: `None` sem supervisão (nada enviado).
pub type FinalProtect = Option<Result<Protected, Error>>;

#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    slot: Option<FeedSlot>,
    /// SIGTERM: a thread faz a última proteção, responde aqui e termina,
    /// sem atender pedido nem feed pendente.
    stop: Option<mpsc::Sender<FinalProtect>>,
    stopped: bool,
}

struct Shared {
    queue: Mutex<Queue>,
    ready: Condvar,
}

impl Shared {
    fn set_slot(&self, slot: FeedSlot) {
        let mut q = self.queue.lock().unwrap();
        if q.stopped || q.stop.is_some() {
            return;
        }
        q.slot = match (q.slot, slot) {
            (Some(FeedSlot::SleepProtect), _) => Some(FeedSlot::SleepProtect),
            (_, s) => Some(s),
        };
        self.ready.notify_one();
    }
}

enum Next {
    Stop(mpsc::Sender<FinalProtect>),
    Feed(FeedSlot),
    Job(Job),
}

/// Fila do Waterforce. Clonar compartilha a mesma thread.
#[derive(Clone)]
pub struct Worker {
    shared: Arc<Shared>,
    feed: Feed,
}

impl Worker {
    /// Sem feed da CPU (sem supervisão). O modelo lido pelas operações vai
    /// para `known`, com um aviso em `refresh` quando muda.
    #[cfg(test)]
    pub fn spawn(
        backend: impl WaterforceBackend + 'static,
        known: KnownModel,
        refresh: Sender<()>,
    ) -> Self {
        Self::spawn_with_feed(
            backend,
            known,
            refresh,
            Feed::unsupervised(PathBuf::from("/nonexistent")),
        )
    }

    /// Como [`Worker::spawn`], com o feed da CPU em `feed`.
    pub fn spawn_with_feed(
        backend: impl WaterforceBackend + 'static,
        known: KnownModel,
        refresh: Sender<()>,
        feed: Feed,
    ) -> Self {
        let mut backend = Observed {
            inner: backend,
            known,
            refresh,
        };
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            ready: Condvar::new(),
        });
        let worker = Self {
            shared: shared.clone(),
            feed: feed.clone(),
        };
        thread::Builder::new()
            .name("waterforce".into())
            .spawn(move || work(&shared, &mut backend, &feed))
            .expect("não consegui criar a thread do Waterforce");
        worker
    }

    pub fn feed(&self) -> &Feed {
        &self.feed
    }

    fn push(&self, job: Job) {
        let mut q = self.shared.queue.lock().unwrap();
        q.jobs.push_back(job);
        self.shared.ready.notify_one();
    }

    /// Põe `f` na fila e espera o resultado. Depois do SIGTERM, a fila não
    /// atende mais nada e a espera só acaba com o processo.
    pub async fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut dyn WaterforceBackend) -> T + Send + 'static,
    ) -> T {
        let (reply, result) = async_channel::bounded(1);
        self.push(Box::new(move |backend| {
            let _ = reply.send_blocking(f(backend));
        }));
        match result.recv().await {
            Ok(value) => value,
            Err(_) => std::future::pending().await,
        }
    }

    /// Como [`Worker::run`], para operações de velocidade: com o feed
    /// supervisionado, o próximo `E0` atual sai logo depois, antes do
    /// pedido seguinte.
    pub async fn run_speed<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut dyn WaterforceBackend) -> T + Send + 'static,
    ) -> T {
        let shared = self.shared.clone();
        let supervised = self.feed.supervised();
        self.run(move |backend| {
            let value = f(backend);
            if supervised {
                shared.set_slot(FeedSlot::Tick);
            }
            value
        })
        .await
    }

    /// Marca um envio do feed. Um pedido já pendente não acumula outro.
    pub fn request_feed(&self, slot: FeedSlot) {
        self.shared.set_slot(slot);
    }

    /// `Session.End()` com acesso só pela ACL: pausa o feed, descarta o envio
    /// pendente e faz a proteção do SIGTERM antes dos pedidos da fila. Depois
    /// dela, nenhum `E0` com Tctl até o `Begin()`. `None` sem supervisão
    /// (nada enviado).
    pub async fn end_session(&self) -> FinalProtect {
        self.feed.pause_session();
        let (reply, result) = async_channel::bounded(1);
        {
            let mut q = self.shared.queue.lock().unwrap();
            if q.stopped || q.stop.is_some() {
                return Some(Err(Error::Device(
                    "o halod está parando; a proteção fica com o SIGTERM".into(),
                )));
            }
            if q.slot == Some(FeedSlot::Tick) {
                q.slot = None;
            }
            let feed = self.feed.clone();
            q.jobs.push_front(Box::new(move |backend| {
                let _ = reply.send_blocking(feed.final_protect(backend));
            }));
            self.shared.ready.notify_one();
        }
        match result.recv().await {
            Ok(value) => value,
            Err(_) => std::future::pending().await,
        }
    }

    /// SIGTERM: nenhum feed nem pedido novo; espera a operação em andamento
    /// terminar e faz a última proteção, até `timeout`. `None` se o prazo
    /// vencer (a thread pode estar presa).
    pub fn shutdown(&self, timeout: Duration) -> Option<FinalProtect> {
        self.feed.begin_stop();
        let (tx, rx) = mpsc::channel();
        {
            let mut q = self.shared.queue.lock().unwrap();
            if q.stopped || q.stop.is_some() {
                return None;
            }
            q.slot = None;
            q.stop = Some(tx);
            self.shared.ready.notify_one();
        }
        rx.recv_timeout(timeout).ok()
    }
}

fn work(shared: &Shared, backend: &mut dyn WaterforceBackend, feed: &Feed) {
    loop {
        let next = {
            let mut q = shared.queue.lock().unwrap();
            loop {
                if let Some(tx) = q.stop.take() {
                    q.stopped = true;
                    break Next::Stop(tx);
                }
                if let Some(slot) = q.slot.take() {
                    break Next::Feed(slot);
                }
                if let Some(job) = q.jobs.pop_front() {
                    break Next::Job(job);
                }
                q = shared.ready.wait(q).unwrap();
            }
        };
        match next {
            Next::Stop(tx) => {
                let _ = tx.send(feed.final_protect(backend));
                return;
            }
            Next::Feed(FeedSlot::Tick) => feed.step(backend),
            Next::Feed(FeedSlot::SleepProtect) => feed.sleep_protect(backend),
            Next::Job(job) => job(backend),
        }
    }
}

#[cfg(test)]
pub(crate) mod fake {
    //! Backend falso: grava as chamadas e devolve respostas programadas.

    use std::collections::VecDeque;
    use std::sync::{Arc, Condvar, Mutex};

    use halo_core::waterforce::probe::CurvePoint;
    use halo_core::waterforce::speed::Snapshot;

    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    pub enum Call {
        Led(LedSetting),
        Status,
        Presets(SetRequest),
        Curve(CurveRequest),
        Restore(Which),
        State(DesiredState),
        /// Envio do feed com a temperatura pedida.
        Feed(u8),
        Protect,
        ResetCpu,
    }

    /// Respostas programadas e o registro das chamadas, compartilhados com o
    /// teste.
    #[derive(Default)]
    pub struct Script {
        pub calls: Vec<Call>,
        /// Erros a devolver, na ordem das chamadas; vazio = sucesso.
        pub failures: VecDeque<Option<Error>>,
        /// Enquanto `true`, a próxima chamada espera.
        pub hold: bool,
        /// Modelo devolvido pelas operações que dão certo.
        pub model: Model,
        /// Sessão do feed aberta: o próximo envio é só `E0`.
        pub feed_session: bool,
        /// Modelo sem feed: os envios devolvem `Disabled`.
        pub disabled_model: Option<Model>,
    }

    #[derive(Clone, Default)]
    pub struct FakeBackend {
        pub script: Arc<(Mutex<Script>, Condvar)>,
    }

    impl FakeBackend {
        pub fn calls(&self) -> Vec<Call> {
            self.script.0.lock().unwrap().calls.clone()
        }

        pub fn fail_next(&self, failures: impl IntoIterator<Item = Option<Error>>) {
            self.script.0.lock().unwrap().failures.extend(failures);
        }

        pub fn hold(&self, on: bool) {
            let (lock, cond) = &*self.script;
            lock.lock().unwrap().hold = on;
            cond.notify_all();
        }

        /// Esquece a sessão do feed sem `ResetCpu`, como o aparelho
        /// reconectado (identidade nova): o próximo envio recomeça.
        pub fn lose_session(&self) {
            self.script.0.lock().unwrap().feed_session = false;
        }

        pub fn set_model(&self, model: Model) {
            self.script.0.lock().unwrap().model = model;
        }

        pub fn set_disabled_model(&self, model: Option<Model>) {
            self.script.0.lock().unwrap().disabled_model = model;
        }

        /// Como `record`, com acesso ao script depois do registro.
        fn record_with<T>(
            &self,
            call: Call,
            ok: impl FnOnce(&mut Script) -> T,
        ) -> Result<T, Error> {
            let (lock, cond) = &*self.script;
            let mut script = cond.wait_while(lock.lock().unwrap(), |s| s.hold).unwrap();
            script.calls.push(call);
            match script.failures.pop_front().flatten() {
                Some(e) => {
                    script.feed_session = false;
                    Err(e)
                }
                None => Ok(ok(&mut script)),
            }
        }

        fn record(&self, call: Call) -> Result<Model, Error> {
            let (lock, cond) = &*self.script;
            let mut script = cond.wait_while(lock.lock().unwrap(), |s| s.hold).unwrap();
            script.calls.push(call);
            match script.failures.pop_front().flatten() {
                Some(e) => Err(e),
                None => Ok(script.model),
            }
        }
    }

    /// Estado devolvido pelo status falso: fan em Performance, bomba em Turbo.
    pub fn status(model: Model) -> SpeedStatus {
        let points = |rpm: [u16; 4]| {
            let celsius = [0, 30, 50, 80];
            std::array::from_fn(|i| CurvePoint {
                celsius: celsius[i],
                rpm: rpm[i],
            })
        };
        SpeedStatus {
            devnode: PathBuf::from("/dev/hidraw3"),
            state: Snapshot {
                taken_unix: 0,
                model,
                fan_mode: 5,
                pump_mode: 4,
                fan_curve: points([1000, 1000, 1500, 2500]),
                pump_curve: points([2800; 4]),
            },
        }
    }

    impl WaterforceBackend for FakeBackend {
        fn apply_led(&mut self, setting: &LedSetting) -> Result<Model, Error> {
            self.record(Call::Led(setting.clone()))
        }

        fn speed_status(&mut self) -> Result<SpeedStatus, Error> {
            self.record(Call::Status).map(status)
        }

        fn set_presets(&mut self, request: &SetRequest) -> Result<Model, Error> {
            self.record(Call::Presets(*request))
        }

        fn set_curve(&mut self, request: &CurveRequest) -> Result<Model, Error> {
            self.record(Call::Curve(*request))
        }

        fn restore(&mut self, which: Which) -> Result<Model, Error> {
            self.record(Call::Restore(which))
        }

        fn apply_state(&mut self, desired: &DesiredState) -> Result<(bool, Model), Error> {
            self.record(Call::State(*desired)).map(|m| (true, m))
        }

        fn feed_cpu(&mut self, celsius: u8) -> Result<Fed, Error> {
            self.record_with(Call::Feed(celsius), |s| {
                if let Some(m) = s.disabled_model {
                    Fed::Disabled(m)
                } else if s.feed_session {
                    Fed::Data
                } else {
                    s.feed_session = true;
                    Fed::Started(s.model)
                }
            })
        }

        fn protect_cpu(&mut self) -> Result<Protected, Error> {
            self.record_with(Call::Protect, |s| {
                s.feed_session = false;
                match s.disabled_model {
                    Some(m) => Protected::NotFed(m),
                    None => Protected::Sent(s.model),
                }
            })
        }

        fn reset_cpu(&mut self) {
            let mut script = self.script.0.lock().unwrap();
            script.calls.push(Call::ResetCpu);
            script.feed_session = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{Call, FakeBackend};
    use super::*;
    use std::time::Duration;

    #[test]
    fn tarefas_em_ordem_e_uma_de_cada_vez() {
        let backend = FakeBackend::default();
        let (refresh, _wake) = async_channel::bounded(1);
        let worker = Worker::spawn(backend.clone(), KnownModel::default(), refresh);
        backend.hold(true);
        let first = {
            let worker = worker.clone();
            thread::spawn(move || zbus::block_on(worker.run(|b| b.restore(Which::Previous))))
        };
        thread::sleep(Duration::from_millis(50));
        let second = {
            let worker = worker.clone();
            thread::spawn(move || zbus::block_on(worker.run(|b| b.restore(Which::Original))))
        };
        thread::sleep(Duration::from_millis(50));
        // A primeira está presa no backend; a segunda espera na fila.
        assert!(backend.calls().is_empty());
        backend.hold(false);
        first.join().unwrap().unwrap();
        second.join().unwrap().unwrap();
        assert_eq!(
            backend.calls(),
            [
                Call::Restore(Which::Previous),
                Call::Restore(Which::Original)
            ]
        );
    }

    fn supervised_worker(backend: &FakeBackend) -> Worker {
        supervised_worker_with_clock(backend).0
    }

    fn supervised_worker_with_clock(
        backend: &FakeBackend,
    ) -> (Worker, Arc<crate::feed::fake::FakeClock>) {
        use crate::feed::fake::{FakeClock, FakeSensor};
        let (refresh, _wake) = async_channel::bounded(1);
        let clock = FakeClock::new();
        let feed = Feed::new(true, FakeSensor::new(Some(40_000)), clock.clone());
        let worker = Worker::spawn_with_feed(backend.clone(), KnownModel::default(), refresh, feed);
        (worker, clock)
    }

    /// Põe um `restore` na fila numa thread e devolve o handle.
    fn queued(worker: &Worker, which: Which) -> thread::JoinHandle<Result<Model, Error>> {
        let worker = worker.clone();
        let handle = thread::spawn(move || zbus::block_on(worker.run(move |b| b.restore(which))));
        thread::sleep(Duration::from_millis(30));
        handle
    }

    fn wait_calls(backend: &FakeBackend, n: usize) {
        let started = std::time::Instant::now();
        while backend.calls().len() < n {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{:?}",
                backend.calls()
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn cpu_feed_slot_unico_sem_backlog() {
        let backend = FakeBackend::default();
        let worker = supervised_worker(&backend);
        backend.hold(true);
        let first = queued(&worker, Which::Previous);
        // Três períodos perdidos enquanto o backend está preso.
        for _ in 0..3 {
            worker.request_feed(FeedSlot::Tick);
        }
        backend.hold(false);
        first.join().unwrap().unwrap();
        wait_calls(&backend, 2);
        thread::sleep(Duration::from_millis(50));
        assert_eq!(
            backend.calls(),
            [Call::Restore(Which::Previous), Call::Feed(80)]
        );
    }

    #[test]
    fn cpu_feed_no_maximo_um_entre_pedidos_em_ordem() {
        let backend = FakeBackend::default();
        let worker = supervised_worker(&backend);
        backend.hold(true);
        let a = queued(&worker, Which::Previous);
        let b = queued(&worker, Which::Original);
        let c = queued(&worker, Which::Previous);
        worker.request_feed(FeedSlot::Tick);
        backend.hold(false);
        for h in [a, b, c] {
            h.join().unwrap().unwrap();
        }
        // O feed vencido entra uma vez, entre dois pedidos inteiros; a
        // ordem dos pedidos não muda.
        assert_eq!(
            backend.calls(),
            [
                Call::Restore(Which::Previous),
                Call::Feed(80),
                Call::Restore(Which::Original),
                Call::Restore(Which::Previous),
            ]
        );
    }

    #[test]
    fn cpu_feed_fila_continua_nao_atrasa_o_feed() {
        let backend = FakeBackend::default();
        let (worker, clock) = supervised_worker_with_clock(&backend);
        // Pedidos sem parar; o feed vence a cada período e entra entre eles.
        for i in 0..5 {
            clock.advance(crate::feed::PERIOD);
            backend.hold(true);
            let h = queued(&worker, Which::Previous);
            let next = queued(&worker, Which::Original);
            worker.request_feed(FeedSlot::Tick);
            backend.hold(false);
            h.join().unwrap().unwrap();
            next.join().unwrap().unwrap();
            let calls = backend.calls();
            assert!(
                matches!(calls[calls.len() - 2], Call::Feed(_)),
                "volta {i}: {calls:?}"
            );
        }
    }

    #[test]
    fn cpu_feed_velocidade_prioriza_o_proximo_e0() {
        let backend = FakeBackend::default();
        let worker = supervised_worker(&backend);
        zbus::block_on(worker.run_speed(|b| b.restore(Which::Previous))).unwrap();
        wait_calls(&backend, 2);
        assert_eq!(
            backend.calls(),
            [Call::Restore(Which::Previous), Call::Feed(80)]
        );
        // Sem supervisão, nada de feed.
        let plain = FakeBackend::default();
        let (refresh, _wake) = async_channel::bounded(1);
        let worker = Worker::spawn(plain.clone(), KnownModel::default(), refresh);
        zbus::block_on(worker.run_speed(|b| b.restore(Which::Previous))).unwrap();
        worker.request_feed(FeedSlot::Tick);
        thread::sleep(Duration::from_millis(50));
        assert_eq!(plain.calls(), [Call::Restore(Which::Previous)]);
    }

    #[test]
    fn cpu_feed_sigterm_descarta_pendentes_e_protege_por_ultimo() {
        let backend = FakeBackend::default();
        let worker = supervised_worker(&backend);
        backend.hold(true);
        let active = queued(&worker, Which::Previous);
        let _pending = queued(&worker, Which::Original);
        worker.request_feed(FeedSlot::Tick);
        let stopper = {
            let worker = worker.clone();
            thread::spawn(move || worker.shutdown(Duration::from_secs(5)))
        };
        thread::sleep(Duration::from_millis(30));
        backend.hold(false);
        active.join().unwrap().unwrap();
        let result = stopper.join().unwrap();
        assert!(
            matches!(result, Some(Some(Ok(Protected::Sent(0))))),
            "{result:?}"
        );
        // Nem o feed pendente nem o pedido seguinte rodam depois.
        worker.request_feed(FeedSlot::Tick);
        thread::sleep(Duration::from_millis(50));
        assert_eq!(
            backend.calls(),
            [Call::Restore(Which::Previous), Call::Protect]
        );
    }

    #[test]
    fn cpu_feed_sigterm_falha_de_transporte() {
        let backend = FakeBackend::default();
        let worker = supervised_worker(&backend);
        backend.fail_next([Some(Error::Busy("ocupado".into()))]);
        let result = worker.shutdown(Duration::from_secs(5));
        assert!(
            matches!(result, Some(Some(Err(Error::Busy(_))))),
            "{result:?}"
        );
    }

    #[test]
    fn cpu_feed_sigterm_worker_preso_tem_prazo() {
        let backend = FakeBackend::default();
        let worker = supervised_worker(&backend);
        backend.hold(true);
        let _stuck = queued(&worker, Which::Previous);
        let started = std::time::Instant::now();
        assert!(worker.shutdown(Duration::from_millis(200)).is_none());
        assert!(started.elapsed() < Duration::from_secs(1));
        backend.hold(false);
    }

    #[test]
    fn cpu_feed_sigterm_sem_supervisao_nao_envia() {
        let backend = FakeBackend::default();
        let (refresh, _wake) = async_channel::bounded(1);
        let worker = Worker::spawn(backend.clone(), KnownModel::default(), refresh);
        assert!(matches!(
            worker.shutdown(Duration::from_secs(5)),
            Some(None)
        ));
        assert!(backend.calls().is_empty());
    }

    #[test]
    fn backend_real_sem_diretorio_de_estado() {
        let sys = tempfile::tempdir().unwrap();
        let mut backend = CoreBackend::new(sys.path().to_owned(), None);
        let err = backend.restore(Which::Previous).unwrap_err();
        assert!(matches!(err, Error::Storage(_)), "{err:?}");
    }
}
