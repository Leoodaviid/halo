// SPDX-License-Identifier: GPL-2.0-or-later
//
// Prepara a saída do journalctl para mostrar no painel.

export const JOURNAL_LINES = 20;

// Linhas que o journalctl imprime quando não há registro.
const EMPTY_MARKERS = ['-- No entries --', '-- Sem entradas --'];

// As últimas `max` linhas do texto, sem linhas vazias nas pontas.
export function journalLines(text, max = JOURNAL_LINES) {
    if (!text)
        return [];
    const lines = text.split('\n');
    while (lines.length > 0 && lines[0].trim() === '')
        lines.shift();
    while (lines.length > 0 && lines[lines.length - 1].trim() === '')
        lines.pop();
    if (lines.length === 1 && EMPTY_MARKERS.includes(lines[0].trim()))
        return [];
    return lines.slice(-max);
}

export function journalText(lines) {
    if (lines.length === 0)
        return 'Sem registro';
    return lines.join('\n');
}
