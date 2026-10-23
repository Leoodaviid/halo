// SPDX-License-Identifier: GPL-2.0-or-later

//! `halod`: daemon do usuário do halo. Publica no D-Bus da sessão o LED, a
//! velocidade e a telemetria do Waterforce e os perfis. Só envia comandos ao
//! hardware quando um cliente pede; os ticks periódicos só leem o sysfs.

mod bus;
mod error;
mod profiles;
#[cfg(test)]
mod testbus;
mod ticks;
mod worker;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use halo_core::waterforce::speed::{self, SnapshotStore};
use zbus::connection;

use crate::bus::BUS_NAME;
use crate::profiles::ProfileStore;
use crate::ticks::Ticks;
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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match zbus::block_on(run(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("halod: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), String> {
    let state_dir = speed::state_dir().ok_or("sem XDG_STATE_HOME nem HOME")?;
    let config_dir = profiles::config_dir().ok_or("sem XDG_CONFIG_HOME nem HOME")?;
    // O worker acorda os ticks pelo `refresh` quando lê um modelo novo.
    let (refresh, wake) = async_channel::bounded(1);
    let model = KnownModel::default();
    let worker = Worker::spawn(
        CoreBackend::new(cli.sys_root.clone(), Some(SnapshotStore::new(&state_dir))),
        model.clone(),
        refresh.clone(),
    );

    let builder = match &cli.address {
        Some(a) => connection::Builder::address(a.as_str()),
        None => connection::Builder::session(),
    };
    let conn = builder
        .and_then(|b| {
            bus::serve(
                b,
                worker,
                ProfileStore::new(&config_dir, &state_dir),
                ticks::first_read(&cli.sys_root),
            )
        })
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

    let ticks = Ticks::new(cli.sys_root, model);
    futures_util::future::join(
        ticks.run(&conn, wake),
        ticks::watch_resume(cli.system_address, refresh),
    )
    .await;
    Ok(())
}
