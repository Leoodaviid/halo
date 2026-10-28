// SPDX-License-Identifier: GPL-2.0-or-later

import {test, deepEqual, run} from './assert.js';
import {CPU_HINT, CPU_MISSING_HINT, cpuOption} from '../lib/prefs.js';
import {COOLANT, CPU} from '../lib/temperature.js';

test('CPU indisponível no daemon: desabilitada, com o motivo', () =>
    deepEqual(cpuOption(false, COOLANT), {sensitive: false, hint: CPU_MISSING_HINT}));
test('CPU disponível no daemon: habilitada, com a origem', () =>
    deepEqual(cpuOption(true, COOLANT), {sensitive: true, hint: CPU_HINT}));
test('CPU escolhida e indisponível: selecionável, com o motivo', () =>
    deepEqual(cpuOption(false, CPU), {sensitive: true, hint: CPU_MISSING_HINT}));
test('CPU escolhida e disponível', () =>
    deepEqual(cpuOption(true, CPU), {sensitive: true, hint: CPU_HINT}));

run();
