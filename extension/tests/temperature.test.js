// SPDX-License-Identifier: GPL-2.0-or-later

import {test, equal, run} from './assert.js';
import {CPU, COOLANT, barText} from '../lib/temperature.js';

const t = (milli, valid = true, available = true) =>
    ({available, coolantMilliCelsius: milli, coolantValid: valid});

// Líquido a 34 °C e a CPU dada.
const c = (milli, valid = true, published = true, available = true) => ({
    ...t(34000, true, available),
    cpuPublished: published, cpuMilliCelsius: milli, cpuValid: valid,
});

test('38,4 °C arredonda para 38°', () => equal(barText(t(38400)), '38°'));
test('38,5 °C arredonda para 39°', () => equal(barText(t(38500)), '39°'));
test('224 °C inválido vira -- e nunca 224°', () => {
    equal(barText(t(224000, false)), '--');
    equal(barText(t(224000, true)) === '--', false);
});
test('telemetria indisponível mostra só o ícone', () =>
    equal(barText(t(38400, true, false)), ''));
test('telemetria nula mostra só o ícone', () => {
    equal(barText(null), '');
    equal(barText(undefined), '');
    equal(barText(null, CPU), '');
});

test('sem fonte, o líquido é o padrão', () => equal(barText(c(61200)), '34°'));
test('líquido escolhido ignora a CPU', () => equal(barText(c(61200), COOLANT), '34°'));
test('troca para a CPU: 61,2 °C vira 61°', () => equal(barText(c(61200), CPU), '61°'));
test('CPU inválida com valor antigo vira -- e nunca 45°', () =>
    equal(barText(c(45000, false), CPU), '--'));
test('CPU sem Waterforce mostra a CPU', () =>
    equal(barText(c(61200, true, true, false), CPU), '61°'));
test('CPU escolhida e não publicada mostra só o ícone', () =>
    equal(barText(c(0, false, false), CPU), ''));

run();
