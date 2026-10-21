# Tasks

## 1. Núcleo de velocidade (`halo-core`, sem aparelho)

- [x] 1.1 Criar `crates/halo-core/src/waterforce/speed.rs` (SPDX GPL-2.0-or-later, citando as fontes `DeviceApi.SetSpeedMode`, `SetSpeedCurve`, `Save` e `FanControl.OnClickApply` no doc do módulo) e registrá-lo em `waterforce/mod.rs`; verificar com `cargo build`
- [x] 1.2 Tipos dos presets da fan (Default 2, Quiet 6, Balanced 0, Performance 5, Turbo 4) e da bomba (Balanced 0, Turbo 4), com nomes em minúsculas para o CLI e recusa dos outros modos; verificar com testes unitários de conversão e recusa (ZeroRPM, Customized, FixedRPM, Quiet na bomba)
- [x] 1.3 Montagem dos payloads `E5 <tipo> <modo> 00`, `E6 00 <tipo>` + 4 pontos big-endian e `B6`, e da sequência na ordem do `OnClickApply`; verificar com testes que conferem os bytes dos cenários da spec (`99 E5 01 06 00`, `99 E5 02 04 00`, `99 E6 00 01 00 03 E8 1E 03 E8 32 05 DC 50 09 C4`, `99 B6`)
- [x] 1.4 Regra do lado não informado: repetir o valor bruto do `DD` e recusar 3 ou mais de 7; verificar com testes dos cenários "Só a bomba informada" e "Lado atual desconhecido"
- [x] 1.5 Tabelas de referência do GCC para o `status` (`FanApi.dcX240`, `FanApi.dcX280_360` e `PumpApi.dcX240_280_360`, extraídas de `extracted/ilspy-uccooler/ucCooler.Api/`), escolhidas pelo modelo do `DE`; verificar com um teste que confere a tabela Performance do X 240 (2000/2000/2200/2500)

## 2. Snapshot

- [x] 2.1 Formato `chave=valor` `version=1` (design, decisão 4), com serialização e parser estrito; verificar com testes de ida e volta e de recusa (arquivo truncado, chave faltando, modo 3, modo 8, curva com temperatura decrescente, RPM acima de 3200)
- [x] 2.2 Caminhos `$XDG_STATE_HOME/halo/waterforce-speed.{previous,original}` com fallback `~/.local/state/halo/`, criação do diretório e gravação atômica (temporário + `rename`); o original só é gravado se não existir; verificar com testes em `tempfile` para a primeira mudança, as mudanças seguintes e um diretório sem permissão de escrita

## 3. Escrita e restauração no aparelho (`halo-core`)

- [x] 3.1 `set_to`/`restore_to` sobre os traits `InputReports`/`OutputReports`: `DE 00` → `DD`, `D9 01`, `D9 02` → gravar o snapshot → `E5` fan, `E5` bomba, `E6` quando houver → `B6` (opcional pelo `--no-save` no `set`), com 1 ms entre as escritas; verificar com testes de um aparelho falso que conferem a sequência exata do cenário "Fan em Quiet com a bomba em Turbo", que nada é escrito com o modelo desconhecido, que nada é escrito sem resposta ao `D9 02` e que nada é escrito se a gravação do snapshot falhar
- [x] 3.2 Funções `status`, `set` e `restore` que abrem o hidraw por VID:PID e usage page (`find_hidraw`), com erros de aparelho ausente e de permissão iguais aos do `led`; verificar com testes de `find_hidraw` num sysfs falso sem o aparelho

## 4. CLI

- [x] 4.1 `halo speed waterforce status`, `set --fan <modo> --pump <modo> [--no-save] [--dry-run]` e `restore [--original]` em `crates/halo-cli/src/main.rs`, com os códigos de saída já existentes (2, 3, 4 e 5); verificar com `cargo run -- speed waterforce set --help`
- [x] 4.2 Saída do `status` com nome e valor bruto dos modos, o slot Customized rotulado como ativo ou não ativo e a tabela do GCC rotulada como referência; verificar com um teste de formatação para `DD` = `05 04` e outro para a fan em Customized
- [x] 4.3 `--dry-run` exige os dois lados e mostra os pacotes sem abrir o aparelho e sem gravar snapshot; verificar com um teste em `crates/halo-cli/tests/speed.rs` (com `--sys-root` vazio e `XDG_STATE_HOME` num diretório temporário que deve continuar vazio)
- [x] 4.4 Recusas antes de abrir o aparelho (`--fan zero-rpm`, `--pump quiet`, nenhum lado, `restore` sem snapshot); verificar com testes em `crates/halo-cli/tests/speed.rs`
- [x] 4.5 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` passando

## 5. Validação no aparelho

Cada passo é observado com `halo telemetry waterforce --watch 2` em outro terminal, com o RPM e a temperatura anotados antes e depois.

- [x] 5.1 `halo speed waterforce status`: envia só `DE 00`, `DD`, `D9 01`, `D9 02` (`DeviceApi.GetDeviceModelName`, `GetMode`, `GetSpeedCurve`); verificar que a saída mostra Performance/Turbo e que `$XDG_STATE_HOME/halo/` continua sem snapshot
- [x] 5.2 Experimento do `B6`: `halo speed waterforce set --fan quiet --no-save`, que envia `E5 01 06 00` e `E5 02 04 00` (`DeviceApi.SetSpeedMode`), sem `B6`; verificar no `--watch` se o RPM da fan cai em até ~30 s e o que o `status` (`DD`) mostra depois; conferir que os dois snapshots foram gravados
- [x] 5.3 Mesmo preset com `B6`: `halo speed waterforce set --fan quiet`, que envia `E5 01 06 00`, `E5 02 04 00` e `B6` (`SetSpeedMode` + `DeviceApi.Save`); verificar a queda do RPM da fan e o `DD` = `06 04`
- [x] 5.4 Percorrer os presets da fan (`default`, `balanced`, `performance`, `turbo`) e da bomba (`balanced`), um por vez, com `E5` + `B6` (mesmas fontes); verificar e anotar o RPM estável de cada um na temperatura do momento
- [x] 5.5 Confirmar que o `D9` é o slot Customized: depois de cada preset, `status` mostra o `D9 01`/`D9 02` sem mudança, e o RPM medido segue o preset e não o `D9`; verificar comparando as anotações de 5.4 com o `D9`
- [x] 5.6 `halo speed waterforce restore` duas vezes, que envia `E5` + `B6` com os modos do snapshot (`SetSpeedMode`, `Save`); verificar que alterna entre os dois últimos estados no `status` e no RPM
- [x] 5.7 `halo speed waterforce restore --original`; verificar a fan em Performance e a bomba em Turbo (`DD` = `05 04`), o RPM de volta a ~2270/~3120 na mesma faixa de temperatura e o arquivo `.original` sem mudança (`sha256sum` antes e depois)
- [x] 5.8 Persistência: com o estado restaurado, desligar e ligar o PC (ou reconectar o aparelho) e rodar `status`; verificar se o modo continua `05 04`. Se o 5.2 mostrou efeito sem `B6`, repetir o `--no-save` antes do desligamento para ver se ele é perdido

## 6. Documentação

- [x] 6.1 Atualizar `docs/hardware.md` com os comandos enviados nesta change, o resultado do experimento do `B6` (5.2, 5.3 e 5.8), o RPM observado em cada preset (5.4) e a confirmação, ou não, de que o `D9` é o slot Customized (5.5); verificar que cada afirmação diz se foi confirmada no aparelho ou inferida
- [x] 6.2 Atualizar `docs/protocols/castor3.md` (status de `E5`/`B6` "confirmado no aparelho", nota sobre o `D9` ser o slot Customized e tabela `FanApi.dcX280_360`) e `docs/roadmap.md` (item 4 da Fase 3 dividido em `waterforce-speed-mode`, concluído, e `waterforce-speed-curve`); verificar com `git diff --stat docs/`
- [x] 6.3 Documentar os comandos `halo speed waterforce` e o local dos snapshots no `README.md`; verificar que os exemplos do README rodam com `--dry-run`

## Workflow follow-up

- Sincronizar a spec `waterforce-speed` e arquivar a change depois da revisão, quando o usuário pedir.
- A `waterforce-speed-curve` deve estender a capability `waterforce-speed` e reaproveitar o snapshot desta change.
