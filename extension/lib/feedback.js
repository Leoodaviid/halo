// SPDX-License-Identifier: GPL-2.0-or-later
//
// Os textos do painel em volta de aplicar e ativar: a faixa de erro, a linha
// de status e o botão de perfil, com os textos do protótipo
// (docs/ui/panel-prototype.html, showError() e renderDevice()).

import {effectName} from './ledState.js';

const ERROR_PREFIX = 'io.github.leoodaviid.Halo1.Error.';

const KNOWN = Object.freeze({
    Busy: ['O aparelho está em uso por outro processo',
        'Outro programa do halo está falando com ele. Tente de novo em alguns segundos.'],
    PermissionDenied: ['Sem permissão para abrir o aparelho',
        'Falta a regra udev do halo. Veja docs/permissions.md.'],
    NotFound: ['Aparelho não encontrado', 'Confira se o Waterforce está conectado.'],
    Device: ['Falha de comunicação com o aparelho',
        'O pedido não foi confirmado. O LED pode estar no estado anterior.'],
});

const SECTIONS = Object.freeze({
    'waterforce.speed': 'velocidade',
    'waterforce.led': 'LED',
    'nenhuma': 'nada',
});

// O fim que o Profiles.Activate acrescenta à mensagem (Error::with_context).
const ACTIVATE_CONTEXT = /;\s*aplicadas: (.*?); falharam: (.*?); o perfil ativo não mudou\s*$/;

const UNCHANGED = 'O perfil ativo não mudou.';

function sections(list) {
    return list.split(',').map(s => s.trim()).map(s => SECTIONS[s] ?? s).join(', ');
}

function sentence(text) {
    const t = text.trim();
    return t === '' || /[.!?]$/.test(t) ? t : `${t}.`;
}

// {title, text} da faixa. error: {name, message}, com o nome do erro D-Bus
// (ou sem nome). activation: o pedido que falhou foi um Profiles.Activate.
export function errorView(error, activation = false) {
    const name = String(error?.name ?? '').replace(ERROR_PREFIX, '');
    let message = String(error?.message ?? '');
    const context = activation ? ACTIVATE_CONTEXT.exec(message) : null;
    if (context)
        message = message.slice(0, context.index);
    const [title, text] = KNOWN[name] ?? ['O halod recusou o pedido', message];
    if (!activation)
        return {title, text};
    const parts = [sentence(text)];
    if (context)
        parts.push(`Já aplicado: ${sections(context[1])}.`, `Falhou: ${sections(context[2])}.`);
    parts.push(UNCHANGED);
    return {title, text: parts.filter(p => p !== '').join(' ')};
}

// A linha de status do aparelho com LED: {icon: 'spinner' | 'check' | null, text}.
export function statusView(led, deviceName) {
    if (led.inFlight !== null)
        return {icon: 'spinner', text: led.pending !== null ? 'Aplicando… (1 na fila)' : 'Aplicando…'};
    if (led.activating !== null)
        return {icon: 'spinner', text: 'Ativando perfil…'};
    if (led.applied === null)
        return {icon: null, text: 'Estado do LED desconhecido'};
    if (led.applied.effect === 'off')
        return {icon: null, text: 'LED desligado'};
    return {icon: 'check', text: `Aplicado ao ${deviceName}`};
}

// O botão de perfil do cabeçalho. owner: o halod está no ar; activating: o
// perfil em ativação ou null; active: o ActiveProfile ('' sem perfil).
export function profileLabel(owner, activating, active) {
    if (!owner)
        return '—';
    if (activating !== null)
        return 'Ativando…';
    return active ? `Perfil: ${active}` : 'Sem perfil';
}

// O efeito ao lado do perfil no menu (o .hint), do LED de profileLed, ou ''.
export function profileHint(led) {
    return led ? effectName(led.effect).toLowerCase() : '';
}
