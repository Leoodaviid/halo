# Proposal

## Why

Depois da `gnome-panel-devices`, o painel mostra o carrossel e o anel do Waterforce com a cor do `LastApplied`, mas ainda não muda nada: o LED só se controla pela CLI. Esta é a segunda das duas changes do painel e a primeira em que a extensão escreve no hardware, mesmo que indiretamente: cada `Led.Apply` do `halod` manda ao Castor3 a sequência da `GcLedLib.dll` e termina sempre com `99 B6`, que grava no aparelho e cujo desgaste não é conhecido. Por isso, os controles chegam junto com as garantias de que cada gesto gera no máximo um pedido, de que nada é enviado sem gesto e de que os testes não tocam no aparelho.

## What Changes

- **Controles de LED do Waterforce** no corpo do aparelho, no lugar do aviso provisório da `gnome-panel-devices`: popup de efeito com os nove efeitos de uma cor, seletor de cor (seis cores prontas, barra de matiz e hex), brilho de 1 a 10 e velocidade de 1 a 5, com o que cada efeito usa. Velocidade como "não se aplica" no estático; cor, brilho e velocidade esmaecidos e travados com o LED desligado, mostrando os últimos valores ligados; efeitos de paleta mostrados com o nome e as cores, sem edição da paleta.
- **Aplicação só ao concluir o gesto**: soltar o slider, escolher efeito ou cor, confirmar o hex. No máximo um `Apply` em voo e só o último valor pendente; nenhum pedido igual ao estado aplicado. Durante o arraste, a mudança aparece só na interface (inclusive no desenho do anel).
- **Liga/desliga do cabeçalho habilitado**: desligar pede o efeito `off`; religar reaplica o último estado ligado visto nesta instância do daemon, depois o LED do perfil ativo (lido com `Profiles.ActiveProfile` e `Profiles.Get`, só leitura) e, por fim, `cycle`.
- **Estado vindo do daemon**: o painel abre com o `LastApplied`; sem ele, indica que o estado do LED não é conhecido e não envia nada até um gesto. Depois de uma falha, os controles voltam ao último estado aplicado, com uma linha mínima de erro (as mensagens por tipo de erro e o "Tentar de novo" ficam para a `gnome-panel-extras`).
- **Lógica pura** em `extension/lib/ledState.js` (efeitos, controles visíveis, desligado, fila, igualdade, religar, cor), testada com `gjs -m`, e a camada St em `extension/ledControls.js`.
- **Testes sem o aparelho**: a fumaça headless e o `nested-shell.sh --daemon` passam a usar um `halod` com sysfs falso cujo nó de dispositivo não existe, então todo `Apply` falha com `NotFound`. A `gnome-panel-devices` já troca o sysfs falso da fumaça para `hidraw99` (antes era `hidraw3`, que nesta máquina é o teclado Redragon); esta change acrescenta à fumaça um `Apply` que precisa voltar `NotFound` e passa o `--daemon` do nested, que hoje lê o `/sys` real, ao mesmo sysfs falso.
- **Conferência final** no aparelho, com o aval do usuário, contando os `Apply` e os `99 B6` de cada gesto por um monitor do D-Bus da sessão (sem root).
- Fora do escopo, na `gnome-panel-extras`: perfis no cabeçalho, detalhes, as mensagens de erro por tipo com "Tentar de novo", linha de status, preferências e itens de menu.

## Capabilities

### New Capabilities

Nenhuma.

### Modified Capabilities
- `gnome-panel`: dois cenários novos em requisitos que esta change implementa, para casos que a spec deixa ambíguos e que decidem quantas vezes o aparelho grava:
  - "Aplicação só ao concluir o gesto": quando o pedido em andamento falha, a mudança que esperava é descartada e não é enviada;
  - "Liga e desliga": um perfil ativo cujo LED é o desligado não serve para religar, e o painel pede o `cycle`.

## Impact

- Novos: `extension/lib/ledState.js`, `extension/ledControls.js`, `extension/tests/ledState.test.js` e um monitor dos `Apply` só de leitura para a conferência (`scripts/dev/watch-led-applies.sh`).
- Alterados: `extension/daemon.js` (`apply(setting)` pelo proxy de `Waterforce.Led`, com `Cancellable`, e a leitura do LED do perfil ativo por um proxy de `Profiles` com `DO_NOT_AUTO_START`), `extension/indicator.js` (controles no `body` e o liga/desliga), `extension/carousel.js` (o anel com o rascunho, por `setLed`), `extension/stylesheet.css` (medidas), `extension/stylesheet-light.css` e `extension/stylesheet-dark.css` (cores de cada tema), `scripts/dev/headless-smoke.sh` e `scripts/dev/nested-shell.sh` (este passa ao `scripts/dev/fake-sysfs.sh` que a `gnome-panel-devices` criou).
- Documentação: `docs/architecture.md`, `docs/roadmap.md`, `README.md` e `docs/hardware.md` (o que a conferência observar, como a contagem de `B6` por gesto e os efeitos vistos pela primeira vez).
- Hardware: nenhum comando novo. O painel só chama `Led.Apply`, e o `halod` manda os pacotes já documentados em `docs/protocols/castor3.md` ("Anel e RGB das fans"): `99 DE 00`, `99 C9`, `99 CD`, `99 B0`–`B3` e um `99 B6` no fim. A diferença é a frequência: um `B6` por gesto concluído.
- `crates/`: nada muda. Depende da `gnome-panel-devices`, que precisa estar implementada antes desta começar.
