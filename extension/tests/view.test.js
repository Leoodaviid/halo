// SPDX-License-Identifier: GPL-2.0-or-later

import {test, equal, deepEqual, run} from './assert.js';
import {countPresent, panelView, indicatorDimmed} from '../lib/view.js';

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

run();
