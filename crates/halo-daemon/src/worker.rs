// SPDX-License-Identifier: GPL-2.0-or-later

//! Fila de um aparelho: uma thread dona das chamadas bloqueantes ao
//! `halo-core`, que atende um pedido de cada vez, na ordem de chegada. Os
//! métodos D-Bus esperam a resposta sem bloquear o executor do zbus.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use async_channel::Sender;
use halo_core::waterforce::led::{self, LedSetting};
use halo_core::waterforce::probe::model_name;
use halo_core::waterforce::speed::{
    self, CurveRequest, DesiredState, SetRequest, SnapshotStore, SpeedError, SpeedStatus,
    StateOutcome, Which,
};

use crate::error::Error;

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
}

/// Backend sobre o `halo-core`, com o mesmo `SnapshotStore` da CLI.
pub struct CoreBackend {
    sys_root: PathBuf,
    store: Option<SnapshotStore>,
}

impl CoreBackend {
    /// Sem `store` (nem `XDG_STATE_HOME` nem `HOME`), as escritas de
    /// velocidade são recusadas antes de abrir o aparelho.
    pub fn new(sys_root: PathBuf, store: Option<SnapshotStore>) -> Self {
        Self { sys_root, store }
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
}

type Job = Box<dyn FnOnce(&mut dyn WaterforceBackend) + Send>;

/// Fila do Waterforce. Clonar compartilha a mesma thread.
#[derive(Clone)]
pub struct Worker {
    jobs: mpsc::Sender<Job>,
}

impl Worker {
    /// O modelo lido pelas operações vai para `known`, com um aviso em
    /// `refresh` quando muda.
    pub fn spawn(
        backend: impl WaterforceBackend + 'static,
        known: KnownModel,
        refresh: Sender<()>,
    ) -> Self {
        let mut backend = Observed {
            inner: backend,
            known,
            refresh,
        };
        let (jobs, queue) = mpsc::channel::<Job>();
        thread::Builder::new()
            .name("waterforce".into())
            .spawn(move || {
                for job in queue {
                    job(&mut backend);
                }
            })
            .expect("não consegui criar a thread do Waterforce");
        Self { jobs }
    }

    /// Põe `f` na fila e espera o resultado.
    pub async fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut dyn WaterforceBackend) -> T + Send + 'static,
    ) -> T {
        let (reply, result) = async_channel::bounded(1);
        let job: Job = Box::new(move |backend| {
            let _ = reply.send_blocking(f(backend));
        });
        self.jobs
            .send(job)
            .expect("a thread do Waterforce não termina");
        result
            .recv()
            .await
            .expect("a thread do Waterforce sempre responde")
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

        pub fn set_model(&self, model: Model) {
            self.script.0.lock().unwrap().model = model;
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

    #[test]
    fn backend_real_sem_diretorio_de_estado() {
        let sys = tempfile::tempdir().unwrap();
        let mut backend = CoreBackend::new(sys.path().to_owned(), None);
        let err = backend.restore(Which::Previous).unwrap_err();
        assert!(matches!(err, Error::Storage(_)), "{err:?}");
    }
}
