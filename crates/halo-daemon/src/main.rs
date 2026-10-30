// SPDX-License-Identifier: GPL-2.0-or-later

//! `halod`: daemon do usuário do halo. Publica no D-Bus da sessão o LED, a
//! velocidade e a telemetria do Waterforce e os perfis. Só envia comandos ao
//! hardware quando um cliente pede, com uma exceção: sob o watchdog da
//! unidade, alimenta o X 240 com a temperatura da CPU (`E1`/`E0`, ver
//! `feed`). `halod --waterforce-cpu-safe` é o modo de uma execução que manda
//! o `E0` de 80 °C, para o `ExecStopPost` (ver `safe`). `halod --session
//! begin|end` é a guarda da sessão gráfica (ver `session`).

mod access;
mod bus;
mod error;
mod feed;
mod led_state;
mod link;
mod profiles;
mod safe;
mod session;
#[cfg(test)]
mod testbus;
mod ticks;
mod watchdog;
mod worker;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use futures_util::future::Either;
use halo_core::waterforce::cpu_feed::{CpuInfo, Protected};
use halo_core::waterforce::speed::{self, SnapshotStore};
use zbus::connection;

use crate::bus::BUS_NAME;
use crate::feed::{Feed, STOP_BUDGET, SysfsSensor, SystemClock};
use crate::profiles::ProfileStore;
use crate::ticks::Ticks;
use crate::watchdog::Notifier;
use crate::worker::{CoreBackend, KnownModel, Worker};

#[derive(Parser)]
#[command(
    name = "halod",
    version,
    about = "Daemon do halo: D-Bus da sessão e perfis"
)]
struct Cli {
    /// Raiz do sysfs (para testes).
    #[arg(long, default_value = "/sys", hide = true)]
    sys_root: PathBuf,
    /// Endereço do D-Bus da sessão no lugar do padrão (para testes).
    #[arg(long, hide = true)]
    address: Option<String>,
    /// Endereço do D-Bus do sistema, onde fica o logind (para testes).
    #[arg(long, hide = true)]
    system_address: Option<String>,
    /// Raiz do procfs, de onde vem o cpuinfo (para testes).
    #[arg(long, default_value = "/proc", hide = true)]
    proc_root: PathBuf,
    /// Envia ao X 240 o E0 de 80 °C (com DE e E1), espera 8 s e sai, sem
    /// D-Bus nem arquivos de estado. É o ExecStopPost da unidade.
    #[arg(long, conflicts_with = "session")]
    waterforce_cpu_safe: bool,
    /// Chama Session.Begin() ou Session.End() no halod e sai. Sem resposta
    /// do End() em 10 s, faz o mesmo que --waterforce-cpu-safe e sai com
    /// falha. É a halod-session.service.
    #[arg(long, value_enum)]
    session: Option<session::Step>,
    /// Nó aberto no lugar do /dev/hidrawN achado no sysfs (para testes).
    #[arg(long, hide = true)]
    hidraw_node: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.waterforce_cpu_safe {
        return safe::run(&cli.sys_root, &cli.proc_root, cli.hidraw_node.as_deref());
    }
    if let Some(step) = cli.session {
        return session::run_client(
            cli.address.as_deref(),
            step,
            &cli.sys_root,
            &cli.proc_root,
            cli.hidraw_node.as_deref(),
        );
    }
    // Antes de qualquer thread: o SIGTERM vira um evento do fluxo normal
    // (`on_terminate`), nunca I/O dentro de um signal handler.
    let signals = block_termination();
    match zbus::block_on(run(cli, signals)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("halod: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Bloqueia SIGTERM e SIGINT nesta thread (e nas que ela criar) e devolve o
/// conjunto, para [`wait_termination`].
fn block_termination() -> libc::sigset_t {
    // SAFETY: `set` é inicializado por `sigemptyset` antes do uso, e a
    // máscara só muda nesta thread, antes de existir outra.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGTERM);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
        set
    }
}

/// Espera, numa thread própria, um sinal de `set` e chama [`on_terminate`].
fn wait_termination(set: libc::sigset_t, worker: Worker) {
    std::thread::Builder::new()
        .name("signals".into())
        .spawn(move || {
            let mut sig = 0;
            // SAFETY: `set` e `sig` são válidos durante a chamada.
            while unsafe { libc::sigwait(&set, &mut sig) } != 0 {}
            std::process::exit(on_terminate(&worker, sig));
        })
        .expect("não consegui criar a thread de sinais");
}

/// SIGTERM: para o feed e os pedidos, espera a operação em andamento e
/// manda a proteção, tudo em até [`STOP_BUDGET`]. Devolve o código de saída.
fn on_terminate(worker: &Worker, sig: i32) -> i32 {
    eprintln!("halod: sinal {sig} recebido; parando o feed da CPU");
    protect(worker)
}

/// Para o feed e os pedidos e manda a proteção, como no SIGTERM. Devolve o
/// código de saída.
fn protect(worker: &Worker) -> i32 {
    match worker.shutdown(STOP_BUDGET) {
        Some(None) => 0,
        Some(Some(Ok(Protected::Sent(m)))) => {
            eprintln!("halod: E0 de 80 °C enviado ao modelo {m} antes de sair");
            0
        }
        Some(Some(Ok(_))) => 0,
        Some(Some(Err(e))) => {
            eprintln!("halod: aparelho não protegido ao sair: {e}");
            1
        }
        None => {
            eprintln!(
                "halod: a fila do Waterforce não terminou em {}s; aparelho não protegido pelo processo",
                STOP_BUDGET.as_secs()
            );
            1
        }
    }
}

/// Supervisão do watchdog pelo ambiente. Sem ela, o feed fica desligado.
fn supervision() -> Option<Arc<dyn Notifier>> {
    match watchdog::from_env(|k| std::env::var(k).ok(), std::process::id()) {
        Ok(Some(s)) => {
            eprintln!(
                "halod: watchdog de {}s; feed da CPU ligado",
                s.timeout.as_secs()
            );
            Some(Arc::new(s.notifier))
        }
        Ok(None) => {
            eprintln!("halod: sem watchdog do systemd; feed da CPU desligado");
            None
        }
        Err(e) => {
            eprintln!("halod: ambiente do watchdog incompatível ({e}); feed da CPU desligado");
            None
        }
    }
}

async fn run(cli: Cli, signals: libc::sigset_t) -> Result<(), String> {
    let state_dir = speed::state_dir().ok_or("sem XDG_STATE_HOME nem HOME")?;
    let config_dir = profiles::config_dir().ok_or("sem XDG_CONFIG_HOME nem HOME")?;
    let notifier = supervision();
    let feed = Feed::new(
        notifier.is_some(),
        Arc::new(SysfsSensor(cli.sys_root.clone())),
        Arc::new(SystemClock),
    )
    .traced(std::env::var_os("HALOD_TRACE_FEED").is_some());
    // O worker acorda os ticks pelo `refresh` quando lê um modelo novo.
    let (refresh, wake) = async_channel::bounded(1);
    let model = KnownModel::default();
    let worker = Worker::spawn_with_feed(
        CoreBackend::with_cpu(
            cli.sys_root.clone(),
            Some(SnapshotStore::new(&state_dir)),
            CpuInfo::read(&cli.sys_root, &cli.proc_root),
        ),
        model.clone(),
        refresh.clone(),
        feed,
    );
    wait_termination(signals, worker.clone());

    let address = cli.address.clone();
    let builder = move || match &address {
        Some(a) => connection::Builder::address(a.as_str()),
        None => connection::Builder::session(),
    };
    // Antes de pegar o nome: o `Begin()` de uma guarda subindo só chega
    // depois desta decisão.
    let access = session::sysfs_probe(cli.sys_root.clone());
    let tick_access = access.clone();
    match builder() {
        Ok(b) => match b.build().await {
            Ok(probe) => {
                session::initial_state(&probe, &worker, &access).await;
            }
            Err(e) => eprintln!("halod: aviso: estado da sessão gráfica não lido: {e}"),
        },
        Err(e) => eprintln!("halod: aviso: estado da sessão gráfica não lido: {e}"),
    }
    // Os objetos de cada conexão, a primeira e as da reconexão (link.rs).
    let connect: link::Connect = {
        let (worker, sys_root, builder) = (worker.clone(), cli.sys_root.clone(), builder.clone());
        Arc::new(move || {
            builder().and_then(|b| {
                bus::serve(
                    b,
                    worker.clone(),
                    ProfileStore::new(&config_dir, &state_dir),
                    Some(led_state::LedState::new(&state_dir)),
                    ticks::first_read(&sys_root),
                    access.clone(),
                )
            })
        })
    };
    let conn = connect()
        .and_then(bus::claim_name)
        .map_err(|e| format!("não consegui preparar o D-Bus: {e}"))?
        .build()
        .await
        .map_err(|e| match e {
            zbus::Error::NameTaken => {
                format!("o halo já está rodando ({BUS_NAME} já tem dono no D-Bus da sessão)")
            }
            e => format!("não consegui conectar ao D-Bus da sessão: {e}"),
        })?;
    eprintln!("halod: {BUS_NAME} no D-Bus da sessão");
    // Type=notify: pronto só com o nome. Com Type=dbus, o systemd pararia a
    // unidade se o nome estivesse sem dono quando o barramento volta.
    if let Some(Err(e)) = watchdog::ready(|k| std::env::var(k).ok()) {
        eprintln!("halod: aviso: READY=1 não enviado: {e}");
    }
    let link = link::BusLink::new(conn);

    // Os ticks começam depois do nome: o primeiro envio do feed só sai com
    // o daemon pronto.
    let ticks =
        Ticks::with_worker(cli.sys_root, model, worker.clone(), notifier).with_access(tick_access);
    let serving = futures_util::future::join(
        ticks.run(&link, wake),
        ticks::watch_resume(cli.system_address, refresh, Some(worker.clone())),
    );
    let alive = link::keep_alive(&link, &connect, &worker);
    futures_util::pin_mut!(serving, alive);
    match futures_util::future::select(serving, alive).await {
        Either::Left(_) => Ok(()),
        Either::Right((reason, _)) => {
            eprintln!("halod: {reason}; saindo pela proteção");
            protect(&worker);
            Err("o D-Bus da sessão não voltou".into())
        }
    }
}
