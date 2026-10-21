// SPDX-License-Identifier: GPL-2.0-or-later

//! Curva Customized plana da fan ou da bomba: o mesmo RPM nos 4 pontos.
//!
//! A temperatura de referência da curva não é a do líquido. O firmware segue
//! a temperatura da CPU, que só o `AorusLcdService` do Windows envia (`E0`), e
//! sem ela uma curva com degraus fica presa num ponto que o usuário não
//! escolheu (`docs/hardware.md`). A curva plana não depende da referência.
//!
//! Os pontos ficam em 0, 30, 50 e 80 °C, as temperaturas do slot de fábrica,
//! o que cumpre as regras da interface do GCC (`GFanCurve.Point_PointerMoved`).
//! A fan fica entre 900 e 2500 RPM (`DeviceApi.GetFanSpeedLimit`). O GCC aplica
//! essa mesma faixa à bomba; o halo usa para ela de 2600 a 2800 RPM, do
//! Balanced da bomba ao maior valor que o GCC grava com o `E6`
//! (`docs/protocols/castor3.md`).
//!
//! Os bytes saem do [`Target::packets`] da `speed`: `E5` fan, `E5` bomba, o
//! `E6` do lado da curva e, ao salvar, um `B6`.

use std::fmt;

use super::probe::{CurvePoint, SpeedMode};
use super::speed::{Side, SpeedError, Target, mode, restorable_mode};

/// Temperaturas dos pontos, as do slot de fábrica (`D9 01`).
pub const CELSIUS: [u8; 4] = [0, 30, 50, 80];

/// `DeviceApi.GetFanSpeedLimit` para o PID `0x7A4D`.
pub const FAN_MIN_RPM: u16 = 900;
pub const FAN_MAX_RPM: u16 = 2500;
/// Regra do halo: o Balanced da bomba (`PumpApi.dcX240_280_360`).
pub const PUMP_MIN_RPM: u16 = 2600;
/// Regra do halo: o valor inicial da Customized da bomba no GCC.
pub const PUMP_MAX_RPM: u16 = 2800;

/// Faixa de RPM da gravação de curva.
pub fn rpm_range(side: Side) -> (u16, u16) {
    match side {
        Side::Fan => (FAN_MIN_RPM, FAN_MAX_RPM),
        Side::Pump => (PUMP_MIN_RPM, PUMP_MAX_RPM),
    }
}

/// RPM fora da faixa do lado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurveError {
    pub side: Side,
    pub rpm: u16,
}

impl fmt::Display for CurveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (min, max) = rpm_range(self.side);
        write!(
            f,
            "{} RPM fora da faixa da {} de {min} a {max} RPM",
            self.rpm,
            self.side.name()
        )?;
        match self.side {
            Side::Pump if self.rpm < min => f.write_str("; o mínimo é o do modo Balanced da bomba"),
            Side::Pump => {
                f.write_str("; para a rotação máxima, use o modo pronto Turbo (set --pump turbo)")
            }
            Side::Fan => Ok(()),
        }
    }
}

impl std::error::Error for CurveError {}

/// Curva plana validada de um lado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Curve {
    side: Side,
    points: [CurvePoint; 4],
}

impl Curve {
    /// Os 4 pontos de [`CELSIUS`] com `rpm`, que precisa estar na faixa do lado.
    pub fn flat(side: Side, rpm: u16) -> Result<Self, CurveError> {
        let (min, max) = rpm_range(side);
        if !(min..=max).contains(&rpm) {
            return Err(CurveError { side, rpm });
        }
        Ok(Self {
            side,
            points: CELSIUS.map(|celsius| CurvePoint { celsius, rpm }),
        })
    }

    pub fn side(&self) -> Side {
        self.side
    }

    pub fn points(&self) -> &[CurvePoint; 4] {
        &self.points
    }

    /// Alvo da gravação: o lado da curva em Customized com o `E6`, e o outro
    /// repetindo o valor bruto lido no `DD`.
    pub fn target(
        &self,
        current_fan: SpeedMode,
        current_pump: SpeedMode,
    ) -> Result<Target, SpeedError> {
        let (other, current) = match self.side {
            Side::Fan => (Side::Pump, current_pump),
            Side::Pump => (Side::Fan, current_fan),
        };
        if !restorable_mode(current.0) {
            return Err(SpeedError::OtherSideUnknown {
                side: other,
                raw: current.0,
            });
        }
        let curve = Some(self.points);
        Ok(match self.side {
            Side::Fan => Target {
                fan: mode::CUSTOMIZED,
                pump: current.0,
                fan_curve: curve,
                pump_curve: None,
            },
            Side::Pump => Target {
                fan: current.0,
                pump: mode::CUSTOMIZED,
                fan_curve: None,
                pump_curve: curve,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fan() -> Curve {
        Curve::flat(Side::Fan, 1500).unwrap()
    }

    fn pump() -> Curve {
        Curve::flat(Side::Pump, 2800).unwrap()
    }

    const FAN_E6: [u8; 16] = [
        0x99, 0xE6, 0x00, 0x01, 0x00, 0x05, 0xDC, 0x1E, 0x05, 0xDC, 0x32, 0x05, 0xDC, 0x50, 0x05,
        0xDC,
    ];

    #[test]
    fn pontos_da_curva_plana() {
        let expected = [(0, 1500), (30, 1500), (50, 1500), (80, 1500)]
            .map(|(celsius, rpm)| CurvePoint { celsius, rpm });
        assert_eq!(fan().points(), &expected);
        assert_eq!(fan().side(), Side::Fan);
    }

    #[test]
    fn curva_da_fan() {
        let t = fan().target(SpeedMode(5), SpeedMode(4)).unwrap();
        assert_eq!(
            t.packets(true),
            [
                vec![0x99, 0xE5, 0x01, 0x01, 0x00],
                vec![0x99, 0xE5, 0x02, 0x04, 0x00],
                FAN_E6.to_vec(),
                vec![0x99, 0xB6],
            ]
        );
    }

    #[test]
    fn curva_da_bomba() {
        let t = pump().target(SpeedMode(5), SpeedMode(4)).unwrap();
        let p = t.packets(true);
        assert_eq!(
            p,
            [
                vec![0x99, 0xE5, 0x01, 0x05, 0x00],
                vec![0x99, 0xE5, 0x02, 0x01, 0x00],
                vec![
                    0x99, 0xE6, 0x00, 0x02, 0x00, 0x0A, 0xF0, 0x1E, 0x0A, 0xF0, 0x32, 0x0A, 0xF0,
                    0x50, 0x0A, 0xF0
                ],
                vec![0x99, 0xB6],
            ]
        );
        assert!(!p.iter().any(|p| p.starts_with(&[0x99, 0xE6, 0x00, 0x01])));
    }

    #[test]
    fn curva_da_fan_sem_b6() {
        let t = fan().target(SpeedMode(5), SpeedMode(4)).unwrap();
        assert_eq!(
            t.packets(false),
            [
                vec![0x99, 0xE5, 0x01, 0x01, 0x00],
                vec![0x99, 0xE5, 0x02, 0x04, 0x00],
                FAN_E6.to_vec(),
            ]
        );
    }

    #[test]
    fn rpm_em_big_endian() {
        let t = Curve::flat(Side::Fan, 2500)
            .unwrap()
            .target(SpeedMode(5), SpeedMode(4))
            .unwrap();
        assert_eq!(t.packets(false)[2][13..16], [0x50, 0x09, 0xC4]);
    }

    #[test]
    fn outro_lado_que_o_halo_nao_repete() {
        for raw in [mode::FIXED_RPM, 8, 0xFF] {
            let err = fan().target(SpeedMode(5), SpeedMode(raw)).unwrap_err();
            assert!(
                matches!(err, SpeedError::OtherSideUnknown { side: Side::Pump, raw: r } if r == raw)
            );
            let msg = err.to_string();
            assert!(msg.contains(&format!("é {raw}")), "{msg}");
            assert!(msg.contains("set --pump"), "{msg}");
            let err = pump().target(SpeedMode(raw), SpeedMode(4)).unwrap_err();
            assert!(matches!(
                err,
                SpeedError::OtherSideUnknown {
                    side: Side::Fan,
                    ..
                }
            ));
            assert!(err.to_string().contains("set --fan"), "{err}");
        }
        // Customized e ZeroRPM já estavam no aparelho e são repetidos.
        let t = fan().target(SpeedMode(5), SpeedMode(1)).unwrap();
        assert_eq!((t.fan, t.pump, t.pump_curve), (1, 1, None));
        let t = pump().target(SpeedMode(7), SpeedMode(4)).unwrap();
        assert_eq!((t.fan, t.pump, t.fan_curve), (7, 1, None));
    }

    #[test]
    fn faixa_da_fan() {
        for rpm in [0, 500, 899, 2501, 2800] {
            let err = Curve::flat(Side::Fan, rpm).unwrap_err();
            assert_eq!(
                err,
                CurveError {
                    side: Side::Fan,
                    rpm
                }
            );
            assert!(err.to_string().contains("de 900 a 2500 RPM"), "{err}");
        }
        for rpm in [900, 2500] {
            assert!(Curve::flat(Side::Fan, rpm).is_ok(), "{rpm}");
        }
    }

    #[test]
    fn faixa_da_bomba() {
        let msg = Curve::flat(Side::Pump, 1500).unwrap_err().to_string();
        assert!(msg.contains("de 2600 a 2800 RPM"), "{msg}");
        assert!(msg.contains("Balanced"), "{msg}");

        let msg = Curve::flat(Side::Pump, 3150).unwrap_err().to_string();
        assert!(msg.contains("de 2600 a 2800 RPM"), "{msg}");
        assert!(msg.contains("Turbo"), "{msg}");

        // A faixa da fan não vale para a bomba.
        assert!(Curve::flat(Side::Pump, 2500).is_err());
        for rpm in [2600, 2800] {
            assert!(Curve::flat(Side::Pump, rpm).is_ok(), "{rpm}");
        }
    }
}
