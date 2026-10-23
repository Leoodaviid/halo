// SPDX-License-Identifier: GPL-2.0-or-later
//
// Texto da temperatura do líquido na barra superior.

// telemetry: { available, coolantMilliCelsius, coolantValid }.
// Indisponível: só o ícone ('' ). Inválida: '--'. Válida: inteiro com '°'.
export function barText(telemetry) {
    if (!telemetry || !telemetry.available)
        return '';
    if (!telemetry.coolantValid)
        return '--';
    return `${Math.round(telemetry.coolantMilliCelsius / 1000)}°`;
}
