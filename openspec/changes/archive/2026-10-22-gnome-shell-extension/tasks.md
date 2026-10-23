# Tasks

## 1. Lógica pura

- [x] 1.1 Criar `extension/tests/assert.js` e `extension/tests/run.sh` (roda cada `*.test.js` com `gjs -m` e falha se algum falhar). Verificar que `sh extension/tests/run.sh` falha com um teste de exemplo que não passa e passa sem ele
- [x] 1.2 Criar `extension/lib/temperature.js` com `barText` (decisão 2): só o ícone sem `available`, `--` com `coolantValid` falso, e graus inteiros arredondados. Verificar com testes dos cenários "Temperatura disponível" (38,4 °C → `38°`), "Líquido inválido" (224 °C inválido → `--`, nunca `224°`) e "Telemetria indisponível" da spec `gnome-panel`, mais o arredondamento de 38,5 °C
- [x] 1.3 Criar `extension/lib/view.js` com `panelView` e `extension/lib/journal.js` (decisões 2 e 4): daemon rodando com a contagem de aparelhos presentes; daemon fora do ar com as ações habilitadas, ou desabilitadas com o motivo quando a unidade é `not-found`; o registro limitado às últimas 20 linhas e "Sem registro" quando vazio. Verificar com testes dos cenários "Daemon fora do ar" e "Unidade não instalada" e da contagem com um aparelho ausente na lista

## 2. Extensão e teste de fumaça

- [x] 2.1 Criar `extension/metadata.json`, `extension/extension.js`, `extension/indicator.js`, `extension/icons/halo-symbolic.svg` (o ícone do protótipo em caminhos preenchidos) e `extension/stylesheet.css` (decisões 1 e 5), com o indicador ainda sem D-Bus: ícone esmaecido e painel de daemon fora do ar. `disable()` destrói o indicador. Verificar com a task 2.2
- [x] 2.2 Criar `scripts/dev/headless-smoke.sh` (decisão 6): Shell 50 headless com `dbus-run-session` e `HOME`, `XDG_*` e `XDG_RUNTIME_DIR` temporários, a extensão do repositório copiada e habilitada pelo `gsettings` do temporário, `gnome-extensions info` em `ACTIVE`, um ciclo `disable`/`enable` de volta a `ACTIVE`, nenhum `JS ERROR` no log, e a conferência de que `/run/user/$UID/gnome-shell-disable-extensions` não foi criado pelo script. Verificar que ele passa com a extensão da 2.1 e falha com um erro posto de propósito no `enable()`

## 3. Cliente do halod e do systemd

- [x] 3.1 Criar `extension/daemon.js` (decisão 3): proxies de `Manager` e `Waterforce.Telemetry` com `DO_NOT_AUTO_START`, o dono pelo `notify::g-name-owner`, telemetria pelo `g-properties-changed`, `Devices()` ao aparecer o dono e a cada `DevicesChanged`, tudo cancelado no `disable()`. Ligar ao indicador. Verificar com a fumaça da 2.2 estendida: com um `target/debug/halod` (`--address` do barramento do teste, `--sys-root` com um Castor3 e um hwmon falsos) a extensão continua `ACTIVE` e sem `JS ERROR`, e sem nenhum `halod` no barramento o nome continua sem dono depois de 10 s com a extensão ativa (cenário "Daemon parado pelo usuário": o painel não ativa o daemon)
- [x] 3.2 Acrescentar ao `daemon.js` o estado da unidade (`LoadUnit` + `LoadState`), "Iniciar halod" (`ResetFailedUnit` e `StartUnit("halod.service", "replace")`) e "Ver registro" (`journalctl --user -u halod.service -n 20 --no-pager -o short-iso` por `Gio.Subprocess`, com "Voltar"), como na decisão 4. Verificar com a fumaça (sem systemd no barramento do teste, a unidade aparece como indisponível e nada quebra) e com os testes da 1.3

## 4. Shell aninhado

- [x] 4.1 Criar `scripts/dev/nested-shell.sh` (decisões 6 e 8): `--devkit` com o isolamento completo, `WAYLAND_DISPLAY` absoluto, a extensão do repositório e, com `--daemon`, um `target/debug/halod` no barramento do teste lendo o `/sys` real (só leitura). Com o aval do usuário, rodar e conferir com ele: a janela aparece, o indicador mostra a temperatura do líquido, o painel abre com a contagem de aparelhos; sem `--daemon`, o ícone esmaecido e as ações de iniciar e de registro desabilitadas; e, ao fechar, a sessão real intacta (sem o marcador de crash, as extensões dele ativas, nenhuma sessão nova no `loginctl list-sessions`). Se a sessão real for afetada, registrar no design e deixar a conferência visual para a task 6.2

## 5. Instalação

- [x] 5.1 Acrescentar a extensão ao `packaging/install.sh` e ao `--uninstall` (decisão 7): staging sem `tests/`, troca só quando mudou, inclusão e remoção do UUID na lista do `gsettings` sem mexer nas outras, aviso de sair e entrar só quando mudou, aviso quando a gravação falhar ou quando `disable-user-extensions` estiver ligado. Verificar com o `packaging/tests/install-test.sh` estendido com um `gsettings` falso: os cenários "Primeira instalação da extensão", "Reinstalar sem mudar a extensão", "Habilitar sem sessão gráfica", "Habilitar sem duplicar" e "Desinstalar com a extensão" da spec `user-service`, com a lista vazia (`@as []`), com outras extensões e com o UUID já presente

## 6. Documentação e integração

- [x] 6.1 Atualizar o `README.md` (a extensão, o aviso de sair e entrar, como ver os erros dela com `journalctl --user -b /usr/bin/gnome-shell`, e os scripts de `scripts/dev/`), o `docs/architecture.md` (a extensão em camadas, sem ativação do daemon, e a equivalência com o St do que já existe) e o `docs/roadmap.md` (a etapa 3 da fase 2 dividida em três changes). Verificar que os comandos citados existem e rodam como escritos
- [x] 6.2 Verificar tudo: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `sh extension/tests/run.sh`, `sh scripts/dev/headless-smoke.sh` e `sh packaging/tests/install-test.sh`. Depois, com o aval do usuário e no terminal dele: `packaging/install.sh`, sair e entrar na sessão, e conferir a temperatura do líquido na barra, o painel com a contagem de aparelhos, `systemctl --user stop halod.service` deixando o ícone esmaecido sem o daemon voltar sozinho, e "Iniciar halod" trazendo-o de volta. Nenhum comando é enviado aos aparelhos: a extensão só lê propriedades e a lista do `halod`

## Workflow follow-up

- Sincronizar as specs `gnome-panel` e `user-service`, arquivar a change e commitar, quando o usuário pedir.
- Depois: `gnome-panel-led` (carrossel, desenho dos aparelhos, controles do LED, fila de `Apply`, religar) e `gnome-panel-extras` (perfis, detalhes, erros, linha de status, preferências e fonte CPU).
