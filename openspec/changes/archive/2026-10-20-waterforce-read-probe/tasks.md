# Tasks

## 1. Telemetria (`waterforce-telemetry`)

- [x] 1.1 Implementar a busca do hwmon `waterforce` ligado ao HID `1044:7A4D` e a leitura de `fan1/2_input`, `pwm1/2` e `temp1_input`, com testes sobre uma árvore sysfs falsa (presente, sem driver, sem aparelho); verificar com `cargo test -p halo-core`
- [x] 1.2 Implementar `halo telemetry waterforce [--watch N]` (mínimo 2 s, conversão para °C e %), com teste da formatação (29000 → 29,0 °C, 224 → 88%) e da recusa de `--watch 1`; verificar com `cargo test`
- [x] 1.3 Executar `halo telemetry waterforce` na máquina (nenhum comando ao aparelho: só sysfs) e verificar que os valores batem com `cat /sys/class/hwmon/hwmon*/fan1_input` do hwmon `waterforce`

## 2. Regra udev (`device-permissions`)

- [x] 2.1 Criar `packaging/udev/70-halo.rules` com as duas linhas do design e documentar instalação, recarga, remoção e verificação (`getfacl`) em `docs/permissions.md`; verificar com `udevadm verify packaging/udev/70-halo.rules` (ou `udevadm test` se `verify` não existir)
- [x] 2.2 Pedir ao usuário que instale a regra com sudo e verificar com `getfacl` no hidraw do `1044:7a4d` que o usuário tem `rw`

## 3. Probe do Castor3 (`waterforce-probe`)

- [x] 3.1 Adicionar ao `Hidraw` `write_output_report()` e `read_input_report(timeout)` com `poll(2)`, com teste de timeout usando um pipe no lugar do hidraw; verificar com `cargo test`
- [x] 3.2 Implementar o montador `99 + payload + zeros` (6144 bytes), só para `[0xD6]` (fonte: `get_firmware_ver_cmd` do `gigabyte_waterforce.c`) e `[0xDE, 0x00]` (fonte: construtor do `GigabyteCastor3Controller.cpp`), com testes dos bytes exatos; verificar com `cargo test`
- [x] 3.3 Implementar a leitura filtrada pelo comando (descarta `DA` e outros, prazo de 2 s) e a decodificação das duas versões de firmware, com testes (`99 D6 02 05` → 25, `99 DE 02` → 2, `DA` antes de `D6`, sem resposta); verificar com `cargo test`
- [x] 3.4 Implementar `halo probe waterforce [--raw]`, com a mensagem de permissão apontando para `packaging/udev/70-halo.rules` e códigos de saída iguais aos do `probe it5702`; verificar com teste de integração sobre um sysfs falso e com `--help`
- [x] 3.5 Verificar por revisão de código que os únicos pacotes enviados ao Castor3 são `99 D6` e `99 DE 00`

## 4. Execução real e documentação

- [x] 4.1 Executar `halo probe waterforce --raw` (envia só `99 D6` do kernel e `99 DE 00` do OpenRGB), com o usuário observando o anel e a tela, e conferir que nada mudou
  - Executado em 2026-10-20. Nada mudou no anel, na tela, na fan nem na bomba. Respostas: `99 d6 01 06` e `99 de 00`.
- [x] 4.2 Atualizar `docs/hardware.md` com a telemetria, as respostas `D6`/`DE`, a conclusão sobre a compatibilidade com o protocolo de anel do OpenRGB e a dependência atual do `60-openrgb.rules`; atualizar `docs/roadmap.md` e o `README.md`; verificar que os comandos documentados rodam como escritos

## Workflow follow-up

- Arquivar a change com `/opsx:archive` depois da revisão.
- Próximas changes possíveis: cor do anel (se o `DE` responder como no X II), análise do GIGABYTE Control Center com o REA para fan/bomba, RGB das fans e LCD.
