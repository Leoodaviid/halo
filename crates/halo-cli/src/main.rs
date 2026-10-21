// SPDX-License-Identifier: GPL-2.0-or-later

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::builder::{PossibleValuesParser, TypedValueParser as _};
use clap::{ArgGroup, Parser, Subcommand};
use halo_core::hid::{Hidraw, OpenError};
use halo_core::it5702::{self, It5702Report, ProbeError};
use halo_core::waterforce::curve::Curve;
use halo_core::waterforce::led::{self, ApplyError, Effect, LedSetting, Rgb};
use halo_core::waterforce::probe::{
    self as wf_probe, CurvePoint, ProbeReport, ReadQuery, Reading, SpeedMode, model_name,
};
use halo_core::waterforce::speed::{
    self, CurveRequest, FanPreset, PumpPreset, SetRequest, Side, SnapshotStore, SpeedError,
    SpeedStatus, Which,
};
use halo_core::waterforce::telemetry::{
    self, Coolant, MIN_INTERVAL_SECS, Telemetry, TelemetryError,
};

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
    /// Aplica cor e efeito ao RGB de um dispositivo.
    Led {
        #[command(subcommand)]
        device: LedDevice,
    },
    /// Lê e escolhe o modo de velocidade da fan e da bomba.
    Speed {
        #[command(subcommand)]
        device: SpeedDevice,
    },
}

#[derive(Subcommand)]
enum SpeedDevice {
    /// AORUS Waterforce (1044:7a4d): presets do firmware e curva Customized.
    Waterforce {
        #[command(subcommand)]
        action: WaterforceSpeed,
    },
}

#[derive(Subcommand)]
enum WaterforceSpeed {
    /// Mostra os modos e o slot Customized. Envia só DE 00, DD, D9 01 e D9 02.
    Status,
    /// Aplica presets com E5 e um B6 no fim. Antes, grava o estado atual como
    /// snapshot em $XDG_STATE_HOME/halo/. O lado não informado mantém o modo atual.
    #[command(group(ArgGroup::new("side").required(true).multiple(true).args(["fan", "pump"])))]
    Set {
        /// Preset da fan.
        #[arg(long, value_parser = PossibleValuesParser::new(FanPreset::NAMES)
            .map(|s: String| FanPreset::from_name(&s).unwrap()))]
        fan: Option<FanPreset>,
        /// Preset da bomba.
        #[arg(long, value_parser = PossibleValuesParser::new(PumpPreset::NAMES)
            .map(|s: String| PumpPreset::from_name(&s).unwrap()))]
        pump: Option<PumpPreset>,
        /// Experimental: envia os E5 sem o B6. O efeito e a persistência dessa
        /// forma não estão confirmados.
        #[arg(long)]
        no_save: bool,
        /// Mostra os pacotes sem abrir o aparelho nem gravar snapshot. Exige
        /// --fan e --pump.
        #[arg(long)]
        dry_run: bool,
    },
    /// Grava a curva Customized plana (o mesmo RPM em 0, 30, 50 e 80 °C) da fan
    /// ou da bomba com E5, E6 e um B6 no fim. A curva é plana porque o firmware
    /// segue a temperatura da CPU, que no Linux ninguém envia. Antes, grava o
    /// estado atual como snapshot em $XDG_STATE_HOME/halo/. O outro lado mantém
    /// o modo atual.
    Curve {
        /// Lado da curva.
        #[arg(value_parser = PossibleValuesParser::new(["fan", "pump"])
            .map(|s: String| if s == "fan" { Side::Fan } else { Side::Pump }))]
        side: Side,
        /// RPM dos 4 pontos. Fan de 900 a 2500, bomba de 2600 a 2800.
        rpm: u16,
        /// Experimental: envia os E5 e o E6 sem o B6. A curva vale na hora, mas
        /// não se sabe se continua depois de desligar.
        #[arg(long)]
        no_save: bool,
        /// Valida a curva e mostra os pacotes sem abrir o aparelho nem gravar
        /// snapshot.
        #[arg(long)]
        dry_run: bool,
    },
    /// Volta ao estado de antes da última escrita do halo.
    Restore {
        /// Volta ao estado de antes da primeira escrita do halo.
        #[arg(long)]
        original: bool,
    },
}

#[derive(Subcommand)]
enum LedDevice {
    /// Anel e fans do AORUS Waterforce (1044:7a4d). Confere o modelo com DE 00 e
    /// envia só C9, CD, B0-B3 e um B6 no fim.
    Waterforce {
        /// Efeito.
        #[arg(value_parser = PossibleValuesParser::new(Effect::NAMES)
            .map(|s: String| Effect::from_name(&s).unwrap()))]
        effect: Effect,
        /// Cor RRGGBB. Repita para montar a paleta (até 8 cores).
        #[arg(long = "color", value_name = "RRGGBB", value_parser = parse_color)]
        colors: Vec<Rgb>,
        /// Brilho de 1 a 10 (padrão 10).
        #[arg(long)]
        brightness: Option<u8>,
        /// Velocidade de 1 a 5 (padrão 5).
        #[arg(long)]
        speed: Option<u8>,
        /// Mostra os pacotes sem abrir o aparelho.
        #[arg(long)]
        dry_run: bool,
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
    /// AORUS Waterforce (1044:7a4d). Envia só os pedidos de leitura D6, DE 00, DD,
    /// D9 01, D9 02, C7 e E8.
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

/// Argumento inválido, o mesmo código do clap.
const EXIT_USAGE: u8 = 2;
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
        } => {
            let mut note_shown = false;
            loop {
                match telemetry::read(sys) {
                    Ok(t) => {
                        print!("{}", format_telemetry(&t));
                        if let Some(note) = coolant_note(&t, &mut note_shown) {
                            eprintln!("{note}");
                        }
                    }
                    Err(e) => return fail(&e, telemetry_exit_code(&e)),
                }
                let Some(secs) = watch else {
                    return ExitCode::SUCCESS;
                };
                std::thread::sleep(Duration::from_secs(secs));
                println!();
            }
        }
        Command::Led {
            device:
                LedDevice::Waterforce {
                    effect,
                    colors,
                    brightness,
                    speed,
                    dry_run,
                },
        } => {
            let setting = match LedSetting::new(effect, &colors, brightness, speed) {
                Ok(s) => s,
                Err(e) => return fail(&e, EXIT_USAGE),
            };
            if dry_run {
                print!("{}", format_packets(&setting.packets()));
                return ExitCode::SUCCESS;
            }
            match led::apply(sys, &setting) {
                Ok(applied) => {
                    println!(
                        "{effect} aplicado em {} ({})",
                        applied.devnode.display(),
                        model_name(applied.model).unwrap_or("modelo desconhecido")
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => fail(&e, led_exit_code(&e)),
            }
        }
        Command::Speed {
            device: SpeedDevice::Waterforce { action },
        } => speed_waterforce(sys, action),
    }
}

fn speed_waterforce(sys: &Path, action: WaterforceSpeed) -> ExitCode {
    let store = || {
        speed::state_dir()
            .map(SnapshotStore::new)
            .ok_or(SpeedError::NoStateDir)
    };
    let result = match action {
        WaterforceSpeed::Status => match speed::status(sys) {
            Ok(s) => {
                print!("{}", format_speed_status(&s));
                return ExitCode::SUCCESS;
            }
            Err(e) => Err(e),
        },
        WaterforceSpeed::Set {
            fan,
            pump,
            no_save,
            dry_run,
        } => {
            let save = !no_save;
            if dry_run {
                let (Some(fan), Some(pump)) = (fan, pump) else {
                    return fail(&"a simulação precisa de --fan e --pump", EXIT_USAGE);
                };
                let target = speed::Target {
                    fan: fan.wire(),
                    pump: pump.wire(),
                    fan_curve: None,
                    pump_curve: None,
                };
                print!("{}", format_packets(&target.packets(save)));
                return ExitCode::SUCCESS;
            }
            if no_save {
                eprintln!(
                    "halo: aviso: --no-save é experimental; não se sabe se o modo vale sem o B6 nem se continua depois de desligar"
                );
            }
            let request = SetRequest { fan, pump, save };
            store().and_then(|store| speed::set(sys, &store, &request).map(|a| (store, a)))
        }
        WaterforceSpeed::Curve {
            side,
            rpm,
            no_save,
            dry_run,
        } => {
            let curve = match Curve::flat(side, rpm) {
                Ok(c) => c,
                Err(e) => return fail(&e, EXIT_USAGE),
            };
            let save = !no_save;
            if dry_run {
                print!("{}", format_curve_simulation(&curve, save));
                return ExitCode::SUCCESS;
            }
            if no_save {
                eprintln!(
                    "halo: aviso: --no-save é experimental; a curva vale na hora, mas não se sabe se continua depois de desligar"
                );
            }
            let request = CurveRequest { curve, save };
            store().and_then(|store| speed::curve(sys, &store, &request).map(|a| (store, a)))
        }
        WaterforceSpeed::Restore { original } => {
            let which = if original {
                Which::Original
            } else {
                Which::Previous
            };
            store().and_then(|store| speed::restore(sys, &store, which).map(|a| (store, a)))
        }
    };
    match result {
        Ok((store, applied)) => {
            print!("{}", format_speed_applied(&store, &applied));
            ExitCode::SUCCESS
        }
        Err(e) => fail(&e, speed_exit_code(&e)),
    }
}

fn speed_exit_code(err: &SpeedError) -> u8 {
    match err {
        SpeedError::NotFound | SpeedError::Open(OpenError::NotFound(_)) => EXIT_NOT_FOUND,
        SpeedError::Open(OpenError::PermissionDenied(_)) => EXIT_PERMISSION,
        SpeedError::NoSide
        | SpeedError::CurrentModeUnknown { .. }
        | SpeedError::OtherSideUnknown { .. } => EXIT_USAGE,
        _ => EXIT_FAILURE,
    }
}

/// Pacotes da curva, com o `E5` do outro lado marcado: o modo dele só é
/// conhecido lendo o `DD` na gravação.
fn format_curve_simulation(curve: &Curve, save: bool) -> String {
    let any = SpeedMode(speed::mode::BALANCED);
    let target = curve
        .target(any, any)
        .expect("Balanced é um modo que o halo repete");
    let other = match curve.side() {
        Side::Fan => Side::Pump,
        Side::Pump => Side::Fan,
    };
    let mut out = String::new();
    for p in target.packets(save) {
        if p[1] == 0xE5 && p[2] == other.wire() {
            let _ = writeln!(
                out,
                "99 E5 {:02X} ?? 00   (?? = modo atual da {}, lido no DD na gravação)",
                other.wire(),
                other.name()
            );
        } else {
            out.push_str(&format_packets(&[p]));
        }
    }
    out
}

fn format_curve(points: &[CurvePoint]) -> String {
    let points: Vec<String> = points
        .iter()
        .map(|p| format!("{} °C {} RPM", p.celsius, p.rpm))
        .collect();
    points.join(", ")
}

fn format_speed_status(s: &SpeedStatus) -> String {
    let st = &s.state;
    let mut out = String::new();
    let model = match model_name(st.model) {
        Some(name) => format!("{name} [{}]", st.model),
        None => format!("desconhecido [{}]", st.model),
    };
    let _ = writeln!(out, "Dispositivo:  {}", s.devnode.display());
    let _ = writeln!(out, "Modelo:       {model}");
    let _ = writeln!(
        out,
        "Fan:          {}",
        format_speed_mode(SpeedMode(st.fan_mode))
    );
    let _ = writeln!(
        out,
        "Bomba:        {}",
        format_speed_mode(SpeedMode(st.pump_mode))
    );
    for (side, mode, curve, query) in [
        (Side::Fan, st.fan_mode, &st.fan_curve, "D9 01"),
        (Side::Pump, st.pump_mode, &st.pump_curve, "D9 02"),
    ] {
        let mode = SpeedMode(mode);
        let active = if mode.0 == speed::mode::CUSTOMIZED {
            "ativo".to_owned()
        } else {
            format!(
                "não ativo: a {} está em {}",
                side.name(),
                mode.name().unwrap_or("modo desconhecido")
            )
        };
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "Slot Customized da {} ({query}), {active}:",
            side.name()
        );
        let _ = writeln!(out, "  {}", format_curve(curve));
        if let Some(reference) = speed::reference_curve(st.model, side, mode) {
            let _ = writeln!(
                out,
                "Curva do preset {}, referência da interface do GCC (não lida do aparelho):",
                mode.name().unwrap_or("?")
            );
            let _ = writeln!(out, "  {}", format_curve(&reference));
        }
    }
    out
}

fn format_speed_applied(store: &SnapshotStore, a: &speed::Applied) -> String {
    let w = &a.written;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Antes:  fan {}, bomba {}",
        format_speed_mode(SpeedMode(w.before.fan_mode)),
        format_speed_mode(SpeedMode(w.before.pump_mode))
    );
    let _ = writeln!(
        out,
        "Snapshot anterior: {}",
        store.path(Which::Previous).display()
    );
    let created = if w.original_created {
        " (criado agora)"
    } else {
        ""
    };
    let _ = writeln!(
        out,
        "Snapshot original: {}{created}",
        store.path(Which::Original).display()
    );
    let _ = writeln!(
        out,
        "Enviado a {} ({}):",
        a.devnode.display(),
        model_name(w.before.model).unwrap_or("modelo desconhecido")
    );
    out.push_str(&format_packets(&w.packets));
    out
}

fn parse_color(s: &str) -> Result<Rgb, String> {
    s.parse().map_err(|e: led::LedError| e.to_string())
}

/// Um pacote por linha, em hexadecimal, sem o preenchimento de zeros.
fn format_packets(packets: &[Vec<u8>]) -> String {
    let mut out = String::new();
    for p in packets {
        let hex: Vec<String> = p.iter().map(|b| format!("{b:02X}")).collect();
        let _ = writeln!(out, "{}", hex.join(" "));
    }
    out
}

fn led_exit_code(err: &ApplyError) -> u8 {
    match err {
        ApplyError::NotFound | ApplyError::Open(OpenError::NotFound(_)) => EXIT_NOT_FOUND,
        ApplyError::Open(OpenError::PermissionDenied(_)) => EXIT_PERMISSION,
        _ => EXIT_FAILURE,
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

const COOLANT_NOTE: &str = "Nota: a temperatura do líquido informada pelo aparelho é impossível \
(acima de 100 °C). O Castor3 já informou valores assim, que só voltaram ao normal depois de \
desligar o PC com corte de energia. Veja \"Bomba plana e temperatura errada no DA\" em docs/hardware.md.";

/// Nota para o stderr na primeira leitura inválida da execução.
fn coolant_note(t: &Telemetry, shown: &mut bool) -> Option<&'static str> {
    if *shown || !matches!(t.coolant(), Coolant::Implausible(_)) {
        return None;
    }
    *shown = true;
    Some(COOLANT_NOTE)
}

fn format_telemetry(t: &Telemetry) -> String {
    let mut out = String::new();
    match t.coolant() {
        Coolant::Valid(_) => {
            let temp = format!("{:.1}", t.coolant_celsius()).replace('.', ",");
            let _ = writeln!(out, "Líquido:  {temp} °C");
        }
        Coolant::Implausible(m) => {
            let _ = writeln!(
                out,
                "Líquido:  inválido (o aparelho informou {} °C)",
                m / 1000
            );
        }
    }
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
            ReadQuery::Firmware => "Firmware (D6):      ",
            ReadQuery::Model => "Modelo (DE):        ",
            ReadQuery::SpeedModes => "Modos (DD):         ",
            ReadQuery::FanCurve => "Curva da fan (D9):  ",
            ReadQuery::PumpCurve => "Curva da bomba (D9):",
            ReadQuery::FanQty => "Fans RGB (C7):      ",
            ReadQuery::LcdMode => "LCD (E8):           ",
        };
        let value = match (&result.response, result.reading()) {
            (None, _) => "sem resposta em 2 s".to_owned(),
            (Some(_), Some(reading)) => format_reading(reading),
            (Some(_), None) => "resposta curta".to_owned(),
        };
        let _ = writeln!(out, "{label} {value}");
    }
    if raw {
        for result in &r.results {
            let Some(response) = &result.response else {
                continue;
            };
            let payload: Vec<String> = result
                .query
                .payload()
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect();
            let _ = writeln!(out, "Resposta {}:", payload.join(" "));
            write_hex(&mut out, &response[..response.len().min(64)]);
        }
    }
    out
}

fn format_reading(reading: Reading) -> String {
    match reading {
        Reading::Firmware { major, minor } => format!("{major}.{minor}"),
        Reading::Model(index) => match model_name(index) {
            Some(name) => format!("{name} [{index}]"),
            None => format!("desconhecido [{index}]"),
        },
        Reading::SpeedModes { fan, pump } => format!(
            "fan {}, bomba {}",
            format_speed_mode(fan),
            format_speed_mode(pump)
        ),
        Reading::Curve(points) => format_curve(&points),
        Reading::FanQty(n) => n.to_string(),
        Reading::LcdMode(mode) => format!("modo {mode}"),
    }
}

fn format_speed_mode(mode: SpeedMode) -> String {
    format!("{} [{}]", mode.name().unwrap_or("desconhecido"), mode.0)
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

    fn telemetry_with_coolant(millicelsius: i32) -> Telemetry {
        Telemetry {
            fan_rpm: 2204,
            pump_rpm: 3030,
            fan_duty: 224,
            pump_duty: 255,
            coolant_millicelsius: millicelsius,
        }
    }

    #[test]
    fn telemetria_com_liquido_invalido() {
        let out = format_telemetry(&telemetry_with_coolant(240_000));
        assert!(out.contains("Líquido:  inválido (o aparelho informou 240 °C)"));
        assert!(out.contains("Fan:      2204 RPM, duty 224 (88%)"));
        assert!(out.contains("Bomba:    3030 RPM, duty 255 (100%)"));
        assert!(!out.contains("corte de energia"));
    }

    #[test]
    fn nota_do_liquido_sai_uma_vez() {
        let bad = telemetry_with_coolant(224_000);
        let mut shown = false;
        assert!(coolant_note(&telemetry_with_coolant(29_000), &mut shown).is_none());
        assert!(
            coolant_note(&bad, &mut shown)
                .unwrap()
                .contains("corte de energia")
        );
        assert!(coolant_note(&bad, &mut shown).is_none());
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
        let mut d9 = vec![0x99, 0xD9, 0x01, 0x00];
        d9.extend([
            0x00, 0x04, 0xB0, 0x1E, 0x04, 0xB0, 0x32, 0x05, 0x78, 0x41, 0x07, 0x08,
        ]);
        let responses: [Option<Vec<u8>>; 7] = [
            Some(vec![0x99, 0xD6, 0x01, 0x06]),
            Some(vec![0x99, 0xDE, 0x00]),
            Some(vec![0x99, 0xDD, 0x02, 0x03]),
            Some(d9),
            None,
            Some(vec![0x99, 0xC7, 0x02]),
            Some(vec![0x99, 0xE8, 0x08]),
        ];
        let report = ProbeReport {
            devnode: PathBuf::from("/dev/hidraw2"),
            hid_name: Some("ITE Tech. Castor3".into()),
            results: ReadQuery::ALL
                .into_iter()
                .zip(responses)
                .map(|(query, response)| QueryResult { query, response })
                .collect(),
        };
        let out = format_waterforce_probe(&report, true);
        assert!(out.contains("Nome HID:           ITE Tech. Castor3"));
        assert!(out.contains("Firmware (D6):       1.6\n"), "{out}");
        assert!(out.contains("Modelo (DE):         Waterforce X 240 [0]\n"));
        assert!(out.contains("Modos (DD):          fan Default [2], bomba Balanced [3]\n"));
        assert!(out.contains(
            "Curva da fan (D9):   0 °C 1200 RPM, 30 °C 1200 RPM, 50 °C 1400 RPM, 65 °C 1800 RPM\n"
        ));
        assert!(out.contains("Curva da bomba (D9): sem resposta em 2 s\n"));
        assert!(out.contains("Fans RGB (C7):       2\n"));
        assert!(out.contains("LCD (E8):            modo 7\n"));
        assert!(out.contains("Resposta D6:\n  00: 99 d6 01 06"));
        assert!(out.contains("Resposta D9 01:\n  00: 99 d9 01 00 00 04 b0"));
        assert!(!out.contains("Resposta D9 02"));
        assert_eq!(out.matches("Resposta ").count(), 6);
    }

    #[test]
    fn modelo_desconhecido() {
        assert_eq!(format_reading(Reading::Model(5)), "desconhecido [5]");
    }

    #[test]
    fn pacotes_em_hexadecimal() {
        let setting =
            LedSetting::new(Effect::Static, &[Rgb(0xFF, 0x66, 0x00)], None, None).unwrap();
        assert_eq!(
            format_packets(&setting.packets()),
            "99 C9 01 64 0A 02 01\n99 CD FF 66 00\n99 B6\n"
        );
    }

    #[test]
    fn codigos_de_saida_do_led() {
        assert_eq!(led_exit_code(&ApplyError::NotFound), EXIT_NOT_FOUND);
        let denied = ApplyError::Open(OpenError::PermissionDenied(PathBuf::from("/dev/hidraw2")));
        assert_eq!(led_exit_code(&denied), EXIT_PERMISSION);
        assert_eq!(led_exit_code(&ApplyError::UnknownModel(5)), EXIT_FAILURE);
        assert_eq!(led_exit_code(&ApplyError::NoModelResponse), EXIT_FAILURE);
    }

    fn speed_status(fan: u8, pump: u8) -> SpeedStatus {
        let points = |p: [(u8, u16); 4]| p.map(|(celsius, rpm)| CurvePoint { celsius, rpm });
        SpeedStatus {
            devnode: PathBuf::from("/dev/hidraw2"),
            state: speed::Snapshot {
                taken_unix: 0,
                model: 0,
                fan_mode: fan,
                pump_mode: pump,
                fan_curve: points([(0, 1000), (30, 1000), (50, 1500), (80, 2500)]),
                pump_curve: points([(0, 2800), (30, 2800), (50, 2800), (80, 2800)]),
            },
        }
    }

    #[test]
    fn status_com_presets() {
        let out = format_speed_status(&speed_status(5, 4));
        assert!(
            out.contains("Modelo:       Waterforce X 240 [0]\n"),
            "{out}"
        );
        assert!(out.contains("Fan:          Performance [5]\n"), "{out}");
        assert!(out.contains("Bomba:        Turbo [4]\n"), "{out}");
        assert!(out.contains(
            "Slot Customized da fan (D9 01), não ativo: a fan está em Performance:\n  0 °C 1000 RPM, 30 °C 1000 RPM, 50 °C 1500 RPM, 80 °C 2500 RPM\n"
        ), "{out}");
        assert!(out.contains(
            "Curva do preset Performance, referência da interface do GCC (não lida do aparelho):\n  0 °C 2000 RPM, 30 °C 2000 RPM, 50 °C 2200 RPM, 65 °C 2500 RPM\n"
        ), "{out}");
        assert!(
            out.contains("Slot Customized da bomba (D9 02), não ativo: a bomba está em Turbo:")
        );
        assert!(out.contains("  0 °C 3150 RPM, 30 °C 3150 RPM, 50 °C 3150 RPM, 65 °C 3150 RPM\n"));
        assert!(!out.contains(", ativo"));
    }

    #[test]
    fn status_com_a_fan_em_customized() {
        let out = format_speed_status(&speed_status(1, 4));
        assert!(out.contains("Fan:          Customized [1]\n"), "{out}");
        assert!(out.contains(
            "Slot Customized da fan (D9 01), ativo:\n  0 °C 1000 RPM, 30 °C 1000 RPM, 50 °C 1500 RPM, 80 °C 2500 RPM\n"
        ), "{out}");
        // Sem tabela do GCC para a fan; só a da bomba.
        assert_eq!(
            out.matches("referência da interface do GCC").count(),
            1,
            "{out}"
        );
    }

    #[test]
    fn codigos_de_saida_da_velocidade() {
        assert_eq!(speed_exit_code(&SpeedError::NotFound), EXIT_NOT_FOUND);
        let denied = SpeedError::Open(OpenError::PermissionDenied(PathBuf::from("/dev/hidraw2")));
        assert_eq!(speed_exit_code(&denied), EXIT_PERMISSION);
        assert_eq!(speed_exit_code(&SpeedError::NoSide), EXIT_USAGE);
        let unknown = SpeedError::CurrentModeUnknown {
            side: Side::Pump,
            raw: 3,
        };
        assert_eq!(speed_exit_code(&unknown), EXIT_USAGE);
        let other = SpeedError::OtherSideUnknown {
            side: Side::Pump,
            raw: 3,
        };
        assert_eq!(speed_exit_code(&other), EXIT_USAGE);
        assert_eq!(speed_exit_code(&SpeedError::UnknownModel(5)), EXIT_FAILURE);
        let none = SpeedError::NoSnapshot(PathBuf::from("/x"));
        assert_eq!(speed_exit_code(&none), EXIT_FAILURE);
    }

    #[test]
    fn simulacao_da_curva() {
        let fan = Curve::flat(Side::Fan, 1500).unwrap();
        assert_eq!(
            format_curve_simulation(&fan, true),
            "99 E5 01 01 00\n\
             99 E5 02 ?? 00   (?? = modo atual da bomba, lido no DD na gravação)\n\
             99 E6 00 01 00 05 DC 1E 05 DC 32 05 DC 50 05 DC\n\
             99 B6\n"
        );
        let pump = Curve::flat(Side::Pump, 2600).unwrap();
        let out = format_curve_simulation(&pump, false);
        assert!(
            out.starts_with(
                "99 E5 01 ?? 00   (?? = modo atual da fan, lido no DD na gravação)\n99 E5 02 01 00\n99 E6 00 02 "
            ),
            "{out}"
        );
        assert!(!out.contains("B6"), "{out}");
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
