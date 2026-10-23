// SPDX-License-Identifier: GPL-2.0-or-later

import {test, equal, deepEqual, run} from './assert.js';
import {JOURNAL_LINES, journalLines, journalText} from '../lib/journal.js';

const numbered = n => Array.from({length: n}, (_, i) => `linha ${i + 1}`);

test('mais de 20 linhas ficam só as últimas 20', () => {
    const out = journalLines(numbered(30).join('\n'));
    equal(out.length, JOURNAL_LINES);
    equal(out[0], 'linha 11');
    equal(out[19], 'linha 30');
});
test('respeita o máximo informado', () =>
    deepEqual(journalLines('a\nb\nc', 2), ['b', 'c']));
test('descarta linhas vazias do começo e do fim', () =>
    deepEqual(journalLines('\n\na\n\nb\n\n'), ['a', '', 'b']));
test('texto vazio ou nulo dá lista vazia', () => {
    deepEqual(journalLines(''), []);
    deepEqual(journalLines(null), []);
    deepEqual(journalLines('\n\n'), []);
});
test('-- No entries -- conta como vazio', () => {
    deepEqual(journalLines('-- No entries --\n'), []);
    deepEqual(journalLines('-- Sem entradas --\n'), []);
});
test('journalText vazio vira Sem registro', () =>
    equal(journalText([]), 'Sem registro'));
test('journalText junta as linhas', () =>
    equal(journalText(['a', 'b']), 'a\nb'));

run();
