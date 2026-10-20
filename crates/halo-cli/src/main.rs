// SPDX-License-Identifier: GPL-2.0-or-later

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use halo_core::hid::{Hidraw, OpenError, SYSFS_HIDRAW};
use halo_core::it5702::{self, It5702Report, ProbeError};

/// Controle de RGB do halo.
#[derive(Parser)]
#[command(name = "halo", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lê um controlador sem alterar o estado dele.
    Probe {
        #[command(subcommand)]
        device: ProbeDevice,
    },
}

#[derive(Subcommand)]
enum ProbeDevice {
    /// Placa-mãe ITE IT5702 (048d:5702). Envia só o pedido de leitura CC 60.
    It5702 {
        /// Mostra também os 64 bytes brutos da resposta.
        #[arg(long)]
        raw: bool,
        /// Raiz do sysfs dos hidraw. Só para testes.
        #[arg(long, hide = true, default_value = SYSFS_HIDRAW)]
        sysfs_root: PathBuf,
    },
}

const EXIT_NOT_FOUND: u8 = 3;
const EXIT_PERMISSION: u8 = 4;
const EXIT_FAILURE: u8 = 5;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Probe {
            device: ProbeDevice::It5702 { raw, sysfs_root },
        } => match probe_it5702(&sysfs_root) {
            Ok((devnode, report)) => {
                print!("{}", format_report(&devnode, &report, raw));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("halo: {e}");
                ExitCode::from(exit_code(&e))
            }
        },
    }
}

fn probe_it5702(sysfs_root: &std::path::Path) -> Result<(PathBuf, It5702Report), ProbeError> {
    let info = it5702::find(sysfs_root)?;
    let device = Hidraw::open(&info.devnode).map_err(ProbeError::Open)?;
    let report = it5702::read_config(&device)?;
    Ok((info.devnode, report))
}

fn exit_code(err: &ProbeError) -> u8 {
    match err {
        ProbeError::NotFound | ProbeError::Open(OpenError::NotFound(_)) => EXIT_NOT_FOUND,
        ProbeError::Open(OpenError::PermissionDenied(_)) => EXIT_PERMISSION,
        _ => EXIT_FAILURE,
    }
}

fn format_report(devnode: &std::path::Path, r: &It5702Report, raw: bool) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Dispositivo:        {}", devnode.display());
    let _ = writeln!(out, "Produto:            {}", r.product_name);
    let _ = writeln!(out, "Código do produto:  0x{:02X}", r.product);
    let _ = writeln!(out, "Firmware:           {}", r.firmware_version());
    let _ = writeln!(out, "Número do disp.:    {}", r.device_num);
    let _ = writeln!(out, "Detecção de fitas:  0x{:02X}", r.strip_detect);
    let _ = writeln!(out, "Comandos suportados: 0x{:02X}", r.support_cmd_flag);
    let _ = writeln!(out, "Chip id:            0x{:08X}", r.chip_id);
    let _ = writeln!(out, "Fitas ARGB:");
    for (i, c) in r.led_counts().iter().enumerate() {
        let note = if c.known { "" } else { " (valor desconhecido)" };
        let _ = writeln!(
            out,
            "  fita {i}: até {} LEDs [0x{:X}]{note}",
            c.limit, c.raw
        );
    }
    let _ = writeln!(out, "Calibração:");
    let _ = writeln!(out, "  cal_strip0: 0x{:08X}", r.cal_strip0);
    let _ = writeln!(out, "  cal_strip1: 0x{:08X}", r.cal_strip1);
    let _ = writeln!(out, "  rgb_cali:   0x{:08X}", r.rgb_cali);
    let _ = writeln!(out, "  cal_spare0: 0x{:08X}", r.cal_spare0);
    let _ = writeln!(out, "  cal_spare1: 0x{:08X}", r.cal_spare1);
    if raw {
        let _ = writeln!(out, "Bytes brutos:");
        for (i, chunk) in r.raw.chunks(16).enumerate() {
            let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
            let _ = writeln!(out, "  {:02x}: {}", i * 16, hex.join(" "));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn report() -> It5702Report {
        let mut r = [0u8; 64];
        r[0] = 0xCC;
        r[4..8].copy_from_slice(&[1, 2, 3, 4]);
        r[8] = 0x10;
        r[12..18].copy_from_slice(b"IT5702");
        it5702::decode(&r).unwrap()
    }

    #[test]
    fn saida_rotulada() {
        let out = format_report(Path::new("/dev/hidraw5"), &report(), false);
        assert!(out.contains("Dispositivo:        /dev/hidraw5"));
        assert!(out.contains("Produto:            IT5702"));
        assert!(out.contains("Firmware:           1.2.3.4"));
        assert!(out.contains("fita 0: até 32 LEDs"));
        assert!(out.contains("fita 1: até 64 LEDs"));
        assert!(!out.contains("Bytes brutos"));
    }

    #[test]
    fn saida_bruta() {
        let out = format_report(Path::new("/dev/hidraw5"), &report(), true);
        assert!(out.contains("Bytes brutos:"));
        assert!(out.contains("  00: cc 00 00 00 01 02 03 04 10 00 00 00 49 54 35 37"));
        assert!(out.contains("  30: "));
    }

    #[test]
    fn codigos_de_saida() {
        assert_eq!(exit_code(&ProbeError::NotFound), EXIT_NOT_FOUND);
        let denied = ProbeError::Open(OpenError::PermissionDenied(PathBuf::from("/dev/hidraw5")));
        assert_eq!(exit_code(&denied), EXIT_PERMISSION);
        let short = ProbeError::Decode(it5702::DecodeError::ShortResponse(10));
        assert_eq!(exit_code(&short), EXIT_FAILURE);
    }
}
