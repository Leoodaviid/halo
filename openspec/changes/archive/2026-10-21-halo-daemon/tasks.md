# Tasks

## 1. Acesso exclusivo ao hidraw (core e CLI)

- [x] 1.1 Em `crates/halo-core/src/hid/hidraw.rs`, adicionar `OpenError::Busy(PathBuf)` e tomar em `Hidraw::open` um `flock(LOCK_EX | LOCK_NB)`, tentando de novo a cada 50 ms até 5 s (design, Decisão 1). Deixar o prazo injetável numa função interna para os testes. Verificar com testes unitários sobre um arquivo comum: um segundo `open` com o primeiro aberto devolve `Busy` depois do prazo curto; com o primeiro fechado no meio da espera, o segundo abre; e dois arquivos diferentes abrem sem esperar
- [x] 1.2 Na CLI, mapear `OpenError::Busy` para o código 5 com a mensagem "ocupado por outro processo do halo" e o caminho, sem a dica da regra udev, em `speed`, `led` e nos dois probes. Verificar com testes unitários do mapa de erros e das mensagens, no estilo dos de `PermissionDenied` em `main.rs` (o devnode da descoberta é sempre `/dev/<nome>`, então o caminho ocupado de ponta a ponta fica para a 1.3)
- [x] 1.3 Com o aval do usuário, conferir o `flock` no `/dev/hidrawN` real do Waterforce sem enviar nada: segurar o lock com `flock /dev/hidrawN sleep 10` e rodar `halo probe waterforce` ao mesmo tempo. Verificar que o probe só envia pedidos depois de 10 s, ou que termina com código 5 com a mensagem de ocupado. Esse teste não envia nenhum comando novo: o probe só manda as leituras `D6`, `DE 00`, `DD`, `D9 01`, `D9 02`, `C7` e `E8` (`docs/protocols/castor3.md`)
- [x] 1.4 Documentar no `README.md` (código 5 também para aparelho ocupado) e no `docs/architecture.md` (o lock por aparelho e por que ele fica no nó do hidraw). Verificar que `cargo fmt --check`, `cargo clippy --all-targets` e `cargo test` passam

## 2. Velocidade para um estado desejado (core)

- [x] 2.1 Em `waterforce/speed.rs`, criar o tipo do estado desejado (cada lado opcional, preset ou RPM de curva plana) e `apply_state_to(device, store, desired)`, com a leitura, a comparação, a recusa do lado ausente em modo não repetível e a escrita por `write_to` com `Snapshot::target()` (design, Decisão 4). Os comandos são os da restauração: `E5`, `E6` e `B6` do `ucCooler.dll` do GCC 26.09.10.01 (`DeviceApi.SetSpeedMode`, `DeviceApi.SetSpeedCurve`, `DeviceApi.Save`), sem nenhum comando novo. Verificar com testes que usam o `FakeDevice` e cobrem os cenários "Velocidade já igual" (só as 4 leituras e nenhum snapshot), "Fan em curva e bomba em preset" (os 4 pacotes byte a byte) e "Lado ausente em FixedRPM" da spec `profiles`, e um lado em RPM cuja curva no `D9` é de outro RPM, que tem de gravar
- [x] 2.2 Expor a função com abertura (`apply_state(sys_root, store, desired)`), como as outras do módulo, e um resultado que distinga "sem mudança" de "escrito". Verificar com um teste de aparelho ausente num sysfs falso e com `cargo test`

## 3. Esqueleto do daemon

- [x] 3.1 Criar `crates/halo-daemon` (binário `halod`, GPL-2.0-or-later, membro do workspace) com as dependências `zbus` 5, `serde` com `derive`, `toml`, `async-channel`, `async-io`, `futures-util` e `clap` (design, Decisões 2 e 8), e os argumentos ocultos `--sys-root` e `--address`. Verificar que `cargo build -p halo-daemon` passa e que `halod --help` não mostra as opções ocultas
- [x] 3.2 Conectar ao bus (da sessão ou do `--address`), pedir `io.github.leoodaviid.Halo1` com `DoNotQueue` e sair com código 1 e mensagem se não for `PrimaryOwner`. Publicar `/io/github/leoodaviid/Halo1` com `Manager.Version` e definir os erros `io.github.leoodaviid.Halo1.Error.*` da spec `daemon-service`, com o mapa do design (Decisão 6). Verificar com um teste de integração que sobe um `dbus-daemon --session` privado, inicia um `halod`, lê `Version` e confere que um segundo `halod` sai com código diferente de zero sem tirar o nome do primeiro

## 4. Aparelhos, laço periódico e resume

- [x] 4.1 Implementar `Manager.Devices()` e o sinal `DevicesChanged` só com o sysfs (`find_devices` com o `DeviceMatch` do IT5702 e do Waterforce), e o laço de 2 s com os ticks de presença e telemetria (design, Decisão 7). Verificar com testes que usam um sysfs falso: a lista com um aparelho presente e outro ausente, o sinal emitido quando o devnode muda de `hidraw3` para `hidraw5` e não emitido quando nada muda
- [x] 4.2 Assinar o `PrepareForSleep` do logind numa conexão com o bus do sistema e rodar os ticks na hora com `false`. Sem bus do sistema, registrar o aviso e continuar. Verificar com um teste que inicia o `halod` com um endereço de sistema inválido e confere que o D-Bus da sessão continua respondendo, e com um teste unitário de que o tratamento do `false` chama os ticks
- [x] 4.3 Verificar a regra "Nada é enviado ao hardware sem pedido": um teste que monta o daemon com o backend falso e um sysfs falso com os dois aparelhos presentes, roda a inicialização e três ciclos de ticks (e um resume simulado) e confere que o backend não recebeu nenhuma chamada. Revisar que os ticks só usam `find_devices` e `telemetry::read`, que não abrem hidraw

## 5. Interface do Waterforce

- [x] 5.1 Criar o worker do Waterforce com a fila e o trait `WaterforceBackend`, a implementação real sobre o `halo-core` e um backend falso para os testes (design, Decisão 3). Verificar com um teste unitário de que duas tarefas enviadas em sequência são executadas na ordem e uma de cada vez, e de que uma chamada a `Manager.Devices()` responde enquanto o backend falso está bloqueado numa tarefa
- [x] 5.2 Publicar `/io/github/leoodaviid/Halo1/waterforce` com `Waterforce.Led` (`Apply` e `LastApplied`), validando antes de enfileirar com os tipos do core. Verificar com testes, sobre o backend falso, os cenários "Efeito com paleta", "Efeito desconhecido", "Depois de aplicar" e "Aplicação que falhou" da spec `daemon-waterforce`, e com um teste do backend real com sysfs falso que confere `NotFound` e `PermissionDenied`
- [x] 5.3 Publicar `Waterforce.Speed` (`Status`, `SetPresets`, `SetFlatCurve` e `Restore`), usando o `SnapshotStore` de `speed::state_dir()`, o mesmo da CLI, e sempre com `B6`. Verificar com testes, sobre o backend falso, os cenários "Status lido", "Só a fan", "Nenhum lado", "RPM fora da faixa" e "Sem snapshot" da spec, e que `SetPresets` e `SetFlatCurve` chamam o core com `save = true`
- [x] 5.4 Publicar `Waterforce.Telemetry` com as propriedades da Decisão 6, alimentadas pelo tick de telemetria, e `PropertiesChanged` só nas que mudaram. Verificar com testes sobre um hwmon falso os cenários "Líquido fora da faixa", "Driver não carregado" e "Valores iguais" da spec

## 6. Perfis

- [x] 6.1 Criar `profiles.rs` com o formato TOML da Decisão 5 (`deny_unknown_fields`), a validação pelos tipos do core, a regra de nome e a gravação atômica (arquivo temporário, `fsync` e `rename`) em `$XDG_CONFIG_HOME/halo/profiles/`, e o `active-profile` em `$XDG_STATE_HOME/halo/`. Verificar com testes unitários os cenários "Perfil salvo", "Nome com barra", "Perfil com as duas seções", "Velocidade com RPM", "RPM fora da faixa" e "Arquivo editado à mão com erro" da spec `profiles`, e um teste em que o `rename` falha (diretório sem escrita) e o arquivo anterior continua inteiro
- [x] 6.2 Publicar `Profiles` (`List`, `Get`, `Save`, `Delete`, `Activate` e `ActiveProfile`), com o perfil como `a{sv}`. `Activate` aplica primeiro a velocidade (`apply_state` da task 2.2) e depois o LED pelo worker, tenta todas as seções e só grava o ativo se todas derem certo. Verificar com testes, sobre o backend falso, os cenários "Apagar o perfil ativo", "Ler perfil inexistente", "Perfil com LED", "LED falha e velocidade dá certo" e "Tudo certo", e um teste de ida e volta `Save` → `Get` com as duas seções
- [x] 6.3 Com o aval do usuário, um teste de ponta a ponta no aparelho: iniciar o `halod`, salvar com `busctl --user` um perfil com o preset que já está no aparelho e `cycle` no LED, e ativá-lo. Comandos enviados: `99 DE 00`, `99 DD`, `99 D9 01` e `99 D9 02` (velocidade igual, sem escrita, `ucCooler.dll`), e a sequência do `cycle` com `99 DE 00` e um `99 B6` (`GcLedLib.dll`, igual a `halo led waterforce cycle`). Verificar que o `halo speed waterforce status` continua igual, que o snapshot anterior não mudou e que o anel está em ciclo. Se o aparelho mostrar algo novo, registrar em `docs/hardware.md`

## 7. Documentação e integração

- [x] 7.1 Atualizar o `README.md` (o `halod`, como iniciar, exemplos de `busctl --user` para LED, preset, telemetria e perfis, e o formato do perfil), o `docs/architecture.md` (o nome do bus, os objetos e interfaces, o worker por aparelho e os ticks) e o `docs/roadmap.md` (Fase 2.1 concluída, com o `systemd-user-service` e o `waterforce-cpu-feed` como próximos). Verificar que cada exemplo de `busctl` do README funciona com o `halod` rodando num `dbus-daemon` privado e um sysfs falso, até o erro `NotFound` esperado
- [x] 7.2 Verificar a integração inteira: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` no workspace, sem erros

## Workflow follow-up

- Sincronizar as specs `device-exclusive-access`, `daemon-service`, `daemon-waterforce` e `profiles`, arquivar a change e commitar, quando o usuário pedir.
