// SPDX-License-Identifier: GPL-2.0-or-later

import {test, equal, deepEqual, run} from './assert.js';
import {errorView, profileHint, profileLabel, statusView} from '../lib/feedback.js';
import {EMPTY, profileLed} from '../lib/ledState.js';

const err = (kind, message = 'detalhe do daemon') =>
    ({name: `io.github.leoodaviid.Halo1.Error.${kind}`, message});

// O contexto que a activate do bus.rs acrescenta (Error::with_context).
const withContext = (message, applied, failed) =>
    `${message}; aplicadas: ${applied}; falharam: ${failed}; o perfil ativo não mudou`;

const pulse = {effect: 'pulse', colors: ['ff6600'], brightness: 8, speed: 2};

// ---- Erros ----

test('Aparelho ocupado', () => {
    deepEqual(errorView(err('Busy')), {
        title: 'O aparelho está em uso por outro processo',
        text: 'Outro programa do halo está falando com ele. Tente de novo em alguns segundos.',
    });
});

test('Sem permissão: aponta a regra udev', () => {
    deepEqual(errorView(err('PermissionDenied')), {
        title: 'Sem permissão para abrir o aparelho',
        text: 'Falta a regra udev do halo. Veja docs/permissions.md.',
    });
});

test('Aparelho não encontrado', () => {
    deepEqual(errorView(err('NotFound')), {
        title: 'Aparelho não encontrado',
        text: 'Confira se o Waterforce está conectado.',
    });
});

test('Falha de comunicação', () => {
    deepEqual(errorView(err('Device')), {
        title: 'Falha de comunicação com o aparelho',
        text: 'O pedido não foi confirmado. O LED pode estar no estado anterior.',
    });
});

test('Erro sem tipo conhecido: o halod recusou, com o texto dele', () => {
    deepEqual(errorView(err('InvalidArgument', 'efeito desconhecido: strobe')),
        {title: 'O halod recusou o pedido', text: 'efeito desconhecido: strobe'});
    deepEqual(errorView(err('ProfileNotFound', 'perfil não encontrado: noite')),
        {title: 'O halod recusou o pedido', text: 'perfil não encontrado: noite'});
    // sem nome (erro local, sem D-Bus)
    deepEqual(errorView({name: null, message: 'tempo esgotado'}),
        {title: 'O halod recusou o pedido', text: 'tempo esgotado'});
    deepEqual(errorView({message: 'tempo esgotado'}).title, 'O halod recusou o pedido');
});

test('Ativação: o texto termina com "O perfil ativo não mudou."', () => {
    equal(errorView(err('Busy'), true).text,
        'Outro programa do halo está falando com ele. Tente de novo em alguns segundos. ' +
        'O perfil ativo não mudou.');
    // ProfileNotFound sai antes do contexto: só a frase final
    equal(errorView(err('ProfileNotFound', 'perfil não encontrado: noite'), true).text,
        'perfil não encontrado: noite. O perfil ativo não mudou.');
});

test('Ativação parcial: velocidade aplicada e LED falhou', () => {
    const v = errorView(err('Busy',
        withContext('aparelho ocupado', 'waterforce.speed', 'waterforce.led')), true);
    equal(v.title, 'O aparelho está em uso por outro processo');
    equal(v.text, 'Outro programa do halo está falando com ele. Tente de novo em alguns segundos. ' +
        'Já aplicado: velocidade. Falhou: LED. O perfil ativo não mudou.');
});

test('Ativação parcial: velocidade falhou e LED aplicado', () => {
    const v = errorView(err('Device',
        withContext('modelo desconhecido', 'waterforce.led', 'waterforce.speed')), true);
    equal(v.text, 'O pedido não foi confirmado. O LED pode estar no estado anterior. ' +
        'Já aplicado: LED. Falhou: velocidade. O perfil ativo não mudou.');
});

test('Ativação parcial: nenhuma aplicada; e o texto do erro genérico sem o contexto', () => {
    const v = errorView(err('Storage',
        withContext('não consegui gravar', 'nenhuma', 'waterforce.speed, waterforce.led')), true);
    equal(v.title, 'O halod recusou o pedido');
    equal(v.text, 'não consegui gravar. Já aplicado: nada. Falhou: velocidade, LED. ' +
        'O perfil ativo não mudou.');
});

test('Fora de uma ativação, o contexto não é lido', () => {
    equal(errorView(err('Storage', withContext('x', 'nenhuma', 'waterforce.led'))).text,
        withContext('x', 'nenhuma', 'waterforce.led'));
});

// ---- Linha de status ----

const NAME = 'AORUS Waterforce X 240';

test('Aplicando com fila', () => {
    deepEqual(statusView({...EMPTY, applied: pulse, inFlight: pulse, pending: pulse}, NAME),
        {icon: 'spinner', text: 'Aplicando… (1 na fila)'});
    deepEqual(statusView({...EMPTY, applied: pulse, inFlight: pulse}, NAME),
        {icon: 'spinner', text: 'Aplicando…'});
});

test('Aplicado: "Aplicado ao AORUS Waterforce X 240" com o check', () => {
    deepEqual(statusView({...EMPTY, applied: pulse, draft: pulse}, NAME),
        {icon: 'check', text: 'Aplicado ao AORUS Waterforce X 240'});
});

test('Ativando perfil', () => {
    deepEqual(statusView({...EMPTY, applied: pulse, activating: 'noite', pending: pulse}, NAME),
        {icon: 'spinner', text: 'Ativando perfil…'});
});

test('LED desligado', () => {
    deepEqual(statusView({...EMPTY, applied: {...pulse, effect: 'off', colors: []}}, NAME),
        {icon: null, text: 'LED desligado'});
});

test('Estado desconhecido', () => {
    deepEqual(statusView(EMPTY, NAME), {icon: null, text: 'Estado do LED desconhecido'});
});

// ---- Botão de perfil ----

test('Perfil ativo marcado: "Perfil: noite"; sem perfil: "Sem perfil"', () => {
    equal(profileLabel(true, null, 'noite'), 'Perfil: noite');
    equal(profileLabel(true, null, ''), 'Sem perfil');
});

test('Ativação em andamento: "Ativando…"', () => {
    equal(profileLabel(true, 'noite', 'jogo'), 'Ativando…');
});

test('Daemon fora do ar: "—"', () => {
    equal(profileLabel(false, null, 'noite'), '—');
    equal(profileLabel(false, 'noite', 'noite'), '—');
});

test('o efeito ao lado do perfil, em minúsculas', () => {
    equal(profileHint(profileLed({'waterforce.led': {effect: 'tri-color', colors: ['ff0000']}})),
        'três cores');
    equal(profileHint(profileLed({'waterforce.led': {effect: 'static'}})), 'estático');
    equal(profileHint(null), '');
});

run();
