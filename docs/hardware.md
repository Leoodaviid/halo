# Análise de hardware

Levantamento feito em 2026-10-20, só com leitura do sistema (sysfs, `lsusb`, `lspci`, descritores HID, log do kernel) e do código-fonte do OpenRGB. Nenhum comando foi enviado a nenhum dispositivo.

## Sistema

| Item | Valor |
|---|---|
| Placa-mãe | Gigabyte B550M AORUS ELITE |
| Sistema | Ubuntu 26.04, kernel 7.0.0-38 |
| Desktop | GNOME Shell 50.1, Wayland (extensões já em uso: blur-my-shell, user-theme) |
| GPU | AMD Radeon RX 6600 (Navi 23, `1002:73ff`, subsistema XFX `1eae:6505`), sem RGB controlável conhecido |
| Ferramentas | `i2c-tools` instalado. OpenRGB 1.1 existe como AppImage em `~/Downloads`, não é usado pelo halo |

## Placa-mãe: ITE IT5702

| Item | Valor |
|---|---|
| USB | `048d:5702`, "ITE Tech. Inc. ITE Device", bcdDevice `0001` |
| hidraw | `/dev/hidraw5` (o número pode mudar entre boots; identificar pelo VID:PID) |
| Permissão | `crw-rw----+`, ACL dá `rw` ao usuário `bumblebee`. Não precisa de root, mas só por causa do `60-openrgb.rules` (ver "Regras udev") |

Descritor HID (bruto):

```text
0689ff0910a101855a0901150026ff0075089510b100c0
0689ff09cca10185cc0901150026ff007508953fb100c0
```

Decodificado: página de uso de fabricante `0xFF89`, com dois feature reports:

- `0x5A`, 16 bytes.
- `0xCC`, 63 bytes + ID. É por ele que todos os comandos são enviados (pacotes de 64 bytes).

Protocolo, segundo o driver do OpenRGB:

| Comando | Função |
|---|---|
| `CC 60` + get feature report | Lê a configuração do controlador (struct `IT8297Report`: produto, firmware, contagem de LEDs ARGB, chip id, calibração) |
| `CC 61` | Calibração extra (só IT5711) |
| `CC 33` | Grava calibração |
| `CC 20`–`CC 27` | Efeito por zona (`0x20` + índice do LED). As fitas ARGB usam cabeçalhos próprios (`HDR_D_LED*_ARGB`) |
| `CC 28` | Aplica os efeitos às zonas selecionadas |
| `CC 31`, `CC 32`, `CC 47`, `CC 48`, `CC 5E` | Liga/desliga recursos (ver o código para o significado de cada um) |

Zonas da B550M AORUS ELITE (layout `it5702_24_device`, `layout_id` `0x0030005F`). O `layout_id` não identifica a placa sozinho, porque outras placas usam o mesmo valor (B550M DS3H R2, por exemplo). O OpenRGB escolhe a entrada da tabela pelo nome DMI da placa (`/sys/class/dmi/id/board_name` = `B550M AORUS ELITE`) e pelo `device_num` lido no `CC 60`:

| Zona | Tipo |
|---|---|
| `D_LED1` | Fita endereçável (ARGB) |
| `D_LED2` | Fita endereçável (ARGB) |
| `LED_C1` | Cor única |
| `LED_CPU` | Cor única |
| `LED_C2` | Cor única |

O OpenRGB já detectou a placa por USB em 2026-10-17, então o comando de leitura `CC 60` já foi enviado a ela sem problemas.

### Leitura real com `halo probe it5702` (2026-10-20)

Comando enviado: só `CC 60` + 62 zeros (set feature report de 64 bytes), seguido de get feature report `0xCC`. Nenhuma fita ARGB nem fita de 12 V está ligada à placa, então não há luz para conferir. O Waterforce (USB próprio) e a RAM (SMBus) não passam por este controlador.

```text
00: cc 01 00 07 02 00 0a 00 00 00 00 00 49 54 35 37
10: 30 32 2d 47 49 47 41 42 59 54 45 20 56 32 2e 30
20: 2e 31 30 2e 30 00 00 00 00 01 02 00 02 00 01 00
30: 02 00 01 00 00 01 02 00 00 01 02 57 00 00 00 00
```

| Campo (`IT8297Report`) | Valor | Observação |
|---|---|---|
| `product` | `0x01` | |
| `device_num` | `0` | Bate com a entrada `B550M AORUS ELITE` do OpenRGB (`device_num` 0) |
| `strip_detect` | `0x07` | Significado não documentado no OpenRGB |
| `fw_ver` | `2.0.10.0` | |
| `argb01/23/45_count` | `0x00` | Todas as fitas no limite de 32 LEDs (`LEDS_32`) |
| `support_cmd_flag` | `0x00` | Bit `0x02` (LampArray) desligado: o OpenRGB não chamaria `EnableLampArray` |
| `str_product` | `IT5702-GIGABYTE V2.0.10.0` | |
| `cal_spare0` | `0x00020100` | |
| `cal_strip0` | `0x00010002` | Bytes `02 00 01 00`, que é `GRB` na tabela `GigabyteCalibrationsLookup` |
| `cal_strip1` | `0x00010002` | `GRB` |
| `rgb_cali` | `0x00020100` | Bytes `00 01 02 00`, que é `BGR` |
| `chip_id` | `0x57020100` | |
| `cal_spare1` | `0x00000000` | |

As cores das fitas de 12 V provavelmente usam a ordem `BGR` (`rgb_cali`) e as ARGB a ordem `GRB` (`cal_strip0/1`). A tabela do OpenRGB sugere isso, mas não foi confirmado na prática porque não há nada conectado.

## AORUS Waterforce X 240 (Castor3)

| Item | Valor |
|---|---|
| USB | `1044:7a4d` (VID da Holtek, usado pela Gigabyte nos periféricos AORUS), "ITE Tech. Castor3", bcdDevice `0200` |
| hidraw | `/dev/hidraw2` |
| Permissão | `crw-------` sem regra. Com a `70-halo.rules`, `rw` para o usuário da sessão |
| Modelo | Waterforce X **240**, primeira geração (tabela de dispositivos do GCC; o usuário tinha informado X II 240) |

Descritor HID: página de uso `0x0000`, uso `0x0002`, report ID `0x99`, entrada de 255 bytes e saída de 6143 bytes.

O OpenRGB tem um driver para esse controlador (`GigabyteCastor3Controller`), mas registrado para a versão 360, com `0414:7a5e`. O descritor do nosso aparelho é idêntico ao que o driver espera (mesmo usage page/usage, report ID e tamanhos), então o protocolo deve ser o mesmo e só o VID:PID muda. Por isso o OpenRGB não o reconhece. A diferença provavelmente vem do tamanho (240 x 360), mas isso não está confirmado.

Protocolo do anel RGB, segundo o driver:

```text
c9 [estilo] [velocidade*20] [brilho] [b4] [b5]   efeito
cd [R] [G] [B]                                    cor única
b0 [estilo] [RGB1] [RGB2] 00                      paleta, cores 1 e 2
b1 / b2 / b3                                      paleta, cores 3 a 8
b6                                                aplica
```

- Estilos: `01` estático, `02` pulso, `03` ciclo, `04` flash, `05` flash duplo, `06` gradiente, `07` transição, `08` onda, `0A` arco-íris, `0B` três cores, `0C` giro, `0D` alternância.
- Velocidade de 1 a 5 (no fio vai `velocidade * 20`), brilho de 1 a 10.

A tela LCD não está implementada no OpenRGB. O GCC envia arquivos para a flash do aparelho em pedaços de 6141 bytes (ver [protocols/castor3.md](protocols/castor3.md)).

### Driver do kernel e telemetria

O Linux tem o driver `gigabyte_waterforce` (`drivers/hwmon/gigabyte_waterforce.c`, GPL-2.0+), registrado para `1044:7a4d` como *"Gigabyte AORUS WATERFORCE X240, X280 and X360"*, ou seja, a primeira geração, e não a X II. Ele está carregado e ligado ao nosso aparelho. Abre o HID com `HID_CONNECT_HIDRAW`, então o hidraw continua disponível.

Ele expõe um hwmon `waterforce` (hoje `hwmon6`, só leitura para todos). Valores lidos em 2026-10-20:

| Arquivo | Valor | Significado |
|---|---|---|
| `fan1_input` | 2204 | Fan, RPM |
| `fan2_input` | 3030 | Bomba, RPM |
| `pwm1` | 224 | Duty da fan (0–255) |
| `pwm2` | 255 | Duty da bomba |
| `temp1_input` | 29000 | Líquido, m°C |

Protocolo do kernel (output report `0x99` de 6144 bytes, input report filtrado por `data[0] == 0x99` e `data[1] == comando`):

| Pedido | Resposta |
|---|---|
| `99 DA` | Status: `data[2..4]` fan RPM (LE), `data[5..7]` bomba RPM (LE), `data[8]` duty fan, `data[9]` duty bomba, `data[0x0D]` temperatura °C |
| `99 D6` | Firmware: `data[2] * 10 + data[3]` |

O driver não controla velocidade: o duty é só leitura. O protocolo de velocidade está no GCC (`99 E5`, `99 E6`, `99 B6`; ver abaixo).

**Modelo confirmado: Waterforce X (primeira geração).** A tabela de dispositivos do próprio GIGABYTE Control Center 26.09.10.01 (`Device.ini` e o INI do módulo `GBT_Cooler_26.08.24.01`) registra `[1044_7A4D] ModelName=GP-AORUS WATERFORCE X`. O X II é só `[0414_7A5E] ModelName=GP-AORUS WATERFORCE X II`, sem variante 240 com outro PID. O GCC ainda suporta o `1044:7A4D`: o `ucCooler.dll` carrega `VID_1044&PID_7A4D`. Análise completa na change `waterforce-protocol-analysis`.

### Probe de firmware com `halo probe waterforce` (2026-10-20)

Comandos enviados: só `99 D6` (do kernel) e `99 DE 00` (do OpenRGB), cada um como output report de 6144 bytes completado com zeros. Nada mudou no anel, na tela, na fan nem na bomba.

| Pedido | Resposta (primeiros bytes; o resto é zero) | Interpretação |
|---|---|---|
| `99 D6` | `99 d6 01 06` | Firmware 16 no formato do kernel (`1 * 10 + 6`), provavelmente 1.6 |
| `99 DE 00` | `99 de 00` | Modelo 0 = **X 240**, segundo o GCC (`DeviceApi.GetDeviceModelName`). O OpenRGB lê o byte como versão |

O `D6` dá firmware **1.6**, e o `DE` identifica o modelo como X 240. Os comandos de anel do OpenRGB (`c9`/`cd`/`b0`–`b3`/`b6`) são os mesmos que a `GcLedLib.dll` do GCC envia para o 7A4D, mas o OpenRGB troca os bytes 2 e 3 do `c9`: no GCC, o byte 2 é o brilho e o byte 3 é a velocidade. Vale o layout do GCC (ver [protocols/castor3.md](protocols/castor3.md#anel-e-rgb-das-fans)).

### Probe com as leituras do GCC (2026-10-20)

`halo probe waterforce --raw`, change `waterforce-led`. Comandos enviados, um de cada vez e cada um esperando a própria resposta: `99 D6` (kernel), `99 DE 00`, `99 DD`, `99 D9 01`, `99 D9 02`, `99 C7` e `99 E8` (`DeviceApi`/`LcdApi` do `ucCooler.dll`). Todos responderam com o próprio comando no byte 1. Nada mudou no anel, na tela, na fan nem na bomba.

| Pedido | Resposta (primeiros bytes; o resto é zero) | Interpretação |
|---|---|---|
| `99 D6` | `99 d6 01 06` | Firmware 1.6 |
| `99 DE 00` | `99 de 00` | Modelo 0 = Waterforce X 240 |
| `99 DD` | `99 dd 05 04` | Fan em Performance (5), bomba em Turbo (4) |
| `99 D9 01` | `99 d9 01 01 00 03 e8 1e 03 e8 32 05 dc 50 09 c4` | Curva da fan: 0 °C 1000, 30 °C 1000, 50 °C 1500, 80 °C 2500 RPM |
| `99 D9 02` | `99 d9 02 02 00 0a f0 1e 0a f0 32 0a f0 50 0a f0` | Curva da bomba: 2800 RPM em 0, 30, 50 e 80 °C |
| `99 C7` | `99 c7 02` | 2 fans RGB |
| `99 E8` | `99 e8 04` | LCD no modo 3 (`r[2] - 1`), temperatura segundo o `ShowMode` (inferido) |

Observações:
- O `D9` repete o tipo em `r[2]` e em `r[3]` (`01 01` para a fan, `02 02` para a bomba). O GCC não confere esses bytes, e o halo também não.
- As curvas guardadas no aparelho não são as tabelas da interface do GCC: o último ponto é 80 °C, e não 65 °C, e os RPM da fan não batem com o Performance do `FanApi.dcX240` (2000/2000/2200/2500). Isso confirma que a interface mostra tabelas próprias quando o modo não é Customized, e que o firmware tem as dele.
- `C7` = 2 bate com o X 240 (duas fans de 120 mm). O halo não envia `C8`.

O firmware 1.6 entende os pedidos de leitura do GCC. Com isso, a change segue para o comando de LED.

### Protocolo segundo o GIGABYTE Control Center (2026-10-20)

Análise estática do `ucCooler.dll` e do `AorusLcdService.exe` (GCC 26.09.10.01, decompilados com o ilspycmd e triados com o REA). Detalhes, fontes e grau de confiança em [protocols/castor3.md](protocols/castor3.md). Desses, os pedidos de leitura da seção anterior, os comandos de LED e o `E5`/`B6` de velocidade (seções abaixo) foram enviados ao aparelho.

| Função | Comandos | Situação |
|---|---|---|
| Identificação e status | `DE` modelo, `D6` firmware, `DA` RPM, `DD` modo, `D9` curva, `C7` nº de fans RGB, `C2` progresso de gravação | confirmados no código; `DE`, `D6`, `DD`, `D9`, `C7` e `E8` também no aparelho |
| Velocidade | `E5 <tipo> <modo>`, `E6` curva de 4 pontos, `B6` salvar | confirmados no código; `E5` e `B6` também no aparelho (ver abaixo); `E6` nunca enviado |
| LCD 320×320 | `E7`/`E8` modo, `E2`/`DF` itens, `E4`/`EA` cores, `FB`/`FC` carrossel, `F1`/`F2` envio de arquivo (JPEG, PNG ou MP4 H.264), `FA` espaço livre, `F0`/`F3`–`F6`/`FD` lista de mídia, `FE` apagar, `E0`/`E1` dados de CPU | confirmados no código; formato do arquivo inferido |
| Anel e RGB das fans | `C9 <efeito> <brilho×10> <velocidade×2> 02 01`, `CD R G B`, `B0`–`B3` paleta, `B6` aplicar; `C8 <n>` nº de fans | confirmados na `GcLedLib.dll` (Ghidra); `C9`, `CD`, `B0`–`B3` e `B6` também no aparelho, com anel e fans no mesmo canal (ver abaixo); `C8` nunca enviado |

### RGB das fans

As fans do Waterforce têm RGB e vinham num modo de cores variadas de fábrica. Nenhuma fita RGB está ligada à placa-mãe, então o RGB das fans é controlado pelo próprio Waterforce. No GCC, as fans e o anel recebem a mesma configuração (uma região só, `nArea = 1`). A única coisa separada é a quantidade de fans RGB (`99 C8 <n>`, de 1 a 8), que aparece só no 7A4D. O aparelho informa 2 (`C7`), e o halo não envia `C8`.

### Primeiro controle de LED com `halo led waterforce` (2026-10-20)

Change `waterforce-led`, com o aval do usuário a cada execução. Cada uma mandou `99 DE 00` (resposta `99 de 00`, X 240), a sequência do efeito e um único `99 B6`. O resultado foi conferido olhando o cooler:

| Comando | Pacotes depois do `DE 00` | Resultado |
|---|---|---|
| `static --color ff6600` | `99 C9 01 64 0A 02 01`, `99 CD FF 66 00`, `99 B6` | Anel e fans laranja, **juntos** |
| `cycle` | `99 C9 03 64 0A 02 01`, `99 B6` | **Igual ao modo de fábrica** |
| `rainbow` | `99 C9 0A 64 0A 02 01`, `99 B6` | Arco-íris, diferente do `cycle` |
| `tri-color --color ff0000 --color 00ff00 --color 0000ff` | `99 C9 0B 64 0A 02 01`, `99 B0 0B FF 00 00 00 FF 00`, `99 B1`/`B2`/`B3 0B 00 00 FF 00 00 FF`, `99 B6` | Vermelho, verde e azul |
| `pulse --speed 2` | `99 C9 02 64 04 02 01`, `99 CD FF 66 00`, `99 B6` | Laranja pulsando devagar |
| `cycle` | igual ao acima | De volta ao modo de fábrica |

Conclusões:
- O anel e as fans usam o mesmo canal, como o GCC sugeria.
- O modo de fábrica é o `cycle` (efeito 03, brilho 10, velocidade 5). Para voltar a ele: `halo led waterforce cycle`.
- O layout do GCC vale neste firmware: o byte 3 do `C9` é a velocidade (o `04` deixou o pulso lento), e não o brilho, como no OpenRGB.
- Nenhum erro de escrita. Foram 6 `B6` no total nesta sessão.

### Velocidade com `halo speed waterforce` (2026-10-20)

Change `waterforce-speed-mode`, com o aval do usuário a cada escrita. Todos os `set` e `restore` mandaram antes `99 DE 00`, `99 DD`, `99 D9 01` e `99 D9 02` e gravaram o snapshot em `~/.local/state/halo/`. Comandos de escrita enviados: só `99 E5 01 <modo> 00`, `99 E5 02 <modo> 00` e `99 B6` (`DeviceApi.SetSpeedMode` e `Save`). Nenhum `E6`. O RPM e o duty vêm do hwmon (`halo telemetry waterforce --watch 3`), observados por ~30 s depois de cada envio. O líquido ficou em 25 °C o tempo todo.

| Modo | `DD` | Duty | RPM observado | Tabela do GCC (30 °C) |
|---|---|---|---|---|
| Fan Quiet (6) | `06 ..` | 143 (56%) | 1221–1617 | 900 |
| Fan Balanced (0) | `00 ..` | 184 (72%) | 1849–2046 | 1500 |
| Fan Performance (5) | `05 ..` | 224 (88%) | 2013–2541, em geral ~2250 | 2000 |
| Fan Turbo (4) | `04 ..` | 255 (100%) | 2508–2640 | 2500 |
| Fan Default (2) | `02 ..` | 255 (100%) | ~2574 | 1200 |
| Bomba Turbo (4) | `.. 04` | 255 (100%) | 3030–3299 | 3150 |
| Bomba Balanced (0) | `.. 00` | 207 (81%) | 2620–2700 | 2600 |

Resultados, todos **confirmados no aparelho**:
- O `E5` vale **sem o `B6`**. Com `set --fan quiet --no-save` (só os dois `E5`), o duty da fan caiu de 224 para 143 já na primeira leitura, e o `DD` passou a `06 04`.
- O `B6` **persiste o modo**. Com Performance/Turbo gravado por `E5` + `B6`, foi enviado Quiet só com `E5`. Depois de desligar o PC com corte de energia, o `DD` voltou a `05 04`, e a fan voltou a duty 224. O modo enviado sem `B6` vale até desligar e não fica na memória do aparelho.
- O `D9` é o **slot Customized** guardado, e não a curva ativa. O `D9 01` (0/30/50/80 °C: 1000/1000/1500/2500) e o `D9 02` (2800 ×4) não mudaram depois de nenhum preset, e nenhum RPM medido ficou perto de 1000.
- Cada preset tem o próprio duty, e a bomba aceita Balanced e Turbo.
- O `restore` alternou entre os dois últimos estados (Turbo/Balanced ↔ Turbo/Turbo), e o `restore --original` voltou a Performance/Turbo sem mudar o arquivo `.original` (sha256 igual antes e depois).
- Foram 9 `B6` de velocidade nesta sessão, sem erro de escrita.

Observações ainda **não explicadas**:
- O **Default** foi para duty 255, igual ao Turbo, e não para os ~1200 RPM da tabela do GCC. Não se sabe se o firmware trata o Default como "sem controle" ou se usa outra referência.
- As tabelas da interface do GCC não são as do firmware. Quiet e Balanced ficaram uns 400 a 500 RPM acima delas a 25 °C.
- Se o `B6` grava na flash a cada chamada. Ele persiste o modo, mas o desgaste por chamada não é conhecido. O halo continua sem enviá-lo em laço.

Estado deixado no aparelho: fan Performance, bomba Turbo (`05 04`), o mesmo de antes da change.

### Curva Customized com `halo speed waterforce curve` (2026-10-20)

Change `waterforce-speed-curve`, com o aval do usuário a cada escrita. Cada `curve` mandou antes `99 DE 00`, `99 DD`, `99 D9 01` e `99 D9 02` e gravou o snapshot. Comandos de escrita enviados: `99 E5 01 01 00`, `99 E5 02 04 00` e `99 E6 00 01 …` (`DeviceApi.SetSpeedMode` e `SetSpeedCurve`), todos com `--no-save` (sem `B6`), e no fim `restore --original` (`99 E5 01 05 00`, `99 E5 02 04 00`, `99 B6`). O RPM e o duty vêm do hwmon, lidos a cada 3 s.

| Passo | Curva da fan (°C:RPM) | `DD` | `D9 01` depois | Líquido | Duty da fan | RPM |
|---|---|---|---|---|---|---|
| Antes | (Performance) | `05 04` | 0:1000, 30:1000, 50:1500, 80:2500 | 26 °C | 224 (88%) | ~2232 |
| Experimento (T = 26) | 0:1500, 27:1500, 29:2500, 100:2500 | `01 04` | igual à enviada | 25–26 °C | 255 (100%), por 54 s | 2475–2838 |
| Curva plana | 0:1500, 30:1500, 50:1500, 80:1500 | `01 04` | igual à enviada | 25–26 °C | 153 (60%), em ~3 s | 1530–1716 |
| `restore --original` | (Performance) | `05 04` | 1500 ×4 (não restaurado) | 26 °C | 224 (88%) | 2209–2508 |

Resultados **confirmados no aparelho**:
- O firmware aceita o `E6` e segue a curva: a curva plana de 1500 RPM levou a fan a duty 153, que é 1500/2500 × 255.
- O `E6` vale **sem o `B6`**, na hora. O `D9` mostra a curva enviada sem `B6`. Ainda não se sabe se ela continua depois de desligar, como acontece com o `E5`.
- A temperatura de referência da curva **não é a do líquido**. Com o líquido em 25–26 °C, abaixo do ponto de 27 °C, a curva em degraus foi para o ponto de 2500 RPM e ficou lá. A referência estava, então, acima de 29 °C. O hwmon lê o líquido do próprio `DA` do aparelho, então não é diferença de sensor.
- O `restore` não volta o slot de um lado que estava num preset. Depois do `restore --original` com `B6`, o `D9 01` ficou com `1500 ×4`, e não com o slot de fábrica. O slot está inativo enquanto a fan estiver em Performance. O slot de fábrica tem degraus, então o `curve`, que só grava curvas planas, não o recria. Os caminhos são o `restore` de um snapshot com a fan em Customized e essa curva, ou o GCC no Windows. Com a fan num preset, o slot não tem efeito.

**Hipótese, não confirmada:** a referência é a temperatura da CPU, que só o `AorusLcdService` do Windows envia (`E0`/`E1`). No Linux, sem ninguém enviar, o firmware usaria um valor fixo perto de 50 °C. Ela bate com os presets: com duty = RPM/2500 × 255, os duties da tabela da speed-mode caem no ponto de 50 °C das tabelas do GCC (Quiet 143 → 1402, tabela 1400; Balanced 184 → 1804, tabela 1800; Performance 224 → 2196, tabela 2200; Turbo 255 → 2500). A bomba Balanced bate com 2600 na escala de 3150 (207 → 2557). O Default (255) é a exceção. Não se sabe se o valor é guardado do último envio do Windows ou se é um padrão do firmware.

Ainda não observado: o comportamento acima do último ponto e o efeito de uma carga na CPU, porque a fan já estava no máximo.

#### Bomba plana e temperatura errada no `DA` (2026-10-20, ~23:38–23:52)

Task 5.1, com o aval do usuário: `halo speed waterforce curve pump 2600 --no-save` (`99 E5 01 05 00`, `99 E5 02 01 00`, `99 E6 00 02 00 0A 28 1E 0A 28 32 0A 28 50 0A 28`, sem `B6`).

- A bomba seguiu a curva: `DD` = `05 01`, `D9 02` = `2600 ×4`, duty 207 (81%), o mesmo do Balanced, e 2593–2700 RPM, estável desde a primeira leitura. A fan não mudou (Performance, duty 224).
- **Anomalia, ainda não explicada:** cerca de 33 s depois do envio, o `temp1_input` foi de 25 °C para **224 °C** e às vezes **240 °C**. O `restore` (`99 E5 01 05 00`, `99 E5 02 04 00`, `99 B6`) voltou a bomba ao Turbo, mas a leitura continuou errada. Nenhum outro processo tinha o hidraw aberto, e o kernel não registrou nada. O firmware não reagiu ao valor: a fan continuou em duty 224.
- **Efeito colateral:** o `B6` do `restore` gravou o slot que estava no aparelho, e o `D9 02` ficou `2600 ×4`. O slot de fábrica (`2800 ×4`) foi sobrescrito. Ele é uma curva plana válida e pode ser recriado com `curve pump 2800` seguido de um `restore`, ou de um preset com `set`. O `D9 01` também está com `1500 ×4`, desde o experimento da fan.

Captura passiva do `99 DA` com `scripts/diag/waterforce_da_sniff.py`. O script abre o hidraw só para leitura, e quem pede o `DA` é o driver do kernel. Nenhum byte foi enviado ao aparelho. Estado no momento: fan Performance e bomba Turbo, depois do `restore`.

```
hora     temp1   Tctl | b2..13 do DA
23:45:35  224000   ?  | d0 08 00 b5 0c 00 58 64 61 2f 01 e0
23:45:40  240000   ?  | dc 08 00 d6 0b 00 58 64 68 2f 01 f0
23:45:47  240000   ?  | d8 08 00 30 0c 00 58 64 7b 38 01 f0
23:46:46  240000  32.4 | .. .. .. .. .. .. 58 64 65 2f 01 f0
23:46:49  240000  32.4 | .. .. .. .. .. .. 58 64 65 38 01 f0
23:46:51  224000  32.4 | .. .. .. .. .. .. 58 64 51 38 01 e0
23:47:06  240000  32.9 | .. .. .. .. .. .. 58 64 e8 38 01 f0
23:47:11  240000  32.8 | .. .. .. .. .. .. 58 64 a6 26 01 f0
23:51:36  224000  33.4 | d7 08 00 de 0c 00 58 64 75 2f 01 e0
23:51:44  224000  33.2 | e6 08 00 d6 0b 00 58 64 e8 2f 01 e0
```

| Byte | Valor | Leitura |
|---|---|---|
| 2–4 | ~2250 LE | RPM da fan, correto |
| 5–7 | ~3030–3290 LE | RPM da bomba, correto |
| 8 | `58` (88) | duty da fan em % (o `pwm1` 224 é 88% de 255) |
| 9 | `64` (100) | duty da bomba em % |
| 10 | 79 a 232, sem padrão | desconhecido |
| 11 | `26`, `2f`, `38` (38, 47, 56) | desconhecido |
| 12 | `01` | desconhecido |
| 13 | `e0` / `f0` | o que o driver lê como temperatura (`data[0x0D]`) |

Observações: nenhum dos bytes 10 a 13 acompanha o Tctl da CPU (k10temp, 32–34 °C). O byte 13 sempre termina em `0` em hexa, o que parece um código de erro ou um sensor saturado, e não uma temperatura. É hipótese. O GCC lê do `DA` só o RPM (`DeviceApi.GetRPMSpeed`), e o `AorusLcdService` não usa a temperatura do líquido (`CoolerData.LiqiudTemperature => 0`). Quem chama o byte 13 de temperatura é só o driver do kernel. O firmware do Castor3 não vem no pacote do GCC.

**Captura de referência, depois do corte de energia (2026-10-21, 19:07–19:08).** O PC foi desligado com corte de energia e ligado às 18:29. Mesmo script, só leitura, nenhum byte enviado. Estado: fan Performance e bomba Turbo (o que o `restore` deixou), CPU em repouso.

```
hora     temp1   Tctl | b2..13 do DA
19:07:41   31000  44.0 | bd 08 00 ae 0c 00 58 64 8e 38 01 1f
19:07:46   31000  42.9 | f9 08 00 30 0c 00 58 64 6f 38 01 1f
19:07:51   31000  43.4 | ee 08 00 ab 0c 00 58 64 95 38 01 1f
19:07:56   31000  43.0 | d1 08 00 d6 0b 00 58 64 4d 2f 01 1f
19:08:01   31000  44.2 | e5 08 00 a3 0c 00 58 64 b2 38 01 1f
19:08:04   31000  45.9 | e5 08 00 c0 0c 00 58 64 b2 00 00 1f
19:08:06   31000  45.4 | cc 09 00 30 0c 00 58 64 4c 38 01 1f
19:08:11   31000  44.2 | ce 08 00 89 0c 00 58 64 d0 38 01 1f
```

- O byte 13 voltou a uma temperatura plausível: `1f` = 31 °C, estável por 14 leituras, enquanto o Tctl variava de 42,8 a 45,9 °C. Ou seja, ele não segue a CPU, e o corte de energia desfez o erro: o firmware (ou o sensor) estava travado, e não foi o valor gravado que mudou.
- Os bytes 8 a 12 se comportam como durante o erro: duty `58`/`64`, byte 10 sem padrão, byte 11 entre `2f` e `38`, byte 12 em `01`. Uma leitura teve `00 00` nos bytes 11 e 12. Nada disso muda com o erro, então o erro ficou só no byte 13.
- Ainda não se sabe o que travou o sensor. O travamento veio ~33 s depois da curva plana da bomba, mas uma única ocorrência não prova a causa. Se acontecer de novo, anotar o comando anterior e repetir o corte de energia.
- Os slots Customized continuam como ficaram (`D9 01` = `1500 ×4`, `D9 02` = `2600 ×4`). Não foram lidos de novo, porque o `D9` exige enviar um pedido ao aparelho.

**Investigação (2026-10-21), sem enviar nada ao aparelho.**
- **Driver do kernel**, decompilado com o REA (Ghidra 12.1.4) a partir do `gigabyte_waterforce.ko` do kernel 7.0.0-38, sha256 `76761217…dadf38` descomprimido. O `waterforce_raw_event` aceita qualquer input report `99 DA`, inclusive a resposta a um pedido de outro programa, e grava `temp = data[0x0D] × 1000` sem sinal e sem validação. Ele também lê o RPM como 16 bits (`data[2..3]`, `data[5..6]`) e o duty como `data[8]`/`data[9]` em %, convertido com `(d × 255 + 50) / 100`. O `waterforce_read` reaproveita o último status por 2000 jiffies. O `224000` vem direto do byte enviado pelo aparelho, e não de um erro do driver.
- **GCC**: nenhum programa da Gigabyte lê o byte 13. O `ucCooler` só lê o RPM do `DA` (`DeviceApi.GetRPMSpeed`, a cada 2 s pela `FanControl`). O `AorusLcdService` manda o byte do líquido no `E0` sempre como 0. A opção "temperatura do líquido" do LCD (`Display` bit `0x40`) faz o próprio firmware desenhar o valor. Ou seja, no Windows, um byte 13 errado não aparece em lugar nenhum, a não ser no LCD.
- **Envio do halo comparado ao do GCC (`FanControl.OnClickApply`, `CoolerApi.Send`)**: mesma ordem (`E5` da fan, `E5` da bomba, `E6`, `B6`), mesmo preenchimento até 6144 bytes, e as respostas às escritas também não são lidas. O GCC não espera entre os pacotes, e o halo espera 1 ms. No Windows, o `AorusLcdService` manda `E0` em laço sem pausa ao mesmo tempo que a tela pede o `DA` a cada 2 s. Então o firmware já recebe comandos de dois processos intercalados, e a concorrência entre o halo e o driver do kernel não é um caso novo.
- **Kernel**: o journal da sessão (boot `-1`, 2026-10-20 22:55–23:53) não tem nenhum erro de USB, de HID nem reset do Castor3.
- **Firmware**: nenhum bloco do instalador do GCC é firmware do Castor3. Os `block-064`/`065` (~49 MB) são o ffmpeg do LCD. Não há como ver no REA como o firmware calcula o byte 13.
- **Hipótese, não confirmada:** como byte com sinal, `e0` = −32 e `f0` = −16. Um termistor desconectado ou um ADC fora da faixa costuma dar uma leitura muito fria, o que encaixaria num valor-sentinela. O nibble baixo sempre em `0` aponta na mesma direção.
- **Efeito no controle:** nenhum. A curva não usa o líquido como referência (ver "Curva Customized com `halo speed waterforce curve`"), a fan ficou no duty do preset, e a bomba seguiu o RPM pedido o tempo todo. Só a leitura ficou errada.

**Como o halo trata o valor (change `waterforce-telemetry-sanity`, 2026-10-21).**
- **Critério:** temperatura do líquido acima de **100 °C** (`temp1_input` > `100000`) é inválida. O limite é a constante `COOLANT_MAX_PLAUSIBLE_MILLICELSIUS` em `crates/halo-core/src/waterforce/telemetry.rs`. O valor bruto continua disponível.
- **Por que 100 °C, e não o padrão `e0`/`f0`:** o padrão vem de uma hipótese (valor-sentinela) tirada de uma ocorrência só, e outro byte de erro (`c3`, por exemplo) passaria como 195 °C. O limite também não é 60 °C, a temperatura máxima de operação do líquido que a Asetek indica para AIOs (a Gigabyte não publica esse número para o Waterforce X). Um líquido entre 60 e 100 °C está fora da especificação, mas é possível, por exemplo com a bomba parada, e precisa aparecer como número. Acima de 100 °C, a água ferve, e o valor só pode ser erro do sensor.
- **O que o `halo telemetry waterforce` mostra:** `Líquido:  inválido (o aparelho informou 224 °C)` no stdout, no lugar da temperatura, como o `FAULT` do `sensors` (lm-sensors). Uma nota sobre o corte de energia vai para o stderr, uma vez por execução, mesmo com `--watch`. Fan e bomba continuam sendo mostradas, e o código de saída continua 0.
- **Limite:** um sensor travado num valor plausível (como `1f`) não é detectado.

## Mouse Logitech G502 SE HERO

`046d:c08b`, `/dev/hidraw0` e `/dev/hidraw1`, permissão liberada ao usuário. Suportado pelo driver Logitech do OpenRGB (`LOGITECH_G502_HERO_PID`). Detectado pelo OpenRGB em 2026-10-17.

As duas interfaces, lidas em 2026-10-22 do `report_descriptor` e do `uevent` no sysfs, sem abrir nenhum hidraw (`HID_NAME=Logitech G502 HERO Gaming Mouse` nas duas):

| hidraw | Interface | Coleções de topo (usage page) |
|---|---|---|
| `hidraw0` | `input0` | `0x0001` (o mouse comum), descritor de 67 bytes |
| `hidraw1` | `input1` | `0x0001` (teclado), `0x000C` (consumer), `0x0001` (system control), `0xFF00` e `0xFF00`, descritor de 151 bytes |

O `hidraw1` é a interface de fabricante que o driver Logitech do OpenRGB usa: `Controllers/LogitechController/LogitechControllerDetect.cpp`, `REGISTER_HID_DETECTOR_IPU("Logitech G502 HERO Gaming Mouse", DetectLogitechWired, LOGITECH_VID, LOGITECH_G502_HERO_PID, 1, 0xFF00, 2)` (interface 1, usage page `0xFF00`, commit `72262b8`). O `halod` encontra o mouse por `046d:c08b` com a usage page `0xFF00` (módulo `g502` do `halo-core`) e só lê o sysfs: nenhum comando foi enviado ao mouse até agora.

## Teclado Redragon (edição Horda, World of Warcraft)

| Item | Valor |
|---|---|
| Modelo | Redragon mecânico, edição Horda de World of Warcraft, switch Blizzard, ABNT2, EAN 6975763141694 |
| LED | Só vermelho. O controle possível é de efeito e brilho, não de cor |
| USB | `258a:013b` (chip Sino Wealth), "USB Keyboard", bcdDevice `2000` |
| hidraw | `/dev/hidraw3` (interface 0, teclado padrão) e `/dev/hidraw4` (interface 1) |
| Permissão | `crw-------`, só root |

A interface 1 tem, além de multimídia e mouse, uma página de uso de fabricante `0xFF00` com estes feature reports:

| Report ID | Tamanho |
|---|---|
| `0x05` | 5 bytes |
| `0x09` | 504 bytes |
| `0x0A` | 41 bytes |
| `0x0B` | 126 bytes |
| `0x0C` | 1920 bytes |

Isso indica que o teclado aceita configuração por software, provavelmente efeitos de luz e macros.

**Alto risco.** O OpenRGB desativou os drivers de teclado Sino Wealth com este aviso em `SinowealthControllerDetect.cpp`: *"Sinowealth keyboards are disabled due to VID/PID pairs being reused from Redragon keyboards, which ended up in bricking the latter"*. Ou seja, comandos de outros teclados Sino Wealth já inutilizaram teclados Redragon. Os drivers que existem usam um report `0x06` de 1032 bytes, que este teclado nem tem como feature report.

Não existe protocolo documentado para este modelo. O único caminho seguro é analisar o software oficial da Redragon com o REA e só enviar o que for confirmado lá. Prioridade baixa.

## Regras udev

Em `/etc/udev/rules.d/60-openrgb.rules` (instalado em 2026-10-17, junto com o OpenRGB) há `TAG+="uaccess"` para o IT5702 (`048d:5702`) e o mouse (`046d:c08b`). É por isso que eles abrem sem root. O mesmo arquivo dá `uaccess` a **todos** os `i2c-*`. Não há regra para o Castor3 (`1044:7a4d`) nem para o teclado.

O halo tem uma regra própria, [`packaging/udev/70-halo.rules`](../packaging/udev/70-halo.rules), só para o IT5702 e o Castor3. Ela foi instalada em 2026-10-20 e, desde então, o `/dev/hidraw` do Castor3 tem `rw` para o usuário. Ver [permissions.md](permissions.md).

## RAM Kingston Fury DDR4 (adiada)

| Item | Valor |
|---|---|
| Modelo | Kingston `KF3200C16D4/16GX` (fabricante JEDEC Bank 2, `0x98`) |
| Protocolo | `KingstonFuryDRAMController` do OpenRGB: DDR4, endereço base `0x58`, 10 LEDs por pente |
| Barramento | SMBus do AMD FCH (`1022:790b`, PCI `00:14.0`) |

O SMBus não está disponível: o módulo `i2c_piix4` carrega, mas o kernel registra `ACPI: OSL: Resource conflict; ACPI support missing from driver?` e não cria o barramento. Os únicos barramentos I2C existentes (`/dev/i2c-0` a `i2c-8`) são da GPU. O OpenRGB também não achou nenhuma memória.

Liberar exige o parâmetro de boot `acpi_enforce_resources=lax`, com o risco de o firmware usar o mesmo barramento ao mesmo tempo. Escrever no SMBus da RAM é a parte mais arriscada do projeto. Decisão adiada.

## Referências

- Código do OpenRGB clonado em `/mnt/workspace/tools/repos/openrgb` (commit `72262b8`). Arquivos relevantes em `Controllers/`:
  - `GigabyteRGBFusion2USBController/`: placa-mãe. Layout da B550M em `GigabyteFusion2USB_Devices.cpp`.
  - `GigabyteCastor3Controller/`: Waterforce X II.
  - `LogitechController/`: mouse.
  - `KingstonFuryDRAMController/`: RAM.
- O OpenRGB é GPL-2.0-or-later. Se código dele for portado, o halo também precisa ser GPL.
- Log antigo do OpenRGB: `~/.config/OpenRGB/logs/` (perfil real do usuário, `/home/bumblebee`).
