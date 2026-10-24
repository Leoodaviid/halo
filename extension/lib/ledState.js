// SPDX-License-Identifier: GPL-2.0-or-later
//
// Estado do LED do Waterforce no painel, sem St: o catálogo de efeitos, o que
// os controles mostram, o que vai em cada Led.Apply, a fila (um pedido em voo
// e só o último pendente) e o religar. Cada Apply termina com 99 B6, que grava
// no aparelho, então toda regra que decide se um pedido sai fica aqui, testada.
//
// Um ajuste é {effect, colors: ['rrggbb'], brightness, speed}. O estado:
//   applied   o último ajuste confirmado (LastApplied ou o Apply que deu certo)
//   draft     o que a interface mostra
//   lastOn    o último ajuste ligado visto nesta instância do daemon
//   inFlight  o ajuste do Apply em andamento
//   pending   só o último ajuste esperando
//   powering  lendo o perfil ativo para religar
//   error     texto da última falha
// Os nulos querem dizer desconhecido ou nenhum.

// Os nove efeitos de uma cor, na ordem do protótipo e com os nomes de
// Effect::NAMES do halo-core.
export const EFFECTS = Object.freeze([
    ['static', 'Estático'], ['pulse', 'Pulso'], ['flash', 'Flash'],
    ['double-flash', 'Flash duplo'], ['gradient', 'Gradiente'], ['cycle', 'Ciclo'],
    ['wave', 'Onda'], ['rainbow', 'Arco-íris'], ['off', 'Desligado'],
]);

// Os de paleta: só mostrados, nunca oferecidos.
export const PALETTE_EFFECTS = Object.freeze({
    'color-shift': 'Transição de cores', 'tri-color': 'Três cores', 'spin': 'Giro',
    'switch': 'Alternância', 'lego': 'Lego', 'aurora': 'Aurora',
});

export const HAS_COLOR = new Set(['static', 'pulse', 'flash', 'double-flash', 'gradient']);
export const HAS_SPEED = new Set(['pulse', 'flash', 'double-flash', 'gradient', 'cycle', 'wave', 'rainbow']);

export const SWATCHES = Object.freeze([
    ['fffbeb', 'Branco quente'], ['ff6600', 'Laranja'], ['f26f98', 'Rosa'],
    ['aa7cd8', 'Roxo'], ['5c9fee', 'Azul'], ['53c7b3', 'Turquesa'],
]);

// Os padrões do GCC, como no halod (Brightness::default, Speed::default,
// Effect::default_colors).
export const DEFAULT_BRIGHTNESS = 10;
export const DEFAULT_SPEED = 5;
export const DEFAULT_COLOR = 'ff6600';

// O daemon completa a paleta até 8 cores repetindo a última.
const PALETTE_SIZE = 8;

const COLOR_RE = /^[0-9a-f]{6}$/;
const ONE_COLOR = new Set(EFFECTS.map(([e]) => e));

const CYCLE = Object.freeze({effect: 'cycle', colors: [], brightness: DEFAULT_BRIGHTNESS, speed: DEFAULT_SPEED});

export const EMPTY = Object.freeze({
    applied: null, draft: null, lastOn: null, inFlight: null, pending: null,
    powering: false, error: null,
});

export function isPalette(effect) {
    return Object.hasOwn(PALETTE_EFFECTS, effect);
}

export function usesSpeed(effect) {
    return HAS_SPEED.has(effect) || isPalette(effect);
}

function knownEffect(effect) {
    return ONE_COLOR.has(effect) || isPalette(effect);
}

export function effectName(effect) {
    const found = EFFECTS.find(([e]) => e === effect);
    return found ? found[1] : PALETTE_EFFECTS[effect] ?? effect;
}

function inRange(v, min, max, fallback) {
    return Number.isInteger(v) && v >= min && v <= max ? v : fallback;
}

function validColors(colors) {
    return Array.isArray(colors) ? colors.filter(c => typeof c === 'string' && COLOR_RE.test(c)) : [];
}

// O LastApplied já normalizado por lastAppliedFrom (lib/view.js), ou null.
// O brilho 0 do desligado e os zeros de "padrão" viram os padrões do GCC.
export function fromLastApplied(rec) {
    if (!rec || typeof rec.effect !== 'string' || !knownEffect(rec.effect))
        return null;
    return {
        effect: rec.effect,
        colors: validColors(rec.colors),
        brightness: inRange(rec.brightness, 1, 10, DEFAULT_BRIGHTNESS),
        speed: inRange(rec.speed, 1, 5, DEFAULT_SPEED),
    };
}

// A paleta sem a repetição final que o daemon acrescenta.
export function paletteShown(colors) {
    const shown = [...colors];
    while (shown.length > 1 && shown[shown.length - 1] === shown[shown.length - 2])
        shown.pop();
    return shown;
}

function padPalette(colors) {
    const out = [...colors];
    while (out.length > 0 && out.length < PALETTE_SIZE)
        out.push(out[out.length - 1]);
    return out;
}

// O que os controles mostram. Com o LED desligado ou desconhecido, os últimos
// valores ligados (ou os padrões), travados. owner: o halod está no ar.
export function controlsView(led, owner = true) {
    const draft = led.draft;
    const unknown = draft === null;
    const locked = unknown || draft.effect === 'off';
    const base = locked ? led.lastOn ?? {...CYCLE, effect: 'static', colors: [DEFAULT_COLOR]} : draft;
    const palette = isPalette(base.effect);
    const speedUsed = usesSpeed(base.effect);
    return {
        unknown,
        effect: unknown ? null : draft.effect,
        effectLabel: unknown ? 'Escolher efeito' : effectName(draft.effect),
        locked,
        palette: palette ? {name: effectName(base.effect), colors: paletteShown(base.colors)} : null,
        showColor: !palette && (HAS_COLOR.has(base.effect) || locked),
        color: (!palette && base.colors[0]) || DEFAULT_COLOR,
        brightness: base.brightness,
        brightnessText: locked ? '—' : `${base.brightness} de 10`,
        speed: base.speed,
        speedEnabled: !locked && speedUsed,
        speedText: locked ? '—' : speedUsed ? `${base.speed} de 5` : 'não se aplica',
        power: {
            on: !locked,
            sensitive: owner && led.inFlight === null && !led.powering,
        },
        error: led.error,
    };
}

// O rascunho ao escolher um efeito no menu. Herda brilho, velocidade e cor do
// rascunho ligado, senão do último ligado, senão dos padrões.
export function effectChoice(led, effect) {
    const on = led.draft && led.draft.effect !== 'off' ? led.draft : null;
    const base = on ?? led.lastOn ?? {effect: 'static', colors: [DEFAULT_COLOR],
        brightness: DEFAULT_BRIGHTNESS, speed: DEFAULT_SPEED};
    const color = (!isPalette(base.effect) && base.colors[0]) || DEFAULT_COLOR;
    return {
        effect,
        colors: HAS_COLOR.has(effect) ? [color] : [],
        brightness: base.brightness,
        speed: base.speed,
    };
}

function editable(draft) {
    return draft !== null && draft.effect !== 'off';
}

export function withColor(led, hex) {
    const d = led.draft;
    if (!editable(d) || !HAS_COLOR.has(d.effect) || !COLOR_RE.test(hex))
        return d;
    return {...d, colors: [hex]};
}

export function withBrightness(led, value) {
    const d = led.draft;
    if (!editable(d))
        return d;
    return {...d, brightness: Math.min(10, Math.max(1, Math.round(value)))};
}

export function withSpeed(led, value) {
    const d = led.draft;
    if (!editable(d) || !usesSpeed(d.effect))
        return d;
    return {...d, speed: Math.min(5, Math.max(1, Math.round(value)))};
}

// "#FF6600", "ff6600" ou " #ff6600 " -> "ff6600"; qualquer outra coisa, null.
export function parseHex(text) {
    const v = String(text ?? '').trim().replace(/^#/, '').toLowerCase();
    return COLOR_RE.test(v) ? v : null;
}

export function hueOf(hex) {
    const [r, g, b] = [0, 2, 4].map(i => parseInt(hex.slice(i, i + 2), 16) / 255);
    const max = Math.max(r, g, b), min = Math.min(r, g, b), d = max - min;
    if (!d)
        return 0;
    let h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
    h *= 60;
    return h < 0 ? h + 360 : h;
}

export function hueToHex(h) {
    const f = n => {
        const k = (n + h / 60) % 6;
        return Math.round(255 * (1 - Math.max(0, Math.min(k, 4 - k, 1))));
    };
    return [f(5), f(3), f(1)].map(v => v.toString(16).padStart(2, '0')).join('');
}

// Os argumentos do Led.Apply: só o que o efeito usa, como a CLI.
export function request(s) {
    if (s.effect === 'off')
        return {effect: 'off', colors: [], brightness: 0, speed: 0};
    let colors = [];
    if (HAS_COLOR.has(s.effect))
        colors = s.colors.slice(0, 1);
    else if (isPalette(s.effect))
        colors = [...s.colors];
    return {
        effect: s.effect,
        colors,
        brightness: s.brightness,
        speed: usesSpeed(s.effect) ? s.speed : 0,
    };
}

// Dois ajustes iguais no aparelho: o mesmo efeito, e o resto só no que o efeito
// usa (um estático pedido com velocidade 0 volta do daemon com 5).
export function sameEffective(a, b) {
    if (!a || !b)
        return a === b;
    if (a.effect !== b.effect)
        return false;
    if (a.effect === 'off')
        return true;
    if (a.brightness !== b.brightness)
        return false;
    if (usesSpeed(a.effect) && a.speed !== b.speed)
        return false;
    if (HAS_COLOR.has(a.effect))
        return a.colors[0] === b.colors[0];
    if (isPalette(a.effect))
        return padPalette(a.colors).join() === padPalette(b.colors).join();
    return true;
}

function remember(lastOn, applied) {
    return applied && applied.effect !== 'off' ? applied : lastOn;
}

// Fim de um gesto. Devolve o estado novo e o pedido a enviar, ou null.
export function commit(led, setting) {
    const next = {...led, draft: setting, error: null};
    if (led.inFlight !== null)
        return {led: {...next, pending: setting}, send: null};
    if (sameEffective(setting, led.applied))
        return {led: next, send: null};
    return {led: {...next, inFlight: setting}, send: request(setting)};
}

// Resposta do Apply em voo. Na falha, o pendente é descartado e os controles
// voltam ao aplicado.
export function finished(led, ok, error = null) {
    if (led.inFlight === null)
        return {led, send: null};
    if (!ok) {
        return {
            led: {...led, inFlight: null, pending: null, draft: led.applied, error},
            send: null,
        };
    }
    const applied = led.inFlight;
    const base = {...led, applied, lastOn: remember(led.lastOn, applied), inFlight: null,
        pending: null, error: null};
    const p = led.pending;
    if (p !== null && !sameEffective(p, applied))
        return {led: {...base, inFlight: p}, send: request(p)};
    return {led: base, send: null};
}

// Um LastApplied novo do daemon (rec já normalizado por lastAppliedFrom). Sem
// nada em voo nem pendente, a interface passa a mostrá-lo.
export function lastAppliedChanged(led, rec) {
    const s = fromLastApplied(rec);
    if (s === null)
        return led;
    const next = {...led, applied: s, lastOn: remember(led.lastOn, s)};
    if (led.inFlight === null && led.pending === null)
        next.draft = s;
    return next;
}

// O LED do perfil, do a{sv} de Profiles.Get já desempacotado, ou null.
export function profileLed(dict) {
    const rec = dict?.['waterforce.led'];
    if (!rec || typeof rec.effect !== 'string' || !knownEffect(rec.effect))
        return null;
    let colors = validColors(rec.colors);
    if (HAS_COLOR.has(rec.effect) && colors.length === 0)
        colors = [DEFAULT_COLOR];
    if (!HAS_COLOR.has(rec.effect) && !isPalette(rec.effect))
        colors = [];
    return {
        effect: rec.effect,
        colors,
        brightness: inRange(rec.brightness, 1, 10, DEFAULT_BRIGHTNESS),
        speed: inRange(rec.speed, 1, 5, DEFAULT_SPEED),
    };
}

// O que religar pede: o último ligado, depois o LED do perfil ativo se ele não
// for o desligado, por fim o ciclo, o mais próximo do modo de fábrica.
export function powerTarget(lastOn, profile) {
    if (lastOn)
        return lastOn;
    if (profile && profile.effect !== 'off')
        return profile;
    return {...CYCLE, colors: []};
}
