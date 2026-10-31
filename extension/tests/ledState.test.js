// SPDX-License-Identifier: GPL-2.0-or-later

import {test, ok, equal, deepEqual, run} from './assert.js';
import {
    EFFECTS, EMPTY, SWATCHES, activate, activateFinished, controlsView, draft, effectChoice,
    finished, fromLastApplied, retry,
    hueOf, hueToHex, isPalette, lastAppliedChanged, paletteShown, parseHex, powerTarget,
    profileLed, request, sameEffective, commit, usesSpeed, withBrightness, withColor,
    withSpeed,
} from '../lib/ledState.js';

const pulse = {effect: 'pulse', colors: ['ff6600'], brightness: 8, speed: 2};
const triColor = {effect: 'tri-color',
    colors: ['ff0000', '00ff00', '0000ff', '0000ff', '0000ff', '0000ff', '0000ff', '0000ff'],
    brightness: 10, speed: 5};

// Um painel com o ajuste s aplicado e mostrado.
const shown = s => ({...EMPTY, applied: s, draft: s, lastOn: s.effect === 'off' ? null : s});

// ---- 2.1 Catálogo e exibição ----

test('o menu tem só os nove efeitos de uma cor', () => {
    deepEqual(EFFECTS.map(([e]) => e),
        ['static', 'pulse', 'flash', 'double-flash', 'gradient', 'cycle', 'wave', 'rainbow', 'off']);
    ok(EFFECTS.every(([e]) => !isPalette(e)));
});

test('Efeito com cor e velocidade: pulso mostra cor, brilho e velocidade', () => {
    const v = controlsView({...EMPTY, draft: effectChoice(EMPTY, 'pulse')});
    ok(v.showColor);
    ok(v.speedEnabled);
    equal(v.speedText, '5 de 5');
    equal(v.brightnessText, '10 de 10');
});

test('Efeito sem cor: ciclo mostra brilho e velocidade, sem cor', () => {
    const v = controlsView({...EMPTY, draft: effectChoice(EMPTY, 'cycle')});
    ok(!v.showColor);
    ok(v.speedEnabled);
    ok(!v.locked);
});

test('Efeito estático: cor e brilho, velocidade "não se aplica"', () => {
    const v = controlsView({...EMPTY, draft: effectChoice(EMPTY, 'static')});
    ok(v.showColor);
    ok(!v.speedEnabled);
    equal(v.speedText, 'não se aplica');
});

test('LED desligado: últimos valores ligados, travados; efeito e switch ativos', () => {
    const led = {...EMPTY, applied: {...pulse, effect: 'off', colors: []},
        draft: {...pulse, effect: 'off', colors: []}, lastOn: pulse};
    const v = controlsView(led);
    ok(v.locked);
    equal(v.effectLabel, 'Desligado');
    equal(v.color, 'ff6600');
    equal(v.brightness, 8);
    equal(v.speed, 2);
    equal(v.brightnessText, '—');
    ok(!v.speedEnabled);
    ok(!v.power.on);
    ok(v.power.sensitive);
    // Os controles não aceitam mudança.
    equal(withBrightness(led, 3), led.draft);
    equal(withColor(led, '0000ff'), led.draft);
});

test('Cor pronta: o laranja é #FF6600', () => {
    deepEqual(SWATCHES.find(([, n]) => n === 'Laranja'), ['ff6600', 'Laranja']);
    const led = shown({...pulse, colors: ['0000ff']});
    deepEqual(withColor(led, 'ff6600').colors, ['ff6600']);
});

test('Hex inválido: #12345G é recusado, e a cor não muda', () => {
    equal(parseHex('#12345G'), null);
    equal(parseHex('12345'), null);
    equal(parseHex(''), null);
    equal(parseHex(' #FF6600 '), 'ff6600');
    equal(parseHex('aa7cd8'), 'aa7cd8');
    const led = shown(pulse);
    equal(withColor(led, 'zzzzzz'), led.draft);
});

test('Daemon com estado aplicado: pulso, #FF6600, 8 de 10, 2 de 5', () => {
    const s = fromLastApplied({effect: 'pulse', colors: ['ff6600'], brightness: 8, speed: 2});
    const v = controlsView(shown(s));
    equal(v.effectLabel, 'Pulso');
    equal(v.color, 'ff6600');
    equal(v.brightnessText, '8 de 10');
    equal(v.speedText, '2 de 5');
    ok(v.power.on);
});

test('Daemon sem estado aplicado: desconhecido, "Escolher efeito", travado, switch desligado', () => {
    equal(fromLastApplied(null), null);
    const v = controlsView(EMPTY);
    ok(v.unknown);
    equal(v.effect, null);
    equal(v.effectLabel, 'Escolher efeito');
    ok(v.locked);
    ok(!v.power.on);
    ok(v.power.sensitive);
});

test('Perfil com tri-color: três cores mostradas das oito, sem cor editável', () => {
    const v = controlsView(shown(fromLastApplied(triColor)));
    equal(v.effectLabel, 'Três cores');
    deepEqual(v.palette, {name: 'Três cores', colors: ['ff0000', '00ff00', '0000ff']});
    ok(!v.showColor);
    ok(v.speedEnabled);
    ok(!v.locked);
});

test('fromLastApplied: desligado com brilho 0 vira o padrão; efeito desconhecido é null', () => {
    deepEqual(fromLastApplied({effect: 'off', colors: [], brightness: 0, speed: 0}),
        {effect: 'off', colors: [], brightness: 10, speed: 5});
    equal(fromLastApplied({effect: 'strobe', colors: [], brightness: 3, speed: 1}), null);
    deepEqual(fromLastApplied({effect: 'static', colors: ['XYZ', 'ff6600'], brightness: 4, speed: 5}).colors,
        ['ff6600']);
});

test('effectChoice herda brilho, velocidade e cor do rascunho ligado', () => {
    const s = effectChoice(shown({...pulse, colors: ['0000ff']}), 'flash');
    deepEqual(s, {effect: 'flash', colors: ['0000ff'], brightness: 8, speed: 2});
    // de uma paleta para um efeito com cor: a cor padrão
    deepEqual(effectChoice(shown(triColor), 'static').colors, ['ff6600']);
    // desligado: herda do último ligado
    const off = {...EMPTY, draft: {...pulse, effect: 'off', colors: []}, lastOn: pulse};
    deepEqual(effectChoice(off, 'cycle'), {effect: 'cycle', colors: [], brightness: 8, speed: 2});
    // sem nada: os padrões do GCC
    deepEqual(effectChoice(EMPTY, 'static'), {effect: 'static', colors: ['ff6600'], brightness: 10, speed: 5});
});

test('withSpeed só nos efeitos que usam velocidade; valores dentro da faixa', () => {
    const st = shown({effect: 'static', colors: ['ff6600'], brightness: 5, speed: 5});
    equal(withSpeed(st, 2), st.draft);
    equal(withSpeed(shown(pulse), 9).speed, 5);
    equal(withBrightness(shown(pulse), 0.4).brightness, 1);
    ok(usesSpeed('tri-color'));
});

test('paletteShown tira só a repetição final', () => {
    deepEqual(paletteShown(['ff0000', '00ff00', '0000ff', '0000ff', '0000ff']), ['ff0000', '00ff00', '0000ff']);
    deepEqual(paletteShown(['ff0000']), ['ff0000']);
});

test('hueOf e hueToHex', () => {
    equal(hueToHex(0), 'ff0000');
    equal(hueToHex(120), '00ff00');
    equal(hueToHex(240), '0000ff');
    equal(Math.round(hueOf('ff6600')), 24);
    equal(hueOf('808080'), 0);
    equal(hueToHex(hueOf('00ffff')), '00ffff');
});

// ---- 2.2 Pedido, igualdade e fila ----

// Aplica os gestos e as respostas; devolve o estado e os pedidos enviados.
function play(led, steps) {
    const sent = [];
    for (const [kind, arg] of steps) {
        let r;
        if (kind === 'commit')
            r = commit(led, arg);
        else if (kind === 'ok')
            r = finished(led, true);
        else
            r = finished(led, false, arg);
        led = r.led;
        if (r.send)
            sent.push(r.send);
    }
    return {led, sent};
}

test('Arrastar o brilho: um pedido, com brilho 9, ao soltar', () => {
    let led = shown({...pulse, brightness: 3});
    // o arraste muda só o rascunho
    for (const v of [4, 5, 7, 8, 9])
        led = {...led, draft: withBrightness(led, v)};
    equal(led.applied.brightness, 3);
    const {sent} = play(led, [['commit', led.draft]]);
    deepEqual(sent, [{effect: 'pulse', colors: ['ff6600'], brightness: 9, speed: 2}]);
});

test('Mudança durante um pedido em andamento: um em voo e só o último pendente', () => {
    const led = shown(pulse);
    const a = {...pulse, colors: ['0000ff']};
    const b = {...pulse, colors: ['00ff00']};
    const c = {...pulse, colors: ['aa7cd8']};
    const {led: end, sent} = play(led, [['commit', a], ['commit', b], ['commit', c], ['ok'], ['ok']]);
    deepEqual(sent.map(s => s.colors[0]), ['0000ff', 'aa7cd8']);
    deepEqual(end.applied.colors, ['aa7cd8']);
    equal(end.inFlight, null);
    equal(end.pending, null);
});

test('Valor igual ao aplicado: nenhum pedido', () => {
    const led = shown(pulse);
    deepEqual(play(led, [['commit', {...pulse}]]).sent, []);
    // pendente igual ao que acabou de ser aplicado também não sai
    const a = {...pulse, colors: ['0000ff']};
    deepEqual(play(led, [['commit', a], ['commit', a], ['ok']]).sent.length, 1);
});

test('Pedido em andamento falha com uma mudança esperando: pendente descartado', () => {
    const led = shown(pulse);
    const a = {...pulse, colors: ['0000ff']};
    const b = {...pulse, colors: ['00ff00']};
    const {led: end, sent} = play(led, [['commit', a], ['commit', b], ['fail', 'NotFound']]);
    equal(sent.length, 1);
    deepEqual(end.draft, pulse);
    equal(end.pending, null);
    equal(end.inFlight, null);
    equal(end.error, 'NotFound');
    equal(controlsView(end).error, 'NotFound');
    // o gesto seguinte limpa a linha de erro
    equal(commit(end, a).led.error, null);
});

test('Brilho com efeito de paleta: as mesmas cores, brilho 5', () => {
    const led = shown(fromLastApplied(triColor));
    const s = withBrightness(led, 5);
    const {sent} = play(led, [['commit', s]]);
    deepEqual(sent, [{effect: 'tri-color', colors: triColor.colors, brightness: 5, speed: 5}]);
});

test('request manda só o que o efeito usa', () => {
    deepEqual(request({effect: 'off', colors: [], brightness: 8, speed: 2}),
        {effect: 'off', colors: [], brightness: 0, speed: 0});
    deepEqual(request({effect: 'static', colors: ['ff6600', '00ff00'], brightness: 7, speed: 3}),
        {effect: 'static', colors: ['ff6600'], brightness: 7, speed: 0});
    deepEqual(request({effect: 'cycle', colors: ['ff6600'], brightness: 7, speed: 3}),
        {effect: 'cycle', colors: [], brightness: 7, speed: 3});
});

test('igualdade com o que o halod devolve', () => {
    // estático pedido com velocidade 0 volta com 5
    const st = {effect: 'static', colors: ['ff6600'], brightness: 7, speed: 2};
    const back = fromLastApplied({effect: 'static', colors: ['ff6600'], brightness: 7, speed: 5});
    ok(sameEffective(st, back));
    // paleta curta volta com 8
    ok(sameEffective({...triColor, colors: ['ff0000', '00ff00', '0000ff']}, fromLastApplied(triColor)));
    // desligado volta com brilho 0
    ok(sameEffective({effect: 'off', colors: [], brightness: 8, speed: 2},
        fromLastApplied({effect: 'off', colors: [], brightness: 0, speed: 0})));
    ok(!sameEffective(pulse, {...pulse, speed: 3}));
    ok(!sameEffective(pulse, {...pulse, colors: ['0000ff']}));
    ok(!sameEffective(pulse, null));
    ok(sameEffective(null, null));
});

test('nenhuma sequência de gestos com um pedido em voo gera mais de dois envios', () => {
    const colors = ['0000ff', '00ff00', 'aa7cd8', 'f26f98', '53c7b3', 'fffbeb'];
    // Todas as rajadas de 1 a 6 gestos durante um pedido, terminando com sucesso ou falha.
    for (let n = 1; n <= colors.length; n++) {
        for (const end of ['ok', 'fail']) {
            const steps = [['commit', {...pulse, colors: ['ff0000']}]];
            for (const c of colors.slice(0, n))
                steps.push(['commit', {...pulse, colors: [c]}]);
            steps.push([end, 'x'], ['ok']);
            const {sent, led} = play(shown(pulse), steps);
            ok(sent.length <= 2, `${sent.length} envios com ${n} gestos e ${end}`);
            equal(led.inFlight, null);
        }
    }
});

test('LastApplied de outro cliente: atualiza o rascunho só sem nada em voo', () => {
    const led = shown(pulse);
    const other = {effect: 'cycle', colors: [], brightness: 4, speed: 3};
    deepEqual(lastAppliedChanged(led, other).draft, other);
    const busy = commit(led, {...pulse, colors: ['0000ff']}).led;
    deepEqual(lastAppliedChanged(busy, other).draft.colors, ['0000ff']);
    // vazio (daemon novo) não muda nada
    equal(lastAppliedChanged(led, null), led);
});

test('reset de dono: tudo desconhecido', () => {
    const v = controlsView(EMPTY);
    ok(v.unknown);
    equal(EMPTY.lastOn, null);
    equal(EMPTY.inFlight, null);
    equal(EMPTY.pending, null);
});

// ---- 2.3 Liga e desliga ----

const profileTri = {'waterforce.led': {effect: 'tri-color', colors: ['ff0000', '00ff00', '0000ff'],
    brightness: 6, speed: 3}};

test('Desligar: pede o efeito desligado, e o switch fica desligado', () => {
    const led = shown(pulse);
    const off = {...led.draft, effect: 'off', colors: []};
    const r = commit(led, off);
    deepEqual(r.send, {effect: 'off', colors: [], brightness: 0, speed: 0});
    ok(!controlsView(r.led).power.on);
    // com o pedido em voo, o switch fica insensível
    ok(!controlsView(r.led).power.sensitive);
});

test('Religar com estado conhecido: o pulso laranja de novo', () => {
    let led = shown(pulse);
    led = finished(commit(led, {...pulse, effect: 'off', colors: []}).led, true).led;
    deepEqual(led.lastOn, pulse);
    const r = commit(led, powerTarget(led.lastOn, null));
    deepEqual(r.send, {effect: 'pulse', colors: ['ff6600'], brightness: 8, speed: 2});
});

test('Religar sem estado conhecido: o ciclo', () => {
    deepEqual(powerTarget(null, null), {effect: 'cycle', colors: [], brightness: 10, speed: 5});
    deepEqual(powerTarget(null, profileLed({})), {effect: 'cycle', colors: [], brightness: 10, speed: 5});
});

test('Religar com o perfil ativo desligado: o ciclo, e não o desligado', () => {
    const p = profileLed({'waterforce.led': {effect: 'off'}});
    equal(p.effect, 'off');
    equal(powerTarget(null, p).effect, 'cycle');
});

test('religar com o tri-color do perfil', () => {
    const target = powerTarget(null, profileLed(profileTri));
    deepEqual(request(target), {effect: 'tri-color', colors: ['ff0000', '00ff00', '0000ff'],
        brightness: 6, speed: 3});
    // o último ligado vence o perfil
    equal(powerTarget(pulse, profileLed(profileTri)), pulse);
});

test('lastOn zera quando o dono muda', () => {
    // O daemon troca o estado por EMPTY quando o dono some ou muda.
    const led = finished(commit(shown(pulse), {...pulse, effect: 'off', colors: []}).led, true).led;
    ok(led.lastOn !== null);
    equal(EMPTY.lastOn, null);
    equal(powerTarget(EMPTY.lastOn, null).effect, 'cycle');
});

test('profileLed com e sem brilho, velocidade e cores', () => {
    deepEqual(profileLed({'waterforce.led': {effect: 'static'}}),
        {effect: 'static', colors: ['ff6600'], brightness: 10, speed: 5});
    deepEqual(profileLed({'waterforce.led': {effect: 'cycle', colors: ['ff0000'], brightness: 3}}),
        {effect: 'cycle', colors: [], brightness: 3, speed: 5});
    deepEqual(profileLed({'waterforce.led': {effect: 'pulse', colors: ['00ff00'], speed: 2}}),
        {effect: 'pulse', colors: ['00ff00'], brightness: 10, speed: 2});
    equal(profileLed({'waterforce.speed': {fan: 'quiet'}}), null);
    equal(profileLed({'waterforce.led': {effect: 'strobe'}}), null);
    equal(profileLed(null), null);
});

for (const [scenario, color] of [
    ['Depois de sair e entrar na sessão', 'ff0000'],
    ['Reboot com corte de energia', 'ffffff'],
]) {
    test(`${scenario}: abre com o último estático, switch ligado e nenhum envio`, () => {
        const saved = {effect: 'static', colors: [color], brightness: 10, speed: 5};
        const led = lastAppliedChanged({...EMPTY}, fromLastApplied(saved));
        const v = controlsView(led);
        equal(v.effectLabel, 'Estático');
        equal(v.color, color);
        ok(v.power.on);
        ok(!v.unknown);
        equal(led.inFlight, null);
        equal(led.pending, null);
        deepEqual(play(led, []).sent, []);
        deepEqual(led.applied, saved);
    });

    test(`${scenario}: desligar e religar pedem off e a mesma cor`, () => {
        const saved = {effect: 'static', colors: [color], brightness: 10, speed: 5};
        let led = lastAppliedChanged({...EMPTY}, fromLastApplied(saved));
        const off = commit(led, {...led.draft, effect: 'off', colors: []});
        deepEqual(off.send, {effect: 'off', colors: [], brightness: 0, speed: 0});
        led = lastAppliedChanged(off.led, {effect: 'off', colors: [], brightness: 0, speed: 5});
        led = finished(led, true).led;
        const on = commit(led, powerTarget(led.lastOn, null));
        deepEqual(on.send, {effect: 'static', colors: [color], brightness: 10, speed: 0});
    });
}

test('Religar após login com off guardado e sem perfil: pede cycle', () => {
    const led = lastAppliedChanged({...EMPTY}, fromLastApplied({
        effect: 'off', colors: [], brightness: 0, speed: 5,
    }));
    equal(led.lastOn, null);
    ok(!controlsView(led).power.on);
    deepEqual(commit(led, powerTarget(led.lastOn, null)).send,
        {effect: 'cycle', colors: [], brightness: 10, speed: 5});
});

// ---- Perfis: a ativação na fila do LED ----

const noite = {effect: 'static', colors: ['5c9fee'], brightness: 10, speed: 5};
const noiteRec = {...noite};
const roxo = {...pulse, colors: ['aa7cd8']};
const busyErr = {name: 'io.github.leoodaviid.Halo1.Error.Busy', message: 'aparelho ocupado'};

// Os passos de play() e mais os da ativação; devolve o estado e os pedidos.
function playAll(led, steps) {
    const sent = [];
    for (const [kind, arg] of steps) {
        let r;
        if (kind === 'commit')
            r = commit(led, arg);
        else if (kind === 'draft')
            r = {led: draft(led, arg), send: null};
        else if (kind === 'activate')
            r = activate(led, arg);
        else if (kind === 'activated')
            r = activateFinished(led, true);
        else if (kind === 'activateFail')
            r = activateFinished(led, false, arg);
        else if (kind === 'last')
            r = {led: lastAppliedChanged(led, arg), send: null};
        else if (kind === 'retry')
            r = retry(led);
        else if (kind === 'ok')
            r = finished(led, true);
        else
            r = finished(led, false, arg);
        led = r.led;
        if (r.send)
            sent.push(r.send);
    }
    return {led, sent};
}

test('Ativar perfil: sai {activate: noite}, e o LastApplied do perfil vira o rascunho', () => {
    const {led, sent} = playAll(shown(pulse), [['activate', 'noite'], ['last', noiteRec], ['activated']]);
    deepEqual(sent, [{activate: 'noite'}]);
    equal(led.activating, null);
    deepEqual(led.draft, noite);
    deepEqual(led.applied, noite);
});

for (const [order, steps] of [
    ['LastApplied antes da resposta', [['activate', 'noite'], ['last', noiteRec], ['activated']]],
    ['LastApplied depois da resposta', [['activate', 'noite'], ['activated'], ['last', noiteRec]]],
]) {
    test(`Controles depois da ativação (${order}): LED do perfil, e o brilho leva efeito e cor dele`, () => {
        const {led} = playAll(shown(pulse), steps);
        deepEqual(led.draft, noite);
        const r = commit(led, withBrightness(led, 4));
        deepEqual(r.send, {effect: 'static', colors: ['5c9fee'], brightness: 4, speed: 0});
    });
}

for (const [order, steps] of [
    ['LastApplied antes da resposta', [['last', noiteRec], ['activated']]],
    ['LastApplied depois da resposta', [['activated'], ['last', noiteRec]]],
]) {
    test(`Gesto durante a ativação (${order}): o rascunho fica com o brilho do gesto`, () => {
        let led = activate(shown(pulse), 'noite').led;
        const gesture = withBrightness(led, 3);
        const r = playAll(led, [['draft', gesture], ...steps]);
        led = r.led;
        deepEqual(led.draft, gesture);
        equal(led.draft.brightness, 3);
        const c = commit(led, led.draft);
        deepEqual(c.send, request(gesture));
    });
}

test('LastApplied de outro cliente durante um gesto: o rascunho não muda', () => {
    const led = shown(pulse);
    const gesture = withBrightness(led, 3);
    const other = {effect: 'cycle', colors: [], brightness: 4, speed: 3};
    const end = lastAppliedChanged(draft(led, gesture), other);
    deepEqual(end.draft, gesture);
    deepEqual(end.applied, other);
    // o commit envia o gesto
    deepEqual(commit(end, end.draft).send, request(gesture));
});

test('Ativação em andamento: o commit vira pendente e sai depois da resposta', () => {
    const {led, sent} = playAll(shown(pulse),
        [['activate', 'noite'], ['commit', roxo], ['last', noiteRec], ['activated']]);
    deepEqual(sent, [{activate: 'noite'}, request(roxo)]);
    deepEqual(led.inFlight, roxo);
    equal(led.pending, null);
});

test('Ativação em andamento: pendente igual ao aplicado antigo sai mesmo assim', () => {
    const old = {...pulse, brightness: 5};
    const profile = {...pulse, colors: ['5c9fee'], brightness: 10};
    // A resposta chega antes do LastApplied: o applied ainda é o antigo.
    const {sent} = playAll(shown(old),
        [['activate', 'noite'], ['commit', {...old}], ['activated'], ['last', profile]]);
    deepEqual(sent, [{activate: 'noite'}, request(old)]);
    equal(sent[1].brightness, 5);
});

test('Pedido do LED pendente: activate com inFlight ou pending não envia nada', () => {
    const flying = commit(shown(pulse), roxo).led;
    const r1 = activate(flying, 'noite');
    equal(r1.send, null);
    equal(r1.led, flying);
    const waiting = commit(flying, {...pulse, colors: ['00ff00']}).led;
    equal(activate(waiting, 'noite').send, null);
    const activating = activate(shown(pulse), 'noite').led;
    equal(activate(activating, 'jogo').send, null);
});

test('Ativação que falha: o rascunho volta ao aplicado, e failed guarda o nome', () => {
    const {led} = playAll(shown(pulse),
        [['activate', 'jogo'], ['commit', roxo], ['activateFail', busyErr]]);
    deepEqual(led.draft, pulse);
    equal(led.pending, null);
    equal(led.activating, null);
    deepEqual(led.failed, {kind: 'activate', name: 'jogo'});
    deepEqual(led.error, busyErr);
});

test('Tentar de novo: um único pedido com a cor roxa', () => {
    const {led, sent} = playAll(shown(pulse), [['commit', roxo], ['fail', busyErr]]);
    deepEqual(led.failed, {kind: 'apply', setting: roxo});
    const r = playAll(led, [['retry'], ['retry']]);
    deepEqual(r.sent, [request(roxo)]);
    equal(r.led.error, null);
    equal(r.led.failed, null);
    equal(sent.length, 1);
    // a mensagem some quando ele dá certo
    const end = finished(r.led, true).led;
    equal(end.error, null);
    deepEqual(end.applied, roxo);
});

test('Ativação de perfil que falha: o retry pede activate, não Apply', () => {
    const {led} = playAll(shown(pulse), [['activate', 'noite'], ['activateFail', busyErr]]);
    const r = playAll(led, [['retry'], ['retry']]);
    deepEqual(r.sent, [{activate: 'noite'}]);
    equal(r.led.activating, 'noite');
});

test('Mensagem some no pedido seguinte: no commit e no activate', () => {
    const {led} = playAll(shown(pulse), [['commit', roxo], ['fail', busyErr]]);
    ok(led.error !== null);
    const c = commit(led, {...pulse, colors: ['00ff00']}).led;
    equal(c.error, null);
    equal(c.failed, null);
    const a = activate(led, 'noite').led;
    equal(a.error, null);
    equal(a.failed, null);
});

run();
