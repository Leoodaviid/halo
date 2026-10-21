# halo

Controle de RGB nativo para Linux, feito para o hardware deste PC. A ideia é cobrir o que o RGB Fusion 2.0 e o GIGABYTE Control Center fazem no Windows, com uma interface própria e minimalista: um ícone na barra superior do GNOME que abre um painel com cores, zonas, efeitos e perfis.

Não é um clone da interface da Gigabyte nem uma camada sobre o OpenRGB. O halo fala direto com cada controlador. O código do OpenRGB serve só como referência de protocolo.

## Estado

Fase de análise concluída em 2026-10-20. Já existem ferramentas que só leem, sem alterar nada, e o controle de LED do Waterforce (ver abaixo):

```sh
cargo run -q -p halo-cli -- probe it5702 --raw          # placa-mãe
cargo run -q -p halo-cli -- telemetry waterforce         # fan, bomba e líquido
cargo run -q -p halo-cli -- probe waterforce --raw      # leituras do Waterforce
```

O probe do Waterforce precisa da regra udev do halo (ver [docs/permissions.md](docs/permissions.md)). Ele envia só os pedidos de leitura `D6`, `DE 00`, `DD`, `D9 01`, `D9 02`, `C7` e `E8` (ver [docs/protocols/castor3.md](docs/protocols/castor3.md)). Exemplo de saída, com valores ilustrativos:

```text
Dispositivo:        /dev/hidraw2
Nome HID:           ITE Tech. Castor3
Firmware (D6):       1.6
Modelo (DE):         Waterforce X 240 [0]
Modos (DD):          fan Default [2], bomba Balanced [3]
Curva da fan (D9):   0 °C 1200 RPM, 30 °C 1200 RPM, 50 °C 1400 RPM, 65 °C 1800 RPM
Curva da bomba (D9): sem resposta em 2 s
Fans RGB (C7):       2
LCD (E8):            modo 7
```

O número entre colchetes é o byte bruto. O modo 3 (FixedRPM) aparece como Balanced, como no GCC. Com `--raw`, a saída mostra também os primeiros 64 bytes de cada resposta, com o pedido no título (`Resposta D9 01:`). Se algum pedido ficar sem resposta, o comando termina com código 5.

### LED do Waterforce

`halo led waterforce <efeito>` aplica cor e efeito ao anel e às fans RGB do Waterforce, com os comandos da `GcLedLib.dll` do GIGABYTE Control Center (ver [docs/protocols/castor3.md](docs/protocols/castor3.md#anel-e-rgb-das-fans)). Antes de escrever, ele confere o modelo com `DE 00`. Cada execução manda uma sequência só, terminada por um único `B6`.

```sh
halo led waterforce static --color ff6600                    # cor estática
halo led waterforce cycle                                     # ciclo de cores
halo led waterforce tri-color --color ff0000 --color 00ff00 --color 0000ff
halo led waterforce pulse --color 00ffff --brightness 5 --speed 2
halo led waterforce off                                       # desliga o anel e as fans
halo led waterforce static --color ff6600 --dry-run           # só mostra os pacotes
```

- Efeitos: `static`, `pulse`, `flash` e `double-flash` (uma cor, padrão `ff6600`), `gradient` (uma cor, padrão `ff0000`), `cycle`, `wave` e `rainbow` (sem cor), `color-shift`, `tri-color`, `spin`, `switch`, `lego` e `aurora` (paleta de até 8 cores, completada com a última), e `off`.
- Sem `--color`, valem as cores padrão do GCC para o efeito.
- `--brightness` vai de 1 a 10 e `--speed` de 1 a 5, com padrão 10 e 5.
- `--dry-run` mostra os pacotes em hexadecimal sem procurar nem abrir o aparelho:

```text
99 C9 01 64 0A 02 01
99 CD FF 66 00
99 B6
```

O protocolo não tem leitura do estado do LED, então o modo de fábrica não pode ser restaurado com exatidão. `cycle` e `rainbow` são os mais próximos. Códigos de saída: 2 para argumento inválido, 3 para aparelho não encontrado, 4 para falta de permissão (instale a regra udev) e 5 para falha de comunicação ou modelo desconhecido.

Precisa do Rust estável (rustup). Licença: GPL-2.0-or-later, porque a lógica de protocolo é portada do OpenRGB.

Veja:

- [docs/hardware.md](docs/hardware.md): inventário do hardware, protocolos e permissões.
- [docs/architecture.md](docs/architecture.md): arquitetura proposta.
- [docs/roadmap.md](docs/roadmap.md): fases e próximos passos.

## Hardware alvo

| Dispositivo | Controlador | Situação |
|---|---|---|
| Placa-mãe Gigabyte B550M AORUS ELITE | ITE IT5702 (`048d:5702`), USB HID | Protocolo conhecido, acesso liberado |
| AORUS Waterforce X 240, telemetria | Driver `gigabyte_waterforce` do kernel (hwmon) | Funciona, só leitura |
| AORUS Waterforce X 240, anel e fans RGB | Castor3 (`1044:7a4d`), USB HID | Funciona: `halo led waterforce`, protocolo do GCC confirmado no aparelho |
| AORUS Waterforce X 240, tela LCD | Castor3 (`1044:7a4d`), USB HID | Protocolo analisado no GCC, implementação pendente |
| Mouse Logitech G502 SE HERO | `046d:c08b`, USB HID | Protocolo conhecido |
| Teclado Redragon edição Horda (só LED vermelho) | Sino Wealth (`258a:013b`), USB HID | Protocolo desconhecido e de alto risco, investigar com REA |
| RAM Kingston Fury DDR4 (`KF3200C16D4/16GX`) | SMBus, endereço `0x58` | Adiado: SMBus bloqueado pelo ACPI |
