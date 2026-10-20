# Design

## Context

Estado observado em 2026-10-20, só com leitura do sistema:

- O Castor3 (`1044:7a4d`, `/dev/hidraw2`, `crw-------`) está ligado ao driver `gigabyte_waterforce` do kernel (`drivers/hwmon/gigabyte_waterforce.c`, GPL-2.0+). O driver abre o aparelho com `HID_CONNECT_HIDRAW`, então o hidraw continua disponível para o espaço de usuário.
- O hwmon `waterforce` (hoje `hwmon6`) tem `fan1_input` (fan), `fan2_input` (bomba), `pwm1`/`pwm2` (duty, só leitura) e `temp1_input` (líquido), todos `r--r--r--`.
- O acesso ao IT5702 e ao mouse vem de `/etc/udev/rules.d/60-openrgb.rules` (instalado em 2026-10-17), que também dá `uaccess` a todos os `i2c-*`. Não há regra para `1044:7a4d`.

Fontes de protocolo:
- Kernel: `get_status_cmd = {0x99, 0xDA}`, `get_firmware_ver_cmd = {0x99, 0xD6}`, `waterforce_write_expanded()` (output report de `MAX_REPORT_LENGTH = 6144` com zeros) e `waterforce_raw_event()` (filtra por `data[0] == 0x99 && data[1] == comando`; firmware = `data[2] * 10 + data[3]`).
- OpenRGB `GigabyteCastor3Controller.cpp`: o construtor envia o payload `{0xDE, 0x00}` com `SendPacket()` (report `0x99` + 6143 bytes, `hid_write`) e lê com `hid_read_timeout(..., 255, 2000)`. A versão é o byte seguinte ao `DE`.

## Goals / Non-Goals

**Goals:**
- Telemetria sem nenhum comando do halo.
- Saber se o firmware responde ao `DE 00` do OpenRGB como o X II 360 (`DE 02`), antes de qualquer change de cor.

**Non-Goals:**
- Escrever no anel, na LCD, na fan ou na bomba.
- Substituir o driver do kernel ou desligá-lo.
- Instalar a regra udev automaticamente (precisa de sudo, fica com o usuário).

## Decisions

### Telemetria pelo hwmon, não pelo hidraw

O kernel já faz o pedido `DA`, decodifica a resposta e guarda o resultado por 2 s (`STATUS_VALIDITY`). Ler o sysfs não exige permissão e não acrescenta nenhum comando nosso. *Alternativa:* enviar `DA` pelo hidraw. Rejeitada porque duplicaria o driver e exigiria a regra udev só para ler a temperatura.

O hwmon é encontrado varrendo `/sys/class/hwmon/*/name == "waterforce"` e conferindo que `device` aponta para um HID com `HID_ID` `1044:7A4D`. Para separar "sem driver" de "sem aparelho", a ausência do hwmon é cruzada com a descoberta de hidraw de `device-discovery`. O mesmo critério de raiz de sysfs injetável do `it5702-read-probe` vale para os testes.

### Transporte de output e input reports

Os ioctls de feature report do IT5702 não servem aqui. O Castor3 usa output report (`write(2)` de 6144 bytes no hidraw, com `0x99` no byte 0) e input report (`read(2)`, até 256 bytes com o report ID). O `Hidraw` ganha `write_output_report()` e `read_input_report(timeout)`, este último com `poll(2)`.

Como o kernel pode pedir status a qualquer momento (quando algo lê o hwmon), o leitor descarta input reports cujo `data[1]` não é o comando esperado até vencer o prazo de 2 s. A ordem é `D6` primeiro, porque o kernel já envia esse pedido ao carregar o driver e o aparelho já respondeu a ele, e depois `DE 00`.

### Montagem dos pacotes

`castor3::request(payload) -> Vec<u8>` devolve sempre 6144 bytes: `0x99` + payload + zeros. Os testes conferem `99 D6 00…` e `99 DE 00 00…`. Payloads diferentes de `[0xD6]` e `[0xDE, 0x00]` não têm construtor público nesta change.

### Regra udev

`packaging/udev/70-halo.rules`:

```text
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="048d", ATTRS{idProduct}=="5702", TAG+="uaccess"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1044", ATTRS{idProduct}=="7a4d", TAG+="uaccess"
```

`70-` fica antes de `73-seat-late.rules`. Só o subsistema `hidraw` (o OpenRGB usa `usb|hidraw`), porque o halo não abre o nó USB. Instalação: `sudo install -m 644 packaging/udev/70-halo.rules /etc/udev/rules.d/ && sudo udevadm control --reload && sudo udevadm trigger --subsystem-match=hidraw`.

### CLI

- `halo telemetry waterforce [--watch <segundos>]`, com mínimo de 2.
- `halo probe waterforce [--raw]`.

Os códigos de saída seguem os do `probe it5702` (3 não encontrado, 4 sem permissão, 5 falha). Uma resposta faltando também gera 5.

## Risks / Trade-offs

- [O aparelho não ser um X II e o `DE 00` não ter significado neste firmware] → o `DE 00` é um pedido de leitura no OpenRGB e não altera estado lá. Se não houver resposta `DE`, isso fica registrado em `docs/hardware.md` e nenhuma change de cor usa o protocolo do OpenRGB antes de uma análise com o REA.
- [Concorrência com o kernel no mesmo hidraw] → o kernel só envia `DA` quando o hwmon é lido, e ignora respostas `D6`/`DE` fora do probe dele. O filtro por comando cobre os dois lados. Durante o probe, nada no halo lê o hwmon.
- [Output report de 6144 bytes acima do limite do hidraw] → o `HID_MAX_BUFFER_SIZE` do kernel atual (16 KiB) é maior que isso, e o próprio kernel envia esse tamanho. Um `write` curto vira erro explícito.
- [A regra udev liberar mais do que o necessário] → uma linha por VID:PID, só `hidraw`, sem I2C.

## Migration Plan

O usuário instala a regra do halo. O `60-openrgb.rules` pode ficar enquanto o mouse depender dele. Remover a regra do OpenRGB é uma decisão à parte, documentada mas fora desta change.

## Open Questions

- As fans são ARGB ligadas ao próprio Waterforce? Como a placa não tem nada conectado, é provável. Isso não muda esta change, só o escopo da análise com o REA.
