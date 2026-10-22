# Tasks

## 1. Aparelhos no halo-core

- [x] 1.1 Criar o módulo `g502` no `halo-core` com `VENDOR_ID` (`0x046D`), `PRODUCT_ID` (`0xC08B`), `USAGE_PAGE` (`0xFF00`) e `DEVICE`, citando a origem (`LogitechControllerDetect.cpp`, `LOGITECH_G502_HERO_PID`, interface 1, usage page `0xFF00`, commit `72262b8`). Sem nenhuma função que abra o hidraw. Verificar com um teste de `find_devices` sobre um sysfs falso com as duas interfaces do mouse (`0x0001` e `0xFF00`), em que só a `0xFF00` é encontrada

## 2. Lista de aparelhos no daemon

- [x] 2.1 Trocar o `DeviceEntry` por um `struct Device` (`id`, `name`, `present`, `devnode`, `capabilities`) e a tabela de `ticks.rs` pela da decisão 2 (`waterforce`, `it5702`, `g502`, com o `DeviceMatch`, as capacidades e a origem do nome). Ler o nome da placa de `<sys_root>/class/dmi/id/board_name`, com o padrão `Placa-mãe (IT5702)` (decisão 3). Verificar com testes unitários sobre um sysfs falso os cenários "Waterforce conectado", "Aparelho desconectado", "G502 detectado sem ser aberto" e "Placa-mãe pelo nome DMI" da spec `daemon-service`, e o nome padrão sem o arquivo DMI
- [x] 2.2 Expor `Manager.Devices()` e o sinal `DevicesChanged` como `aa{sv}`, com as chaves e os tipos da decisão 1, convertidos só na borda do D-Bus. Verificar com testes de integração no `dbus-daemon` privado que a lista tem as cinco chaves com os tipos certos em cada aparelho, que um aparelho ausente tem `present` falso e `devnode` vazio, e que a ordem é `waterforce`, `it5702`, `g502`
- [x] 2.3 Atualizar os testes de `DevicesChanged` para o formato novo e acrescentar o cenário "Nada mudou". Verificar que o sinal sai quando o devnode muda e não sai em dois ciclos iguais

## 3. Nome do Waterforce pelo modelo

- [x] 3.1 Fazer o `WaterforceBackend` devolver o modelo nas operações que leem o `DE 00` (`apply_led`, `speed_status`, `set_presets`, `set_curve`, `restore` e `apply_state`), a partir de `led::Applied.model` e do `Snapshot.model` que o `halo-core` já devolve, e o backend falso devolver um modelo configurável. Verificar com `cargo test -p halo-daemon` que os testes existentes continuam passando
- [x] 3.2 Criar o `KnownModel` compartilhado entre o worker e os ticks e passar o `Sender` do `refresh` ao worker (decisão 4). Depois de uma operação que deu certo com um modelo da tabela de `model_name`, o worker grava o modelo e manda `try_send(())`. O tick monta o nome com `AORUS ` mais o nome do modelo, e `AORUS Waterforce X` sem modelo. Verificar com testes de integração sobre o backend falso os cenários "Daemon recém-iniciado" (sem nenhuma chamada ao backend), "Nome muda depois da primeira operação" (o sinal com `AORUS Waterforce X 240` depois de um `Apply`), "Leitura de velocidade revela o modelo" (modelo 2 vira `AORUS Waterforce X 360`) e "Modelo desconhecido" (o nome não muda)

## 4. Painel e documentação

- [x] 4.1 Ajustar o `docs/ui/panel-prototype.html` ao formato novo: o `fakeDaemon.Devices()` devolve os dicionários, o painel filtra os presentes, usa o `name` do daemon e mostra os controles quando há `led`. Acrescentar ao painel de cenários o G502 ausente e a troca de nome do Waterforce. Verificar no navegador os cenários "Lista do daemon", "Um aparelho só", "Aparelho desconectado", "Nome muda" e "Placa-mãe detectada" da spec `gnome-panel`
- [x] 4.2 Atualizar o `README.md` (exemplo de `busctl` e a saída nova do `Devices`), o `docs/architecture.md` (a interface `Manager` com `aa{sv}`, as capacidades e o nome do Waterforce) e o `docs/ui/README.md` (os cenários novos). Verificar que o exemplo de `busctl` funciona como escrito com o `halod` num `dbus-daemon` privado e um sysfs falso
- [x] 4.3 Registrar em `docs/hardware.md`, na seção do G502, que o `hidraw1` tem a coleção de topo na usage page `0xFF00` (lido do `report_descriptor` no sysfs, sem abrir o hidraw) e que é a interface do driver Logitech do OpenRGB. Verificar que a seção cita a origem

## 5. Integração

- [x] 5.1 Verificar a integração inteira: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` e `cargo test` no workspace, sem erros
- [x] 5.2 Com o aval do usuário, conferir no PC real: iniciar o `halod` e ler `Devices` com `busctl --user` (só sysfs: o Waterforce como `AORUS Waterforce X`, a placa como `B550M AORUS ELITE` e o G502 em `/dev/hidraw1`, nenhum hidraw aberto). Depois, um `Waterforce.Speed.Status`, que envia só as leituras `99 DE 00`, `99 DD`, `99 D9 01` e `99 D9 02` (`DeviceApi`, `ucCooler.dll`, ver `docs/protocols/castor3.md`), e conferir que o `DevicesChanged` traz `AORUS Waterforce X 240`. Nenhuma escrita no aparelho

## Workflow follow-up

- Sincronizar as specs `daemon-service` e `gnome-panel`, arquivar a change e commitar, quando o usuário pedir.
