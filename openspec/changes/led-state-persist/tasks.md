# Tasks

Nenhuma task das seções 1 a 3 envia comandos ao hardware: os testes usam o backend falso do worker ou o sysfs falso com o Castor3 em `hidraw99`, que não existe nesta máquina (`scripts/dev/fake-sysfs.sh`). Só a conferência da 4.2 aplica LED no aparelho, e pelos mesmos `Led.Apply` da `gnome-panel-led`.

## 1. Arquivo do último LED

- [ ] 1.1 Criar `crates/halo-daemon/src/led_state.rs` com `LedState` (`path` = `<state_dir>/waterforce-led.last`), `load() -> Option<LedSetting>` e `save(&LedSetting)` (design, decisões 1 e 2): TOML com os campos de `LedSection`, o desligado sem `brightness`, os efeitos sem cor sem `colors`, a leitura validada por `profiles::led_setting()` e a escrita por `write_atomic()` (passa a `pub(crate)`). Verificar com testes de unidade num diretório temporário: ida e volta do estático vermelho, do `off`, do `cycle` com brilho 3 e do `tri-color` com 8 cores; arquivo ausente → `None`; campo desconhecido, efeito desconhecido, cor `zzzzzz` e brilho 11 → `None`, sem pânico
- [ ] 1.2 Ligar o `LedState` ao `Led` em `bus.rs` e ao `main.rs` (design, decisões 2 e 3): `Led::new(worker, store)` carrega o registro ao criar, `apply_led()` grava depois de atualizar o `LedRecord` (o `Apply` e o `Profiles.Activate` com LED), e uma falha de gravação vira aviso no stderr sem mudar a resposta; o `main.rs` passa o `LedState` do `state_dir`, e sem `state_dir` o daemon segue como hoje. Verificar com testes em `bus.rs` com o backend falso, cobrindo os cenários da spec `daemon-waterforce`: "Depois de aplicar" (o arquivo passa a ter o `cycle`), "Aplicação que falhou" (o arquivo não muda), "Depois de reiniciar" (um `Led` novo sobre o mesmo diretório publica o estático vermelho e o backend falso não recebe nenhuma chamada), "Sem valor guardado", "Valor guardado inválido" e "Falha ao guardar" (diretório sem permissão de escrita: o `Apply` responde com sucesso e o `LastApplied` muda)
- [ ] 1.3 Acrescentar a `crates/halo-daemon/tests/daemon.rs` um teste do binário: com um `waterforce-led.last` (estático vermelho) no `HOME/.local/state/halo` temporário e o sysfs falso, o primeiro `Get` do `LastApplied` já traz o estático vermelho, e o log do `halod` não tem nenhuma abertura de hidraw nem aviso; com um arquivo inválido, o daemon sobe, o `LastApplied` vem vazio e o stderr tem o aviso. Verificar com `cargo test`

## 2. Painel

- [ ] 2.1 Acrescentar a `extension/tests/ledState.test.js` os cenários novos da spec `gnome-panel`, sem mudar o código da extensão: "Depois de sair e entrar na sessão" (estado zerado, depois `lastAppliedChanged` com o estático vermelho → `controlsView` com estático, `ff0000` e o switch ligado, e nenhum envio) e "Religar depois de entrar na sessão" (desligar e religar pedem `off` e o estático vermelho); e "Religar sem estado conhecido" com o último registrado sendo o desligado (pede `cycle`). Verificar com `sh extension/tests/run.sh`

## 3. Documentação

- [ ] 3.1 Atualizar o `README.md` (a propriedade `LastApplied` dura entre execuções do `halod`, o arquivo `waterforce-led.last`, o que ele é e o que não é, e que a CLI não o atualiza), o `docs/architecture.md` (o estado do daemon, o arquivo no `state_dir` e o painel que abre com o último estado depois do login) e o `docs/roadmap.md` (a change na fase 2). `docs/hardware.md` não muda: nada novo é descoberto sobre o aparelho. Verificar que os caminhos citados existem e que o texto bate com o comportamento dos testes

## 4. Integração

- [ ] 4.1 Verificar tudo: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `sh extension/tests/run.sh`, `sh scripts/dev/headless-smoke.sh` e `sh packaging/tests/install-test.sh`
- [ ] 4.2 Com o aval do usuário e no terminal dele: `packaging/install.sh`, `scripts/dev/watch-led-applies.sh` aberto, aplicar o estático vermelho pelo painel (um `Led.Apply`, que faz o `halod` mandar `99 DE 00`, `99 C9`, `99 CD FF 00 00` e um `99 B6`, como em `docs/protocols/castor3.md`, "Anel e RGB das fans"), conferir o `~/.local/state/halo/waterforce-led.last`, sair e entrar na sessão e conferir: o painel abre com o estático vermelho e o switch ligado, e o monitor não mostra nenhum `Apply` no login nem ao abrir o painel; desligar e religar pedem `off` e o estático vermelho (um `B6` cada). Estado final combinado com o usuário

## Workflow follow-up

- Sincronizar os deltas das specs `daemon-waterforce` e `gnome-panel`, arquivar e commitar quando o usuário pedir.
