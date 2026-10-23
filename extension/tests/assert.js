// SPDX-License-Identifier: GPL-2.0-or-later
//
// Asserções mínimas para os testes da extensão, rodados com `gjs -m`.

import System from 'system';

const tests = [];

export function test(name, fn) {
    tests.push({name, fn});
}

export function ok(value, message = 'esperava um valor verdadeiro') {
    if (!value)
        throw new Error(message);
}

export function equal(actual, expected) {
    if (actual !== expected) {
        throw new Error(
            `esperava ${JSON.stringify(expected)}, veio ${JSON.stringify(actual)}`);
    }
}

export function deepEqual(actual, expected) {
    const a = JSON.stringify(actual);
    const e = JSON.stringify(expected);
    if (a !== e)
        throw new Error(`esperava ${e}, veio ${a}`);
}

// Roda os testes registrados; sai com 1 se algum falhar.
export function run() {
    let failures = 0;
    for (const {name, fn} of tests) {
        try {
            fn();
            print(`ok    ${name}`);
        } catch (e) {
            print(`FALHA ${name}: ${e.message}`);
            failures++;
        }
    }
    if (failures > 0)
        System.exit(1);
}
