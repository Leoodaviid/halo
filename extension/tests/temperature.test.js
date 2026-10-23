// SPDX-License-Identifier: GPL-2.0-or-later

import {test, equal, run} from './assert.js';
import {barText} from '../lib/temperature.js';

const t = (milli, valid = true, available = true) =>
    ({available, coolantMilliCelsius: milli, coolantValid: valid});

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
});

run();
