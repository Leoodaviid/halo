// SPDX-License-Identifier: GPL-2.0-or-later

import {test, equal, deepEqual, run} from './assert.js';
import {
    countPresent, panelView, indicatorDimmed, devicesFrom, lastAppliedFrom,
} from '../lib/view.js';

const running = n => panelView({owner: true, devicesPresent: n, unit: 'loaded'});
const down = unit => panelView({owner: false, devicesPresent: 0, unit});

test('rodando com 0 aparelhos', () =>
    deepEqual(running(0), {kind: 'running', devices: 0, message: 'Nenhum aparelho detectado'}));
test('rodando com 1 aparelho', () =>
    deepEqual(running(1), {kind: 'running', devices: 1, message: '1 aparelho detectado'}));
test('rodando com 3 aparelhos', () =>
    deepEqual(running(3), {kind: 'running', devices: 3, message: '3 aparelhos detectados'}));

test('countPresent ignora o aparelho ausente', () =>
    equal(countPresent([{present: true}, {present: false}, {present: true}]), 2));
test('countPresent com lista vazia ou nula', () => {
    equal(countPresent([]), 0);
    equal(countPresent(null), 0);
});

test('fora do ar com unit carregada permite iniciar e ver o registro', () =>
    deepEqual(down('loaded'), {
        kind: 'down', message: 'O halod não está rodando',
        canStart: true, canJournal: true, reason: null,
    }));
test('fora do ar com unit não encontrada', () =>
    deepEqual(down('not-found'), {
        kind: 'down', message: 'O halod não está rodando',
        canStart: false, canJournal: false,
        reason: 'Serviço não instalado: rode packaging/install.sh',
    }));
test('fora do ar com unit desconhecida', () =>
    deepEqual(down('unknown'), {
        kind: 'down', message: 'O halod não está rodando',
        canStart: false, canJournal: false, reason: 'Verificando o serviço…',
    }));
test('fora do ar com erro do systemd', () =>
    deepEqual(down('error'), {
        kind: 'down', message: 'O halod não está rodando',
        canStart: false, canJournal: false,
        reason: 'O systemd do usuário não respondeu',
    }));

test('indicatorDimmed só com o daemon fora do ar', () => {
    equal(indicatorDimmed({owner: false, devicesPresent: 0, unit: 'loaded'}), true);
    equal(indicatorDimmed({owner: true, devicesPresent: 1, unit: 'loaded'}), false);
});

// O que o Devices() devolve depois do recursiveUnpack.
const unpacked = [
    {id: 'waterforce', name: 'AORUS Waterforce X', present: true,
        devnode: '/dev/hidraw99', capabilities: ['led', 'speed', 'telemetry']},
    {id: 'it5702', name: 'Gigabyte IT5702', present: true,
        devnode: '/dev/hidraw98', capabilities: []},
    {id: 'g502', name: 'Logitech G502', present: false, devnode: '', capabilities: []},
];

test('devicesFrom fica com id, name, present e capabilities', () =>
    deepEqual(devicesFrom(unpacked), [
        {id: 'waterforce', name: 'AORUS Waterforce X', present: true,
            capabilities: ['led', 'speed', 'telemetry']},
        {id: 'it5702', name: 'Gigabyte IT5702', present: true, capabilities: []},
        {id: 'g502', name: 'Logitech G502', present: false, capabilities: []},
    ]));
test('devicesFrom tolera chaves ausentes', () =>
    deepEqual(devicesFrom([{id: 'novo'}]),
        [{id: 'novo', name: 'novo', present: false, capabilities: []}]));
test('devicesFrom com lista vazia, nula ou item sem id', () => {
    deepEqual(devicesFrom([]), []);
    deepEqual(devicesFrom(null), []);
    deepEqual(devicesFrom(undefined), []);
    deepEqual(devicesFrom([{name: 'sem id', present: true}]), []);
});
test('countPresent dá o mesmo sobre o resultado do devicesFrom', () => {
    equal(countPresent(devicesFrom(unpacked)), countPresent(unpacked));
    equal(countPresent(devicesFrom(unpacked)), 2);
});

test('lastAppliedFrom vazio ou sem effect é null', () => {
    equal(lastAppliedFrom({}), null);
    equal(lastAppliedFrom(null), null);
    equal(lastAppliedFrom(undefined), null);
    equal(lastAppliedFrom({colors: ['ff6600'], brightness: 8, speed: 2}), null);
});
test('lastAppliedFrom preserva o pulso laranja', () =>
    deepEqual(lastAppliedFrom({effect: 'pulse', colors: ['ff6600'], brightness: 8, speed: 2}),
        {effect: 'pulse', colors: ['ff6600'], brightness: 8, speed: 2}));
test('lastAppliedFrom do desligado', () =>
    deepEqual(lastAppliedFrom({effect: 'off', colors: [], brightness: 0, speed: 0}),
        {effect: 'off', colors: [], brightness: 0, speed: 0}));
test('lastAppliedFrom tolera colors, brightness e speed ausentes', () =>
    deepEqual(lastAppliedFrom({effect: 'cycle'}),
        {effect: 'cycle', colors: [], brightness: 0, speed: 0}));

run();
