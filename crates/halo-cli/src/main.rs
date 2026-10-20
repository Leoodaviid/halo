// SPDX-License-Identifier: GPL-2.0-or-later

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use halo_core::hid::{Hidraw, OpenError};
use halo_core::it5702::{self, It5702Report, ProbeError};
use halo_core::waterforce::probe::{self as wf_probe, FirmwareQuery, ProbeReport};
use halo_core::waterforce::telemetry::{self, MIN_INTERVAL_SECS, Telemetry, TelemetryError};

/// Controle de RGB do halo.
#[derive(Parser)]
#[command(name = "halo", version)]
struct Cli {
    /// Raiz do sysfs. Só para testes.
    #[arg(long, global = true, hide = true, default_value = "/sys")]
    sys_root: PathBuf,
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
    /// Mostra sensores de um dispositivo.
    Telemetry {
        #[command(subcommand)]
        device: TelemetryDevice,
    },
}

#[derive(Subcommand)]
enum ProbeDevice {
    /// Placa-mãe ITE IT5702 (048d:5702). Envia só o pedido de leitura CC 60.
    It5702 {
        /// Mostra também os 64 bytes brutos da resposta.
        #[arg(long)]
        raw: bool,
    },
    /// AORUS Waterforce (1044:7a4d). Envia só os pedidos de firmware D6 e DE 00.
    Waterforce {
        /// Mostra também os primeiros 64 bytes de cada resposta.
        #[arg(long)]
        raw: bool,
    },
}

#[derive(Subcommand)]
enum TelemetryDevice {
    /// AORUS Waterforce, pelo hwmon do kernel. Não envia nada ao aparelho.
    Waterforce {
        /// Repete a leitura a cada N segundos (mínimo 2) até Ctrl+C.
        #[arg(long, value_name = "SEGUNDOS", value_parser = parse_interval)]
        watch: Option<u64>,
    },
}

const EXIT_NOT_FOUND: u8 = 3;
const EXIT_PERMISSION: u8 = 4;
const EXIT_FAILURE: u8 = 5;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let sys = cli.sys_root.as_path();
    match cli.command {
        Command::Probe {
            device: ProbeDevice::It5702 { raw },
        } => match probe_it5702(sys) {
            Ok((devnode, report)) => {
                print!("{}", format_report(&devnode, &report, raw));
                ExitCode::SUCCESS
            }
            Err(e) => fail(&e, exit_code(&e)),
        },
        Command::Probe {
            device: ProbeDevice::Waterforce { raw },
        } => match wf_probe::probe(sys) {
            Ok(report) => {
                print!("{}", format_waterforce_probe(&report, raw));
                if report.complete() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(EXIT_FAILURE)
                }
            }
            Err(e) => fail(&e, waterforce_probe_exit_code(&e)),
        },
        Command::Telemetry {
            device: TelemetryDevice::Waterforce { watch },
        } => loop {
            match telemetry::read(sys) {
                Ok(t) => print!("{}", format_telemetry(&t)),
                Err(e) => return fail(&e, telemetry_exit_code(&e)),
            }
            let Some(secs) = watch else {
                return ExitCode::SUCCESS;
            };
            std::thread::sleep(Duration::from_secs(secs));
            println!();
        },
    }
}

fn fail(err: &dyn std::fmt::Display, code: u8) -> ExitCode {
    eprintln!("halo: {err}");
    ExitCode::from(code)
}

fn parse_interval(s: &str) -> Result<u64, String> {
    let secs: u64 = s
        .parse()
        .map_err(|_| format!("`{s}` não é um número de segundos"))?;
    if secs < MIN_INTERVAL_SECS {
        return Err(format!(
            "o intervalo mínimo é {MIN_INTERVAL_SECS} segundos (o driver só atualiza o status a cada 2 s)"
        ));
    }
    Ok(secs)
}

fn probe_it5702(sys_root: &Path) -> Result<(PathBuf, It5702Report), ProbeError> {
    let info = it5702::find(&sys_root.join("class/hidraw"))?;
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

fn waterforce_probe_exit_code(err: &wf_probe::ProbeError) -> u8 {
    match err {
        wf_probe::ProbeError::NotFound | wf_probe::ProbeError::Open(OpenError::NotFound(_)) => {
            EXIT_NOT_FOUND
        }
        wf_probe::ProbeError::Open(OpenError::PermissionDenied(_)) => EXIT_PERMISSION,
        _ => EXIT_FAILURE,
    }
}

fn telemetry_exit_code(err: &TelemetryError) -> u8 {
    match err {
        TelemetryError::NotFound | TelemetryError::DriverNotLoaded => EXIT_NOT_FOUND,
        _ => EXIT_FAILURE,
    }
}

fn format_telemetry(t: &Telemetry) -> String {
    let mut out = String::new();
    let temp = format!("{:.1}", t.coolant_celsius()).replace('.', ",");
    let _ = writeln!(out, "Líquido:  {temp} °C");
    let _ = writeln!(
        out,
        "Fan:      {} RPM, duty {} ({}%)",
        t.fan_rpm,
        t.fan_duty,
        Telemetry::duty_percent(t.fan_duty)
    );
    let _ = writeln!(
        out,
        "Bomba:    {} RPM, duty {} ({}%)",
        t.pump_rpm,
        t.pump_duty,
        Telemetry::duty_percent(t.pump_duty)
    );
    out
}

fn format_waterforce_probe(r: &ProbeReport, raw: bool) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Dispositivo:        {}", r.devnode.display());
    let _ = writeln!(
        out,
        "Nome HID:           {}",
        r.hid_name.as_deref().unwrap_or("(sem nome)")
    );
    for result in &r.results {
        let label = match result.query {
            FirmwareQuery::Kernel => "Firmware (kernel, D6):  ",
            FirmwareQuery::OpenRgb => "Firmware (OpenRGB, DE): ",
        };
        let value = match (&result.response, result.firmware()) {
            (None, _) => "sem resposta em 2 s".to_owned(),
            (Some(_), Some(v)) => v.to_string(),
            (Some(_), None) => "resposta curta".to_owned(),
        };
        let _ = writeln!(out, "{label}{value}");
    }
    if raw {
        for result in &r.results {
            let Some(response) = &result.response else {
                continue;
            };
            let _ = writeln!(out, "Resposta {:02X}:", result.query.command());
            write_hex(&mut out, &response[..response.len().min(64)]);
        }
    }
    out
}

fn write_hex(out: &mut String, bytes: &[u8]) {
    for (i, chunk) in bytes.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let _ = writeln!(out, "  {:02x}: {}", i * 16, hex.join(" "));
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
        write_hex(&mut out, &r.raw);
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
    fn telemetria_formatada() {
        let t = Telemetry {
            fan_rpm: 2204,
            pump_rpm: 3030,
            fan_duty: 224,
            pump_duty: 255,
            coolant_millicelsius: 29000,
        };
        let out = format_telemetry(&t);
        assert!(out.contains("Líquido:  29,0 °C"));
        assert!(out.contains("Fan:      2204 RPM, duty 224 (88%)"));
        assert!(out.contains("Bomba:    3030 RPM, duty 255 (100%)"));
    }

    #[test]
    fn intervalo_minimo() {
        assert_eq!(parse_interval("2"), Ok(2));
        assert!(
            parse_interval("1")
                .unwrap_err()
                .contains("mínimo é 2 segundos")
        );
        assert!(parse_interval("x").is_err());
    }

    #[test]
    fn probe_do_waterforce_formatado() {
        use halo_core::waterforce::probe::QueryResult;
        let report = ProbeReport {
            devnode: PathBuf::from("/dev/hidraw2"),
            hid_name: Some("ITE Tech. Castor3".into()),
            results: vec![
                QueryResult {
                    query: FirmwareQuery::Kernel,
                    response: Some(vec![0x99, 0xD6, 0x02, 0x05]),
                },
                QueryResult {
                    query: FirmwareQuery::OpenRgb,
                    response: None,
                },
            ],
        };
        let out = format_waterforce_probe(&report, true);
        assert!(out.contains("Nome HID:           ITE Tech. Castor3"));
        assert!(out.contains("Firmware (kernel, D6):  25"));
        assert!(out.contains("Firmware (OpenRGB, DE): sem resposta em 2 s"));
        assert!(out.contains("Resposta D6:\n  00: 99 d6 02 05"));
        assert!(!out.contains("Resposta DE"));
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
