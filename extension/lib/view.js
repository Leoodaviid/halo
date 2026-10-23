// SPDX-License-Identifier: GPL-2.0-or-later
//
// Decide o que o painel mostra a partir do estado do daemon e do systemd.

// Quantos aparelhos da lista estão presentes.
export function countPresent(devices) {
    if (!devices)
        return 0;
    return devices.filter(d => d && d.present).length;
}

function runningMessage(n) {
    if (n === 0)
        return 'Nenhum aparelho detectado';
    if (n === 1)
        return '1 aparelho detectado';
    return `${n} aparelhos detectados`;
}

// Por que o botão de iniciar e o do registro estão desligados, por unit.
const REASONS = {
    'not-found': 'Serviço não instalado: rode packaging/install.sh',
    'unknown': 'Verificando o serviço…',
    'error': 'O systemd do usuário não respondeu',
};

// state: { owner, devicesPresent, unit }, unit em
// 'unknown' | 'loaded' | 'not-found' | 'error'.
export function panelView(state) {
    if (state.owner) {
        return {
            kind: 'running',
            devices: state.devicesPresent,
            message: runningMessage(state.devicesPresent),
        };
    }
    const loaded = state.unit === 'loaded';
    return {
        kind: 'down',
        message: 'O halod não está rodando',
        canStart: loaded,
        canJournal: loaded,
        reason: loaded ? null : (REASONS[state.unit] ?? REASONS['unknown']),
    };
}

// O ícone fica apagado enquanto o daemon não está no ar.
export function indicatorDimmed(state) {
    return !state.owner;
}

// A lista do Devices() (ou do DevicesChanged) já desempacotada com
// recursiveUnpack, só com o que o painel usa. Tolera chaves ausentes: sem
// present é ausente, sem capabilities é lista vazia, sem name usa o id.
export function devicesFrom(list) {
    if (!Array.isArray(list))
        return [];
    return list
        .filter(d => d && typeof d.id === 'string')
        .map(d => ({
            id: d.id,
            name: typeof d.name === 'string' && d.name !== '' ? d.name : d.id,
            present: d.present === true,
            capabilities: Array.isArray(d.capabilities)
                ? d.capabilities.filter(c => typeof c === 'string')
                : [],
        }));
}

// O LastApplied do Waterforce.Led já desempacotado. Vazio (antes da primeira
// aplicação) ou sem effect dá null.
export function lastAppliedFrom(dict) {
    if (!dict || typeof dict.effect !== 'string' || dict.effect === '')
        return null;
    return {
        effect: dict.effect,
        colors: Array.isArray(dict.colors)
            ? dict.colors.filter(c => typeof c === 'string')
            : [],
        brightness: Number(dict.brightness ?? 0),
        speed: Number(dict.speed ?? 0),
    };
}
