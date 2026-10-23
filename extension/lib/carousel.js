// SPDX-License-Identifier: GPL-2.0-or-later
//
// Lógica do carrossel de aparelhos, sem St: quais páginas existem, qual fica
// selecionada quando a lista muda, o status de cada aparelho e a roda do mouse.
//
// Um aparelho é { id, name, present, capabilities: ['led', ...] }.

// A roda dá um passo quando a soma dos deltas passa do limiar e o último passo
// foi há mais de WHEEL_STEP_MS; a soma zera depois de WHEEL_RESET_MS sem
// eventos. São os números do protótipo.
export const WHEEL_THRESHOLD = 24;
export const WHEEL_STEP_MS = 330;
export const WHEEL_RESET_MS = 350;

// Só os presentes, na ordem da lista do daemon.
export function presentDevices(devices) {
    if (!devices)
        return [];
    return devices.filter(d => d && d.present);
}

// Próxima página, voltando ao início depois da última.
export function nextIndex(i, n) {
    if (n <= 0)
        return 0;
    return (((i + 1) % n) + n) % n;
}

// Página anterior, indo para a última antes da primeira.
export function prevIndex(i, n) {
    if (n <= 0)
        return 0;
    return (((i - 1) % n) + n) % n;
}

// O índice, entre os presentes, do aparelho que estava selecionado (pelo id,
// então um nome novo não muda a página). Se ele saiu, o primeiro; sem
// presentes, null.
export function selectAfterChange(prevId, devices) {
    const present = presentDevices(devices);
    if (present.length === 0)
        return null;
    const keep = present.findIndex(d => d.id === prevId);
    return keep >= 0 ? keep : 0;
}

function hasLed(device) {
    return Boolean(device && Array.isArray(device.capabilities) &&
        device.capabilities.includes('led'));
}

export function statusText(device) {
    return hasLed(device) ? 'Disponível' : 'Detectado';
}

// gate: o estado da roda que o chamador guarda entre eventos
// ({sum, lastEvent, lastStep}), ou null/undefined no começo. dy positivo é a
// roda para baixo (avança). Devolve o gate novo e o passo: -1, 0 ou 1.
//
// O protótipo mede a pausa a partir do último passo; aqui ela conta do último
// evento ("350 ms parado"), senão os deltas pequenos do touchpad, que só
// passam do limiar somados, nunca dariam o primeiro passo.
export function wheelStep(gate, dy, now) {
    const lastStep = gate ? gate.lastStep : -Infinity;
    const lastEvent = gate ? gate.lastEvent : -Infinity;
    let sum = gate ? gate.sum : 0;
    if (now - lastEvent > WHEEL_RESET_MS)
        sum = 0;
    sum += dy;
    if (Math.abs(sum) > WHEEL_THRESHOLD && now - lastStep > WHEEL_STEP_MS)
        return {gate: {sum: 0, lastEvent: now, lastStep: now}, step: Math.sign(sum)};
    return {gate: {sum, lastEvent: now, lastStep}, step: 0};
}

// O aviso de um aparelho que o halod vê mas ainda não controla.
export function lockedText(device) {
    return `O halod detectou ${device.name}, mas ainda não tem controle de iluminação para ele.`;
}
