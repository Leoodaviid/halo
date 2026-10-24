// SPDX-License-Identifier: GPL-2.0-or-later

import GdkPixbuf from 'gi://GdkPixbuf?version=2.0';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {test, ok, equal, deepEqual, run} from './assert.js';
import {deviceSvg, ledPaint} from '../lib/art.js';

const waterforce = {id: 'waterforce', name: 'AORUS Waterforce X', present: true, capabilities: ['led']};
const board = {id: 'it5702', name: 'Gigabyte IT5702', present: true, capabilities: []};
const generic = name => ({id: 'novo', name, present: true, capabilities: []});

const NEUTRAL = {color: null, off: false, rainbow: false};
const OFF = {color: null, off: true, rainbow: false};
const RAINBOW = {color: null, off: false, rainbow: true};
const BLUE = {color: '0000ff', off: false, rainbow: false};

const HEAD = '<?xml version="1.0" encoding="UTF-8"?>\n<svg xmlns="http://www.w3.org/2000/svg" ';

// Decodifica o texto como o St.Icon faz: num quadrado, sem preservar a
// proporção. Um SVG que não fosse quadrado sairia distorcido aqui também.
function decode(svg) {
    const bytes = new GLib.Bytes(new TextEncoder().encode(svg));
    return GdkPixbuf.Pixbuf.new_from_stream_at_scale(
        Gio.MemoryInputStream.new_from_bytes(bytes), 300, 300, false, null);
}

function pixel(pb, x, y) {
    const i = y * pb.get_rowstride() + x * pb.get_n_channels();
    const px = pb.get_pixels();
    return [px[i], px[i + 1], px[i + 2]];
}

// Ponto do anel da ventoinha da esquerda do Waterforce. A ventoinha está em
// translate(15 19) + translate(50 51) = (65, 70), e o anel tem r=27 com
// stroke-width 5, ou seja, vai do raio 24,5 ao 29,5. Com a folga de 72 do
// viewBox quadrado, o ponto (92, 70) da arte vira o pixel (92, 142), com o
// centro em (92,5; 70,5) da arte: raio 27,5, no meio do traço, à direita do centro,
// onde nenhuma pá passa por cima. O anel tem opacity .92 sobre o fundo escuro
// da ventoinha (#23262c), então a cor sai quase pura, não exata.
const RING = [92, 142];

const ringOf = led => pixel(decode(deviceSvg(waterforce, led)), ...RING);

test('o texto começa com a declaração XML seguida direto do <svg>', () => {
    for (const d of [waterforce, board, generic('X')])
        ok(deviceSvg(d, BLUE).startsWith(HEAD), `início errado para ${d.id}`);
});
test('tamanho e viewBox fixos, sem class, filter nem currentColor', () => {
    for (const d of [waterforce, board, generic('X')]) {
        for (const led of [NEUTRAL, OFF, RAINBOW, BLUE]) {
            const s = deviceSvg(d, led);
            ok(s.includes('width="300" height="300" viewBox="0 -72 300 300"'), d.id);
            ok(!/class=|filter|currentColor|inherit|<!--/.test(s), d.id);
            ok(s.endsWith('</svg>'), d.id);
        }
    }
});
test('o Waterforce leva o ledRainbow no <defs>', () => {
    const s = deviceSvg(waterforce, RAINBOW);
    ok(s.includes('<linearGradient id="ledRainbow"'));
    ok(s.includes('stroke="url(#ledRainbow)"'));
    ok(s.includes('fill="url(#ledRainbow)"'));
});
test('a cor do LED entra no Waterforce', () => {
    const s = deviceSvg(waterforce, BLUE);
    ok(s.includes('stroke="#0000ff"'));
    ok(s.includes('fill="#0000ff"'));
});
test('cor fora do formato não entra no texto', () => {
    const s = deviceSvg(waterforce, {color: '"/><x', off: false, rainbow: false});
    ok(!s.includes('<x'));
    deepEqual(s, deviceSvg(waterforce, NEUTRAL));
});
test('o nome do genérico é escapado', () => {
    const s = deviceSvg(generic('<a&b>'), NEUTRAL);
    ok(s.includes('&lt;a&amp;b&gt;'));
    ok(!s.includes('<a&b>'));
    ok(deviceSvg(generic(`"x'`), NEUTRAL).includes('&quot;x&apos;'));
});
test('o genérico usa cor fixa e sans-serif', () => {
    const s = deviceSvg(generic('Mouse'), NEUTRAL);
    ok(s.includes('fill="#8a9198" opacity=".75" font-family="sans-serif">Mouse</text>'));
});
test('a placa-mãe e o genérico ignoram o led no texto', () => {
    for (const led of [OFF, RAINBOW, BLUE]) {
        equal(deviceSvg(board, led), deviceSvg(board, NEUTRAL));
        equal(deviceSvg(generic('Y'), led), deviceSvg(generic('Y'), NEUTRAL));
    }
});

test('ledPaint: sem estado é neutro', () => {
    deepEqual(ledPaint(null), NEUTRAL);
    deepEqual(ledPaint(undefined), NEUTRAL);
});
test('ledPaint: off é esmaecido', () =>
    deepEqual(ledPaint({effect: 'off', colors: [], brightness: 0, speed: 0}), OFF));
test('ledPaint: cycle, wave e rainbow são arco-íris', () => {
    for (const effect of ['cycle', 'wave', 'rainbow'])
        deepEqual(ledPaint({effect, colors: [], brightness: 8, speed: 3}), RAINBOW);
});
test('ledPaint: static com 0000ff', () =>
    deepEqual(ledPaint({effect: 'static', colors: ['0000ff'], brightness: 10, speed: 0}), BLUE));
test('ledPaint: efeitos de uma cor usam a cor', () => {
    for (const effect of ['pulse', 'flash', 'double-flash', 'gradient'])
        deepEqual(ledPaint({effect, colors: ['ff6600'], brightness: 8, speed: 2}).color, 'ff6600');
});
test('ledPaint: tri-color usa a primeira cor da paleta', () =>
    deepEqual(ledPaint({effect: 'tri-color', colors: ['ff0000', '00ff00', '0000ff'], brightness: 10, speed: 3}),
        {color: 'ff0000', off: false, rainbow: false}));
test('ledPaint: cor inválida é neutro', () => {
    for (const colors of [['00FF00'], ['#00ff00'], ['12345g'], ['fff'], [], undefined])
        deepEqual(ledPaint({effect: 'static', colors, brightness: 10, speed: 0}), NEUTRAL);
});
test('ledPaint: efeito desconhecido é neutro', () => {
    deepEqual(ledPaint({effect: 'strobe', colors: ['ff0000'], brightness: 10, speed: 3}), NEUTRAL);
    deepEqual(ledPaint({colors: ['ff0000']}), NEUTRAL);
});

test('cada desenho decodifica em 300×300, quadrado como o St.Icon', () => {
    for (const d of [waterforce, board, generic('<a&b>')]) {
        for (const led of [NEUTRAL, OFF, RAINBOW, BLUE]) {
            const pb = decode(deviceSvg(d, led));
            equal(pb.get_width(), 300);
            equal(pb.get_height(), 300);
        }
    }
});
// O St.Icon não preserva a proporção: com o SVG quadrado, decodificar com e
// sem preservar dá a mesma imagem. Um SVG de 300x156 sairia esticado.
test('o desenho não distorce no quadrado do St.Icon', () => {
    for (const d of [waterforce, board, generic('G502')]) {
        const bytes = new GLib.Bytes(new TextEncoder().encode(deviceSvg(d, BLUE)));
        const load = keep => GdkPixbuf.Pixbuf.new_from_stream_at_scale(
            Gio.MemoryInputStream.new_from_bytes(bytes), 300, 300, keep, null);
        const kept = load(true);
        const stretched = load(false);
        equal(kept.get_height(), 300, d.id);
        const a = kept.get_pixels(), b = stretched.get_pixels();
        ok(a.length === b.length && a.every((v, i) => v === b[i]),
            `${d.id}: a imagem muda sem preservar a proporção`);
    }
});

test('Cor do LED: o anel do Waterforce sai azul com 0000ff', () => {
    const [r, g, b] = ringOf(ledPaint({effect: 'static', colors: ['0000ff'], brightness: 10, speed: 0}));
    ok(b >= 230 && r <= 10 && g <= 10, `esperava azul, veio ${r} ${g} ${b}`);
});
test('o anel neutro é o cinza #4a4a50 quase opaco', () => {
    const [r, g, b] = ringOf(NEUTRAL);
    // 0,92 × #4a4a50 + 0,08 × #23262c ≈ (71, 71, 77).
    ok(Math.abs(r - 71) <= 3 && Math.abs(g - 71) <= 3 && Math.abs(b - 77) <= 3,
        `esperava cinza, veio ${r} ${g} ${b}`);
});
test('o anel desligado é cinza e mais escuro que o neutro', () => {
    const [r, g, b] = ringOf(OFF);
    // 0,6 × #4a4a50 + 0,4 × #23262c ≈ (58, 60, 66).
    ok(Math.abs(r - 58) <= 3 && Math.abs(g - 60) <= 3 && Math.abs(b - 66) <= 3,
        `esperava cinza esmaecido, veio ${r} ${g} ${b}`);
    const [nr, ng, nb] = ringOf(NEUTRAL);
    ok(r < nr && g < ng && b < nb, 'o desligado devia ser mais escuro que o neutro');
});
test('o anel arco-íris não é cinza', () => {
    const [r, g, b] = ringOf(RAINBOW);
    ok(Math.max(r, g, b) - Math.min(r, g, b) > 60, `veio ${r} ${g} ${b}`);
});
test('o desenho da placa-mãe é igual com qualquer led', () => {
    const base = decode(deviceSvg(board, NEUTRAL)).get_pixels();
    for (const led of [OFF, RAINBOW, BLUE]) {
        const px = decode(deviceSvg(board, led)).get_pixels();
        equal(px.length, base.length);
        ok(px.every((v, i) => v === base[i]), 'os pixels mudaram com o led');
    }
});

run();
