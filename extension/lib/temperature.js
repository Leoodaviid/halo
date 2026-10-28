// SPDX-License-Identifier: GPL-2.0-or-later
//
// Texto da temperatura na barra superior, do líquido ou da CPU.

// Fontes da chave temperature-source do schema.
export const COOLANT = 'coolant';
export const CPU = 'cpu';

function degrees(milli) {
    return `${Math.round(milli / 1000)}°`;
}

// telemetry: { available, coolantMilliCelsius, coolantValid, cpuPublished,
// cpuMilliCelsius, cpuValid }. Sem leitura da fonte: só o ícone (''). Inválida:
// '--'. Válida: inteiro com '°'. A CPU não depende do Waterforce, então
// `available` só vale para o líquido.
export function barText(telemetry, source = COOLANT) {
    if (!telemetry)
        return '';
    if (source === CPU) {
        if (!telemetry.cpuPublished)
            return '';
        return telemetry.cpuValid ? degrees(telemetry.cpuMilliCelsius) : '--';
    }
    if (!telemetry.available)
        return '';
    if (!telemetry.coolantValid)
        return '--';
    return degrees(telemetry.coolantMilliCelsius);
}
