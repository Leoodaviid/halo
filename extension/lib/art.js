// SPDX-License-Identifier: GPL-2.0-or-later
//
// Desenhos dos aparelhos como texto SVG, com a cor do LED já aplicada.
//
// A arte é a do protótipo (docs/ui/panel-prototype.html: drawWaterforce,
// drawBoard e drawGeneric). O carregador de imagens não tem CSS, então as
// classes .led-stroke e .led-fill viraram atributos com a pintura resolvida, o
// ledRainbow foi para o <defs> do Waterforce e a sombra (CSS) ficou de fora.
// O texto começa com a declaração XML seguida direto do <svg>: é assim que o
// carregador reconhece o formato.

// Cinza do LED sem estado e desligado; o desligado fica esmaecido como o
// .led-off do protótipo (opacity .6 no lugar do .92 do desenho).
const LED_GRAY = '#4a4a50';
const LED_OPACITY = '.92';
const LED_OFF_OPACITY = '.6';

const COLOR_RE = /^[0-9a-f]{6}$/;

const HAS_COLOR = new Set(['static', 'pulse', 'flash', 'double-flash', 'gradient']);
const PALETTE = new Set(['color-shift', 'tri-color', 'spin', 'switch', 'lego', 'aurora']);
const RAINBOW = new Set(['cycle', 'wave', 'rainbow']);

const NEUTRAL = {color: null, off: false, rainbow: false};

// Decide a pintura do LED a partir do LastApplied ({effect, colors, ...} ou
// null), como o setLedPaint() do protótipo. Diferença: sem estado fica neutro
// (cinza com a opacidade do desenho), e só o desligado fica esmaecido.
export function ledPaint(lastApplied) {
    if (!lastApplied || typeof lastApplied.effect !== 'string')
        return {...NEUTRAL};
    const effect = lastApplied.effect;
    if (effect === 'off')
        return {color: null, off: true, rainbow: false};
    if (RAINBOW.has(effect))
        return {color: null, off: false, rainbow: true};
    if (HAS_COLOR.has(effect) || PALETTE.has(effect)) {
        const colors = Array.isArray(lastApplied.colors) ? lastApplied.colors : [];
        const first = colors[0];
        if (typeof first === 'string' && COLOR_RE.test(first))
            return {color: first, off: false, rainbow: false};
    }
    return {...NEUTRAL};
}

function escapeXml(s) {
    return String(s).replace(/[&<>"']/g, c => ({
        '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;',
    })[c]);
}

// A pintura e a opacidade das partes do LED. Uma cor fora do formato não
// entra no texto: vira o neutro.
function ledAttrs(led) {
    if (led && led.off)
        return {paint: LED_GRAY, opacity: LED_OFF_OPACITY};
    if (led && led.rainbow)
        return {paint: 'url(#ledRainbow)', opacity: LED_OPACITY};
    if (led && typeof led.color === 'string' && COLOR_RE.test(led.color))
        return {paint: `#${led.color}`, opacity: LED_OPACITY};
    return {paint: LED_GRAY, opacity: LED_OPACITY};
}

// O St.Icon carrega a imagem num quadrado sem preservar a proporção. Por isso o
// SVG é quadrado: a arte de 300x156 fica centrada num viewBox de 300x300
// (72 de folga em cima e embaixo), e o carrossel corta a folga.
function svg(content) {
    return '<?xml version="1.0" encoding="UTF-8"?>\n' +
        '<svg xmlns="http://www.w3.org/2000/svg" width="300" height="300" viewBox="0 -72 300 300">' +
        `${content}</svg>`;
}

const LED_RAINBOW = '<linearGradient id="ledRainbow" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ff3b30"/><stop offset=".25" stop-color="#ffcc00"/><stop offset=".5" stop-color="#34c759"/><stop offset=".75" stop-color="#0a84ff"/><stop offset="1" stop-color="#bf5af2"/></linearGradient>';

function drawWaterforce(led) {
    const {paint, opacity} = ledAttrs(led);
    const blades = Array.from({length: 7}, (_, i) =>
        `<path d="M 2 -3 C 22 -29 8 -28 -2 -27 C -9 -18 -10 -10 2 -3Z" transform="rotate(${i * 360 / 7})"/>`).join('');
    const fan = (cx, cy) => `<g transform="translate(${cx} ${cy})"><g><rect x="-39" y="-39" width="78" height="78" rx="8" fill="#1c2024" stroke="#36393f" stroke-width="2"/><circle r="35" fill="#12161b" stroke="#3b3e42" stroke-width="1.4"/><circle r="29" fill="#23262c"/></g><circle r="27" fill="none" opacity="${opacity}" stroke-width="5" stroke="${paint}"/><g><g fill="#343941" stroke="#41464d" stroke-width=".8">${blades}</g><circle r="9" fill="#0e1317" stroke="#5c6268"/><circle r="4" fill="#9b8e5a"/></g></g>`;
    return svg(`<defs><linearGradient id="wfCase" x1="0" y1="0" x2="1" y2="1"><stop stop-color="#34383f"/><stop offset=".48" stop-color="#12171d"/><stop offset="1" stop-color="#45484c"/></linearGradient>${LED_RAINBOW}</defs><g transform="translate(15 19)"><g><rect width="180" height="103" rx="8" fill="url(#wfCase)" stroke="#1d2227" stroke-width="2"/></g>${fan(50, 51)}${fan(129, 51)}<g><path d="M190 37 Q236 25 243 65" fill="none" stroke="#343a42" stroke-width="12" stroke-linecap="round"/><path d="M191 49 Q225 44 228 84" fill="none" stroke="#272b33" stroke-width="12" stroke-linecap="round"/></g><g transform="translate(239 96)"><g><circle r="42" fill="url(#wfCase)" stroke="#14191e" stroke-width="1.5"/><circle r="34" fill="#0d1319" stroke="#596169" stroke-width="2"/></g><circle r="26" stroke-width="3.2" fill="none" opacity="${opacity}" stroke="${paint}"/><path d="M-11 8 1 -14 13 -3 1 -5 -7 11 Z" fill="#dadee4" opacity=".76"/><path d="M-7 12 7 -9 5 5Z" opacity="${opacity}" fill="${paint}"/></g></g>`);
}

function drawBoard() {
    const rs = Array.from({length: 24}, (_, i) =>
        `<rect x="${(i % 6) * 7 + 11}" y="${Math.floor(i / 6) * 8 + 11}" width="3.5" height="5" rx=".6" fill="#666b71"/>`).join('');
    return svg(`<g transform="translate(63 3)"><rect width="170" height="145" rx="2" fill="#20252b" stroke="#454b52" stroke-width="1.3"/><path d="M15 8H75L69 47H15Z" fill="#30353b"/><path d="M17 8H65L39 23H17Z" fill="#5b646c"/><rect x="76" y="28" width="50" height="48" rx="3" fill="#292e34" stroke="#79818a"/><rect x="85" y="35" width="33" height="33" fill="#1f2429" stroke="#4c5158"/>${rs}<path d="M77 80H130L126 84H77Z" fill="#c1c7ce"/><rect x="142" y="9" width="7" height="78" fill="#0c1115" stroke="#636a70"/><rect x="153" y="9" width="7" height="78" fill="#0c1115" stroke="#636a70"/><rect x="164" y="9" width="4" height="78" fill="#0c1115" stroke="#636a70"/><rect x="10" y="102" width="139" height="9" fill="#a8afb6"/><rect x="10" y="115" width="139" height="7" fill="#1c2329" stroke="#737b83"/><rect x="107" y="120" width="37" height="23" fill="#2c3238"/><path d="M20 42H58L33 69" fill="none" stroke="#6f7880" stroke-width="3"/><path d="M25 135L55 119M21 130l41-30" fill="none" stroke="#5c666e" stroke-width="3"/></g>`);
}

// O texto usa a cor fixa do desenho (#8a9198), legível nos dois temas, no
// lugar do currentColor do protótipo.
function drawGeneric(name) {
    return svg(`<g><rect x="95" y="16" width="110" height="82" rx="14" fill="#2a2e34" stroke="#4b5158" stroke-width="2"/><path d="M135 46h30M150 38v22" stroke="#8a9198" stroke-width="5" stroke-linecap="round"/><rect x="138" y="66" width="24" height="12" rx="3" fill="#8a9198"/><text x="150" y="132" text-anchor="middle" font-size="15" fill="#8a9198" opacity=".75" font-family="sans-serif">${escapeXml(name)}</text></g>`);
}

// O texto SVG do aparelho. led = { color: 'rrggbb' | null, off, rainbow }; só
// o Waterforce tem partes de LED, os outros o ignoram.
export function deviceSvg(device, led) {
    if (device.id === 'waterforce')
        return drawWaterforce(led);
    if (device.id === 'it5702')
        return drawBoard();
    return drawGeneric(device.name ?? device.id);
}
