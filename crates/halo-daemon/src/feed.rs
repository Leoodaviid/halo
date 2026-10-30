// SPDX-License-Identifier: GPL-2.0-or-later

//! Alimentação da CPU no X 240 (`waterforce-cpu-feed`): o estado do feed e os
//! passos que a thread do Waterforce executa.
//!
//! Os ticks marcam um envio vencido a cada [`PERIOD`] num slot único da fila
//! ([`crate::worker::Worker::request_feed`]); a thread executa o passo, relê o
//! Tctl na hora e envia. Atraso perde o período, sem rajada nem amostra velha.
//! Cada sessão lógica começa com `DE`/`E1`/`E0` de 80 °C e fica
//! [`SAFE_WINDOW`] em 80 °C antes de usar o Tctl. Sem supervisão do watchdog
//! (execução manual ou unidade sem `WatchdogSec`), nada é enviado: um `E0`
//! frio só sai com alguém capaz de proteger depois de uma morte do processo.
//!
//! Com o acesso ao Castor3 só pela ACL da sessão, o `Session.End()` do logout
//! manda a proteção e pausa o feed ([`Feed::pause_session`]) até o
//! `Session.Begin()` do próximo login, que abre uma sessão lógica nova.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use halo_core::waterforce::cpu_feed::{self, Fed, PROTECT_CELSIUS, Protected};

use crate::error::Error;
use crate::worker::WaterforceBackend;

/// Período do `E0`, o mesmo dos ticks.
pub const PERIOD: Duration = Duration::from_secs(2);
/// Proteção no início de cada sessão: o maior atraso observado até o duty
/// seguir o `E0` (~5,9 s) mais um `DA` (~2 s). Não é limite físico garantido.
pub const SAFE_WINDOW: Duration = Duration::from_secs(8);
/// Intervalo mínimo entre dois envios do slot. Uma volta extra dos ticks
/// (modelo novo, operação de velocidade) não manda um `E0` colado ao
/// anterior; resume e sessão nova passam na hora.
pub const MIN_GAP: Duration = Duration::from_secs(1);
/// Idade máxima do último `E0` com Tctl para o `CpuFeedActive`.
pub const ACTIVE_MAX_AGE: Duration = Duration::from_secs(4);
/// Prazo de uma operação do feed na thread: `flock` (5 s) e `DE` (2 s).
pub const FEED_BUDGET: Duration = Duration::from_secs(7);
/// Prazo total da proteção no SIGTERM.
pub const STOP_BUDGET: Duration = Duration::from_secs(18);

/// Relógios do feed. Os testes usam um falso.
pub trait Clock: Send + Sync {
    /// Monotônico, parado durante a suspensão.
    fn now(&self) -> Instant;
    /// `CLOCK_BOOTTIME`, que conta o tempo suspenso.
    fn boottime(&self) -> Duration;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn boottime(&self) -> Duration {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: `ts` é um timespec válido durante a chamada.
        unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts) };
        Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
    }
}

/// Fonte do Tctl em m°C. Os testes usam uma falsa.
pub trait CpuSensor: Send + Sync {
    fn read(&self) -> Result<i32, String>;
}

/// Tctl do `k10temp` sob a raiz do sysfs.
pub struct SysfsSensor(pub PathBuf);

impl CpuSensor for SysfsSensor {
    fn read(&self) -> Result<i32, String> {
        cpu_feed::read_tctl(&self.0).map_err(|e| e.to_string())
    }
}

#[derive(Default)]
struct State {
    supervised: bool,
    sleeping: bool,
    stopping: bool,
    /// `Session.End()` com acesso só pela ACL: nenhum feed até o `Begin()`.
    session_paused: bool,
    /// Pedido de sessão nova (resume, salto do relógio).
    reset: bool,
    /// Fim da janela segura da sessão atual; `None` sem sessão.
    window_until: Option<Instant>,
    /// Último `E0` com Tctl aceito pelo transporte, depois da janela.
    last_data: Option<Instant>,
    /// Último envio aceito do slot (sessão nova ou dados).
    last_sent: Option<Instant>,
    /// Conta os envios aceitos (dados ou proteção), para o watchdog.
    ok_seq: u64,
    /// O `ok_seq` do último ping.
    pinged_seq: u64,
    /// Modelo sem feed na sessão atual.
    disabled: Option<u8>,
    /// O último `E0` aceito foi um Tctl, sem `E0` de 80 °C depois. Pausado
    /// assim (proteção do `End()` na fila ou falhando), o scan não renova o
    /// watchdog.
    cold: bool,
}

/// Estado do feed, compartilhado entre a thread do Waterforce, os ticks e o
/// D-Bus. Clonar compartilha o mesmo estado.
#[derive(Clone)]
pub struct Feed {
    state: Arc<Mutex<State>>,
    sensor: Arc<dyn CpuSensor>,
    clock: Arc<dyn Clock>,
    trace: bool,
}

impl Feed {
    pub fn new(supervised: bool, sensor: Arc<dyn CpuSensor>, clock: Arc<dyn Clock>) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                supervised,
                ..State::default()
            })),
            sensor,
            clock,
            trace: false,
        }
    }

    /// Com `trace`, cada envio do feed vai ao log (ensaios no aparelho,
    /// `HALOD_TRACE_FEED=1`).
    pub fn traced(mut self, trace: bool) -> Self {
        self.trace = trace;
        self
    }

    /// Sem supervisão, com o Tctl do sysfs: só publica a CPU.
    pub fn unsupervised(sys_root: PathBuf) -> Self {
        Self::new(
            false,
            Arc::new(SysfsSensor(sys_root)),
            Arc::new(SystemClock),
        )
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    pub fn supervised(&self) -> bool {
        self.state().supervised
    }

    pub fn clock(&self) -> &dyn Clock {
        &*self.clock
    }

    pub fn read_cpu(&self) -> Result<i32, String> {
        self.sensor.read()
    }

    /// Supervisão, fora de suspensão e parada, com um `E0` de Tctl aceito há
    /// no máximo [`ACTIVE_MAX_AGE`]. Não confirma RPM.
    pub fn active(&self) -> bool {
        let now = self.clock.now();
        let s = self.state();
        s.supervised
            && !s.stopping
            && !s.sleeping
            && !s.session_paused
            && s.last_data
                .is_some_and(|t| now.saturating_duration_since(t) <= ACTIVE_MAX_AGE)
    }

    /// Condição para gravar uma curva com degraus: supervisão, Tctl válido
    /// agora e feed ativo (o que exclui a janela segura).
    pub fn curve_allowed(&self) -> Result<(), Error> {
        let refuse = |why: &str| {
            Err(Error::Device(format!(
                "{why}; a curva com degraus exige o feed da CPU ativo, nada foi enviado ao aparelho"
            )))
        };
        if !self.supervised() {
            return refuse("o halod roda sem supervisão do watchdog");
        }
        if let Err(e) = self.read_cpu() {
            return refuse(&format!("Tctl inválido ({e})"));
        }
        if !self.active() {
            return refuse("o feed da CPU não está ativo (janela segura, atraso ou falha)");
        }
        Ok(())
    }

    /// Um envio vencido. Na janela ou sem sessão, 80 °C; depois, o Tctl lido
    /// agora. Falha de transporte tenta a proteção numa transação nova.
    pub fn step(&self, backend: &mut dyn WaterforceBackend) {
        let in_window = {
            let mut s = self.state();
            if !s.supervised || s.stopping || s.sleeping || s.session_paused {
                return;
            }
            let now = self.clock.now();
            if std::mem::take(&mut s.reset) {
                backend.reset_cpu();
                s.window_until = None;
                s.last_data = None;
                s.last_sent = None;
                s.disabled = None;
            }
            if s.last_sent
                .is_some_and(|t| now.saturating_duration_since(t) < MIN_GAP)
            {
                return;
            }
            s.window_until.is_none_or(|t| now < t)
        };
        let celsius = if in_window {
            PROTECT_CELSIUS
        } else {
            cpu_feed::wire_celsius(self.sensor.read().ok())
        };
        let result = backend.feed_cpu(celsius);
        if self.trace {
            match &result {
                Ok(Fed::Started(m)) => eprintln!("halod: feed: sessão nova, E0 80 °C (modelo {m})"),
                Ok(Fed::Data) => eprintln!("halod: feed: E0 {celsius} °C"),
                Ok(Fed::Disabled(m)) => eprintln!("halod: feed: modelo {m} sem feed, nada enviado"),
                Err(e) => eprintln!("halod: feed: E0 {celsius} °C falhou: {e}"),
            }
        }
        let now = self.clock.now();
        let mut s = self.state();
        match result {
            Ok(Fed::Started(_)) => {
                s.window_until = Some(now + SAFE_WINDOW);
                s.last_data = None;
                s.last_sent = Some(now);
                s.disabled = None;
                s.cold = false;
                s.ok_seq += 1;
            }
            Ok(Fed::Data) => {
                if !in_window {
                    s.last_data = Some(now);
                }
                s.last_sent = Some(now);
                s.cold = !in_window;
                s.ok_seq += 1;
            }
            Ok(Fed::Disabled(model)) => {
                if s.disabled != Some(model) {
                    eprintln!("halod: modelo {model} sem feed de CPU validado; nada enviado");
                }
                s.disabled = Some(model);
                s.window_until = None;
                s.last_data = None;
            }
            Err(Error::NotFound(_)) => {
                s.window_until = None;
                s.last_data = None;
                s.disabled = None;
            }
            Err(e) => {
                s.window_until = None;
                s.last_data = None;
                drop(s);
                eprintln!("halod: feed da CPU falhou ({e}); tentando E0 de 80 °C");
                self.recover(backend);
            }
        }
    }

    fn recover(&self, backend: &mut dyn WaterforceBackend) {
        match backend.protect_cpu() {
            Ok(Protected::Sent(_)) => {
                eprintln!("halod: E0 de 80 °C aceito depois da falha");
                let now = self.clock.now();
                let mut s = self.state();
                s.ok_seq += 1;
                s.cold = false;
                // O envio pendente (vencido durante a espera) não sai colado.
                s.last_sent = Some(now);
            }
            Ok(Protected::Absent) => {}
            Ok(Protected::NotFed(m)) => self.state().disabled = Some(m),
            Err(e) => eprintln!("halod: aparelho não protegido: {e}"),
        }
    }

    /// `PrepareForSleep(true)`: para os envios frios e tenta a proteção.
    /// Sem inibidor, nada garante que ela termine antes do sono.
    pub fn sleep_protect(&self, backend: &mut dyn WaterforceBackend) {
        if !self.supervised() || self.state().stopping {
            return;
        }
        match backend.protect_cpu() {
            Ok(p) => {
                if matches!(p, Protected::Sent(_)) {
                    self.state().cold = false;
                }
                eprintln!("halod: proteção antes da suspensão enviada")
            }
            Err(e) => eprintln!("halod: proteção antes da suspensão falhou: {e}"),
        }
    }

    /// Marca a suspensão: nenhum feed até [`Feed::resume`].
    pub fn begin_sleep(&self) {
        let mut s = self.state();
        s.sleeping = true;
        s.last_data = None;
    }

    /// Resume ou salto do relógio: sessão nova, com janela segura.
    pub fn resume(&self) {
        let mut s = self.state();
        s.sleeping = false;
        s.reset = true;
        s.window_until = None;
        s.last_data = None;
    }

    /// Logout com acesso só pela ACL: nenhum feed a partir daqui, até
    /// [`Feed::resume_session`]. Um envio em andamento na thread termina
    /// antes da proteção que vem depois na mesma thread.
    pub fn pause_session(&self) {
        let mut s = self.state();
        s.session_paused = true;
        s.window_until = None;
        s.last_data = None;
    }

    pub fn session_paused(&self) -> bool {
        self.state().session_paused
    }

    /// Login: só com o feed pausado por sessão, abre uma sessão lógica nova
    /// (`E1` e a janela segura). Devolve se retomou; fora da pausa, nada muda.
    pub fn resume_session(&self) -> bool {
        let mut s = self.state();
        if !s.session_paused {
            return false;
        }
        s.session_paused = false;
        s.reset = true;
        s.window_until = None;
        s.last_data = None;
        true
    }

    /// SIGTERM: nada mais de feed a partir daqui.
    pub fn begin_stop(&self) {
        let mut s = self.state();
        s.stopping = true;
        s.last_data = None;
    }

    /// Última proteção do processo. Sem supervisão nunca houve `E0` frio, e
    /// nada é enviado.
    pub fn final_protect(
        &self,
        backend: &mut dyn WaterforceBackend,
    ) -> Option<Result<Protected, Error>> {
        self.supervised().then(|| {
            let r = backend.protect_cpu();
            if let Ok(Protected::Sent(_)) = r {
                self.state().cold = false;
            }
            r
        })
    }

    /// Progresso para o watchdog: com o Waterforce ausente, sem feed pelo
    /// modelo ou com o feed pausado por sessão, basta o scan; presente, só um
    /// envio aceito novo desde o último ping. Pausado com um Tctl frio no
    /// aparelho (a proteção do `End()` ainda não aceita), não há progresso:
    /// o watchdog mata o processo e o `ExecStopPost` tenta a proteção.
    pub fn progressed(&self, waterforce_present: bool) -> bool {
        let mut s = self.state();
        if !waterforce_present || s.disabled.is_some() {
            return true;
        }
        if s.session_paused {
            if s.cold {
                return false;
            }
            // A proteção do `End()` não vale como envio novo depois do
            // `Begin()`.
            s.pinged_seq = s.ok_seq;
            return true;
        }
        if s.ok_seq > s.pinged_seq {
            s.pinged_seq = s.ok_seq;
            return true;
        }
        false
    }
}

#[cfg(test)]
pub(crate) mod fake {
    use super::*;

    /// Relógio parado, que o teste avança.
    pub struct FakeClock {
        pub now: Mutex<Instant>,
        pub boot: Mutex<Duration>,
    }

    impl FakeClock {
        pub fn new() -> Arc<Self> {
            Arc::new(Self {
                now: Mutex::new(Instant::now()),
                boot: Mutex::new(Duration::from_secs(1000)),
            })
        }

        pub fn advance(&self, d: Duration) {
            *self.now.lock().unwrap() += d;
            *self.boot.lock().unwrap() += d;
        }

        /// Suspensão: só o `CLOCK_BOOTTIME` anda.
        pub fn suspend(&self, d: Duration) {
            *self.boot.lock().unwrap() += d;
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Instant {
            *self.now.lock().unwrap()
        }

        fn boottime(&self) -> Duration {
            *self.boot.lock().unwrap()
        }
    }

    /// Tctl programado; `None` é falha de leitura.
    pub struct FakeSensor(pub Mutex<Option<i32>>);

    impl FakeSensor {
        pub fn new(mc: Option<i32>) -> Arc<Self> {
            Arc::new(Self(Mutex::new(mc)))
        }

        pub fn set(&self, mc: Option<i32>) {
            *self.0.lock().unwrap() = mc;
        }
    }

    impl CpuSensor for FakeSensor {
        fn read(&self) -> Result<i32, String> {
            (*self.0.lock().unwrap()).ok_or_else(|| "sem Tctl".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{FakeClock, FakeSensor};
    use super::*;
    use crate::worker::fake::{Call, FakeBackend};

    fn feed(supervised: bool, tctl: Option<i32>) -> (Feed, Arc<FakeClock>, Arc<FakeSensor>) {
        let clock = FakeClock::new();
        let sensor = FakeSensor::new(tctl);
        (
            Feed::new(supervised, sensor.clone(), clock.clone()),
            clock,
            sensor,
        )
    }

    fn sent(backend: &FakeBackend) -> Vec<Call> {
        backend.calls()
    }

    /// Roda passos a cada `PERIOD` até `secs`.
    fn run(feed: &Feed, clock: &FakeClock, backend: &mut FakeBackend, secs: u64) {
        for _ in 0..secs / 2 {
            feed.step(backend);
            clock.advance(PERIOD);
        }
    }

    #[test]
    fn cpu_feed_janela_inicial_e_tctl_constante() {
        let (feed, clock, _) = feed(true, Some(40_200));
        let mut b = FakeBackend::default();
        run(&feed, &clock, &mut b, 14);
        // 0, 2, 4, 6 s: 80 °C (o primeiro abre a sessão); 8, 10, 12 s: Tctl
        // arredondado para cima, repetido mesmo constante.
        assert_eq!(
            sent(&b),
            [
                Call::Feed(80),
                Call::Feed(80),
                Call::Feed(80),
                Call::Feed(80),
                Call::Feed(41),
                Call::Feed(41),
                Call::Feed(41),
            ]
        );
    }

    #[test]
    fn cpu_feed_sem_e0_colado_ao_anterior() {
        let (feed, clock, _) = feed(true, Some(40_000));
        let mut b = FakeBackend::default();
        // Primeiro envio e a volta extra dos ticks pelo modelo novo.
        feed.step(&mut b);
        feed.step(&mut b);
        assert_eq!(sent(&b), [Call::Feed(80)]);
        clock.advance(MIN_GAP);
        feed.step(&mut b);
        assert_eq!(sent(&b), [Call::Feed(80), Call::Feed(80)]);
        // Resume passa na hora, mesmo colado.
        feed.resume();
        feed.step(&mut b);
        assert_eq!(sent(&b)[2..], [Call::ResetCpu, Call::Feed(80)]);
    }

    #[test]
    fn cpu_feed_ativo_so_depois_da_janela() {
        let (feed, clock, _) = feed(true, Some(40_000));
        let mut b = FakeBackend::default();
        for _ in 0..4 {
            feed.step(&mut b);
            assert!(!feed.active());
            assert!(feed.curve_allowed().is_err());
            clock.advance(PERIOD);
        }
        feed.step(&mut b);
        assert!(feed.active());
        assert!(feed.curve_allowed().is_ok());
        // Sem novo envio por mais de 4 s, deixa de estar ativo.
        clock.advance(Duration::from_millis(4001));
        assert!(!feed.active());
    }

    #[test]
    fn cpu_feed_releitura_antes_do_envio() {
        let (feed, clock, sensor) = feed(true, Some(30_000));
        let mut b = FakeBackend::default();
        run(&feed, &clock, &mut b, 8);
        // O valor que vale é o lido ao executar, não o da hora do pedido.
        sensor.set(Some(66_500));
        feed.step(&mut b);
        assert_eq!(sent(&b).last(), Some(&Call::Feed(67)));
    }

    #[test]
    fn cpu_feed_erro_do_sensor_envia_alto() {
        let (feed, clock, sensor) = feed(true, Some(35_000));
        let mut b = FakeBackend::default();
        run(&feed, &clock, &mut b, 8);
        sensor.set(None);
        feed.step(&mut b);
        assert_eq!(sent(&b).last(), Some(&Call::Feed(80)));
        assert!(feed.curve_allowed().is_err());
    }

    #[test]
    fn cpu_feed_sem_supervisao_nao_envia() {
        let (feed, clock, _) = feed(false, Some(35_000));
        let mut b = FakeBackend::default();
        run(&feed, &clock, &mut b, 20);
        assert!(sent(&b).is_empty());
        assert!(!feed.active());
        assert!(feed.final_protect(&mut b).is_none());
        assert!(sent(&b).is_empty());
    }

    #[test]
    fn cpu_feed_reset_por_resume_reabre_a_janela() {
        let (feed, clock, _) = feed(true, Some(35_000));
        let mut b = FakeBackend::default();
        run(&feed, &clock, &mut b, 10);
        assert!(feed.active());
        feed.begin_sleep();
        feed.step(&mut b);
        assert!(!feed.active());
        let before = sent(&b).len();
        feed.resume();
        feed.step(&mut b);
        let calls = sent(&b);
        // Sessão nova: reset no backend e 80 °C, sem repetir o que ficou para trás.
        assert_eq!(calls[before..], [Call::ResetCpu, Call::Feed(80)]);
        assert!(!feed.active());
    }

    #[test]
    fn cpu_feed_falha_tenta_protecao() {
        let (feed, clock, _) = feed(true, Some(35_000));
        let mut b = FakeBackend::default();
        run(&feed, &clock, &mut b, 10);
        b.fail_next([Some(Error::Busy("ocupado".into()))]);
        feed.step(&mut b);
        let calls = sent(&b);
        assert_eq!(calls[calls.len() - 2..], [Call::Feed(35), Call::Protect]);
        assert!(!feed.active());
        // Sem E0 colado à proteção; no período seguinte, a sessão recomeça
        // com 80 °C.
        feed.step(&mut b);
        assert_eq!(sent(&b).last(), Some(&Call::Protect));
        clock.advance(PERIOD);
        feed.step(&mut b);
        assert_eq!(sent(&b).last(), Some(&Call::Feed(80)));
    }

    #[test]
    fn cpu_feed_watchdog_so_com_progresso_novo() {
        let (feed, clock, _) = feed(true, Some(35_000));
        let mut b = FakeBackend::default();
        assert!(!feed.progressed(true));
        feed.step(&mut b);
        assert!(feed.progressed(true));
        // O mesmo sucesso não renova duas vezes.
        assert!(!feed.progressed(true));
        clock.advance(PERIOD);
        b.fail_next([
            Some(Error::Device("x".into())),
            Some(Error::Device("y".into())),
        ]);
        feed.step(&mut b);
        assert!(!feed.progressed(true));
        // Ausência legítima: o scan basta.
        assert!(feed.progressed(false));
    }

    #[test]
    fn cpu_feed_watchdog_modelo_sem_feed() {
        let (feed, _, _) = feed(true, Some(35_000));
        let mut b = FakeBackend::default();
        b.set_disabled_model(Some(2));
        feed.step(&mut b);
        assert_eq!(sent(&b), [Call::Feed(80)]);
        assert!(feed.progressed(true));
        assert!(feed.progressed(true));
        assert!(!feed.active());
        assert!(feed.curve_allowed().is_err());
    }
}
