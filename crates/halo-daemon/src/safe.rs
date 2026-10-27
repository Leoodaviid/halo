// SPDX-License-Identifier: GPL-2.0-or-later

//! `halod --waterforce-cpu-safe`: a proteção independente do processo
//! principal, rodada pelo `ExecStopPost` da unidade depois de qualquer fim
//! dele (SIGTERM, SIGKILL, watchdog, panic, falha no início).
//!
//! Não usa D-Bus, perfis, snapshots, Tctl nem diretório de estado. Acha o
//! Castor3 pelo sysfs, toma o `flock` (até 5 s), confere o modelo e manda
//! `DE`/`E1`/`E0` de 80 °C, a mesma sequência do SIGTERM
//! ([`cpu_feed::protect_to`]). Depois fecha o nó e espera 8 s, para o `DA`
//! mostrar a proteção antes do restart. Tudo em até [`SAFE_BUDGET`].

use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use halo_core::hid::{Hidraw, OpenError};
use halo_core::waterforce::cpu_feed::{self, CpuInfo, FeedError, Protected};
use halo_core::waterforce::find_hidraw;

/// Prazo total da execução, abaixo do `TimeoutStopSec=20s` da unidade.
pub const SAFE_BUDGET: Duration = Duration::from_secs(18);
/// Prazo do envio: `flock` (5 s), `DE` (2 s) e os writes.
pub const PROTECT_BUDGET: Duration = Duration::from_secs(9);
/// Espera depois do envio, para o efeito aparecer no `DA` antes do restart.
pub const OBSERVE: Duration = Duration::from_secs(8);

/// Como [`cpu_feed::protect`], abrindo `node` no lugar do devnode achado.
fn protect_node(
    sys_root: &Path,
    node: &Path,
    info: &CpuInfo,
    deadline: Instant,
) -> Result<Protected, FeedError> {
    if find_hidraw(sys_root).map_err(FeedError::Io)?.is_none() {
        return Ok(Protected::Absent);
    }
    let wait = deadline
        .saturating_duration_since(Instant::now())
        .min(halo_core::hid::hidraw::LOCK_TIMEOUT);
    let device = match Hidraw::open_with_timeout(node, wait) {
        Ok(d) => d,
        Err(OpenError::NotFound(_)) => return Ok(Protected::Absent),
        Err(e) => return Err(FeedError::Open(e)),
    };
    cpu_feed::protect_to(&device, info, deadline)
}

/// Roda a proteção e devolve o código de saída: sucesso com o `E0` enviado,
/// com o aparelho ausente ou com modelo 1/2 (que nunca recebem feed); falha
/// em qualquer outro caso, sem mascarar.
pub fn run(sys_root: &Path, proc_root: &Path, node: Option<&Path>) -> ExitCode {
    // Limite rígido: mesmo um write preso não passa do prazo.
    std::thread::spawn(|| {
        std::thread::sleep(SAFE_BUDGET);
        eprintln!(
            "halod: proteção da CPU não terminou em {}s; aparelho não protegido",
            SAFE_BUDGET.as_secs()
        );
        std::process::exit(2);
    });
    let info = CpuInfo::read(sys_root, proc_root);
    let deadline = Instant::now() + PROTECT_BUDGET;
    let result = match node {
        Some(node) => protect_node(sys_root, node, &info, deadline),
        None => cpu_feed::protect(sys_root, &info, deadline),
    };
    match result {
        Ok(Protected::Sent(model)) => {
            eprintln!(
                "halod: E0 de 80 °C enviado ao modelo {model}; esperando {}s",
                OBSERVE.as_secs()
            );
            std::thread::sleep(OBSERVE);
            ExitCode::SUCCESS
        }
        Ok(Protected::Absent) => {
            eprintln!("halod: Waterforce ausente; nada a proteger");
            ExitCode::SUCCESS
        }
        Ok(Protected::NotFed(model)) => {
            eprintln!("halod: modelo {model} sem feed de CPU; nada a proteger");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("halod: aparelho NÃO protegido: {e}");
            ExitCode::FAILURE
        }
    }
}
