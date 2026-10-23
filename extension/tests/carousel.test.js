// SPDX-License-Identifier: GPL-2.0-or-later

import {test, ok, equal, deepEqual, run} from './assert.js';
import {
    presentDevices, nextIndex, prevIndex, selectAfterChange, statusText,
    wheelStep, lockedText,
} from '../lib/carousel.js';

const waterforce = (name = 'AORUS Waterforce X') =>
    ({id: 'waterforce', name, present: true, capabilities: ['led', 'speed', 'telemetry']});
const board = {id: 'it5702', name: 'Gigabyte IT5702', present: true, capabilities: []};
const g502Absent = {id: 'g502', name: 'Logitech G502', present: false, capabilities: []};

test('Lista do daemon: duas páginas, Disponível e Detectado, sem o G502', () => {
    const pages = presentDevices([waterforce(), board, g502Absent]);
    deepEqual(pages.map(d => d.id), ['waterforce', 'it5702']);
    deepEqual(pages.map(statusText), ['Disponível', 'Detectado']);
});
test('presentDevices com lista vazia ou nula', () => {
    deepEqual(presentDevices([]), []);
    deepEqual(presentDevices(null), []);
});
test('statusText sem capacidades é Detectado', () => {
    equal(statusText({id: 'x', name: 'x', present: true}), 'Detectado');
    equal(statusText(board), 'Detectado');
});

test('Um aparelho só: uma página, e a navegação fica no lugar', () => {
    const pages = presentDevices([waterforce(), {...board, present: false}]);
    equal(pages.length, 1);
    equal(nextIndex(0, 1), 0);
    equal(prevIndex(0, 1), 0);
    equal(selectAfterChange('waterforce', pages), 0);
});

test('Aparelho desconectado: cai no primeiro presente', () => {
    const before = [waterforce(), board];
    equal(selectAfterChange('it5702', before), 1);
    const after = [waterforce(), {...board, present: false}];
    equal(selectAfterChange('it5702', after), 0);
});
test('Aparelho desconectado: sem presentes dá null', () => {
    equal(selectAfterChange('waterforce', [{...waterforce(), present: false}, g502Absent]), null);
    equal(selectAfterChange('waterforce', []), null);
    equal(selectAfterChange(null, []), null);
});
test('sem seleção anterior começa no primeiro', () =>
    equal(selectAfterChange(null, [waterforce(), board]), 0));
test('o índice conta só os presentes', () =>
    equal(selectAfterChange('it5702', [g502Absent, waterforce(), board]), 1));

test('Nome muda: mesma página pelo id, com o nome novo', () => {
    const before = [board, waterforce()];
    const i = selectAfterChange('waterforce', before);
    equal(i, 1);
    const after = [board, waterforce('AORUS Waterforce X 240')];
    equal(selectAfterChange('waterforce', after), i);
    equal(presentDevices(after)[i].name, 'AORUS Waterforce X 240');
});

test('Volta ao início: depois do último vem o primeiro', () => {
    equal(nextIndex(2, 3), 0);
    equal(nextIndex(0, 3), 1);
    equal(nextIndex(1, 2), 0);
});
test('antes do primeiro vem o último', () => {
    equal(prevIndex(0, 3), 2);
    equal(prevIndex(2, 3), 1);
});
test('sem páginas os índices ficam em 0', () => {
    equal(nextIndex(0, 0), 0);
    equal(prevIndex(0, 0), 0);
});

// Passa vários eventos da roda, [dy, agora], e junta os passos.
function wheel(events, gate = null) {
    const steps = [];
    for (const [dy, now] of events) {
        const r = wheelStep(gate, dy, now);
        gate = r.gate;
        steps.push(r.step);
    }
    return {gate, steps};
}

test('Roda do mouse: rolagem rápida dá um passo só', () => {
    const {steps} = wheel([[30, 1000], [30, 1016], [30, 1032], [30, 1048], [30, 1100]]);
    deepEqual(steps, [1, 0, 0, 0, 0]);
});
test('Roda do mouse: outro passo depois de 330 ms', () => {
    const {steps} = wheel([[30, 1000], [30, 1200], [30, 1330], [30, 1331]]);
    // Em 1331 a soma tem 90 e já se passaram 331 ms do passo em 1000.
    deepEqual(steps, [1, 0, 0, 1]);
});
test('Roda do mouse: para cima volta um aparelho', () => {
    const {steps} = wheel([[-30, 5000], [-30, 5010]]);
    deepEqual(steps, [-1, 0]);
});
test('Roda do mouse: deltas pequenos somam até passar do limiar', () => {
    const {steps} = wheel([[10, 1000], [10, 1010], [10, 1020]]);
    deepEqual(steps, [0, 0, 1]);
});
test('Roda do mouse: a soma zera depois de 350 ms parado', () => {
    // Passo em 1000; 10 em 1340 fica na soma; em 1700, 360 ms sem eventos, a
    // soma zera antes de somar 20, que sozinho não passa de 24.
    const zerou = wheel([[30, 1000], [10, 1340], [20, 1700]]);
    deepEqual(zerou.steps, [1, 0, 0]);
    equal(zerou.gate.sum, 20);
    // Sem a pausa, os mesmos 10 + 20 passam do limiar.
    const somou = wheel([[30, 1000], [10, 1340], [20, 1345]]);
    deepEqual(somou.steps, [1, 0, 1]);
});
test('Roda do mouse: a pausa conta do último evento, não do último passo', () => {
    // Eventos a cada 300 ms nunca ficam 350 ms parados: a soma continua.
    const {steps} = wheel([[30, 1000], [10, 1300], [10, 1600], [10, 1650]]);
    deepEqual(steps, [1, 0, 0, 1]);
});
test('Roda do mouse: evento discreto acima do limiar é um passo', () => {
    const r = wheelStep(undefined, 25, 0);
    equal(r.step, 1);
    ok(r.gate);
});
test('Roda do mouse: dy 0 não dá passo', () =>
    equal(wheelStep(null, 0, 1000).step, 0));

test('lockedText usa o nome do daemon', () =>
    equal(lockedText(board),
        'O halod detectou Gigabyte IT5702, mas ainda não tem controle de iluminação para ele.'));

run();
