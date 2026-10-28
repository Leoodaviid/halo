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

### LED pelo painel do GNOME (2026-10-23)

Change `gnome-panel-led`, com o aval do usuário, na sessão real depois do `packaging/install.sh`. O painel só chama `Led.Apply` do `halod`, que manda `99 DE 00`, a sequência do efeito e um único `99 B6`, os mesmos pacotes da CLI ([protocols/castor3.md](protocols/castor3.md), "Anel e RGB das fans"). Os pedidos foram contados com `scripts/dev/watch-led-applies.sh` (só leitura, `dbus-monitor` no D-Bus da sessão): uma linha por `Apply`, com os argumentos, e uma por `PropertiesChanged` do `LastApplied`, que só sai com sucesso.

| Hora | Gesto no painel | `Apply` (efeito, cores, brilho, velocidade) | `B6` |
|---|---|---|---|
| 19:33:31 | ligar pelo switch, com o `halod` recém-iniciado e sem perfil com LED | `cycle`, `[]`, 10, 5 | 1 |
| 19:33:59 | efeito Estático | `static`, `[ff6600]`, 10, 0 | 1 |
| 19:34:56 | clique no começo do brilho | `static`, `[ff6600]`, 1, 0 | 1 |
| 19:35:00 / 19:35:04 | dois arrastes do brilho, soltos em 9 e em 10 | `static`, brilho 9 e 10 | 1 cada |
| 19:35:16 / 19:35:23 | duas mudanças de cor | `static`, `[ff000c]` e `[ff0000]` | 1 cada |
| 19:35:33 | efeito Pulso | `pulse`, `[ff0000]`, 10, 5 | 1 |
| 19:35:48 | efeito Flash | `flash`, `[ff0000]`, 10, 5 | 1 |
| 19:35:58 | efeito Gradiente | `gradient`, `[ff0000]`, 10, 5 | 1 |
| 19:36:05 | desligar pelo switch | `off`, `[]`, 0, 0 | 1 |
| 19:36:12 | efeito Estático com o LED desligado (herda a cor e o brilho do último ligado) | `static`, `[ff0000]`, 10, 0 | 1 |
| 19:40:58 | efeito Ciclo | `cycle`, `[]`, 10, 5 | 1 |
| 19:41:48–19:42:08 | brilho 2, 8, 1 e 10, velocidade 2 e 5 | `cycle`, um pedido por gesto | 1 cada |

Conclusões:
- **Um `Apply` e um `B6` por gesto concluído**, nenhum durante os arrastes, nenhum ao abrir o painel e nenhum erro. Foram 19 `B6` na sessão inteira.
- **Vistos pela primeira vez no aparelho:** `flash` (efeito `04`), `gradient` (`06`) e o desligado (`off`, enviado como estático com brilho 0 e `CD 00 00 00`). O usuário conferiu o cooler e todos responderam como esperado; os pacotes são os de `LedSetting::packets()`.
- **Ainda não vistos:** `double-flash` (`05`), `wave` (`08`) e os de paleta que o painel não oferece (`07`, `0C` a `0F`).
- Não testados no aparelho, só no Shell headless: soltar o brilho no valor já aplicado (nenhum `Apply`) e religar pelo switch com um estado ligado conhecido (pede o último ligado).
- Estado final: `cycle`, brilho 10, velocidade 5 (o modo de fábrica).

### LED preservado após corte de energia (2026-10-25)

O usuário desligou o PC com o Waterforce em **estático branco**, aplicado pelo painel do halo, que usa `Led.Apply` e grava com `99 B6`. Não havia `halo led` no histórico nem `active-profile`. Houve corte de energia; no boot às **16:52**, o anel continuava branco. Assim como o modo de velocidade, o LED aplicado com `B6` sobrevive ao corte de energia.

O painel abriu com "estado desconhecido" e o switch desligado: o `LastApplied` existia só na memória do `halod`. Ligar o switch nesse estado pediria `cycle`. A change `led-state-persist` guarda o último aplicado pelo daemon para recuperar a apresentação, sem reaplicar no login. Esta observação confirma o estático branco com `B6`; não testa a persistência sem `B6` nem todos os efeitos.

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

### Pré-condições da proposta `waterforce-cpu-feed` (2026-10-25)

Estado: investigação estática; **nenhum experimento novo executado**. A proposta
depende das verificações abaixo antes do design. Não interpretar a hipótese de
50 °C como comportamento confirmado ou como garantia de segurança.

Fontes conferidas no decompilado
`/mnt/workspace/tools/samples/gigabyte-control-center/extracted/ilspy-aoruslcdservice/`:
`AorusLcdService.API.Cooler/CoolerDataApi.cs`, `GetCoolerData` (linhas 230–257),
`SendCpuName` e `SendCoolerData`; e
`AorusLcdService.API.CPU/CpuDataApi.cs`, `MakeCpuData`.
O ramo do 7A4D espera 20 s, prepara nome/fabricante/threads/núcleos, envia E1 e
repete aquisição e envio de E0 sem `Sleep` explícito. Isso não mede o período
efetivo: a aquisição também consome tempo. O `Sleep(1000)` de outros ramos não
justifica adotar 1 s para este aparelho. Layout dos pacotes em
[Dados de sistema](protocols/castor3.md#dados-de-sistema-aoruslcdservice).

| Questão obrigatória | Evidência existente | Verificação ainda pendente |
|---|---|---|
| A curva usa a temperatura enviada em E0? | A curva reage ao E6; o ensaio anterior excluiu o líquido como referência, mas não demonstrou E0. | Com curva de degraus documentada e aprovada, comparar poucos E0 de 30, 50 e 80 °C com duty e RPM do DA, mantendo os demais campos controlados. Se a hipótese falhar, parar esta change e reportar. |
| O que acontece quando E0 para, e em quanto tempo? | Não medido. | Observar a interrupção após valor alto, com registro temporal; retenção ou ausência de mudança durante uma janela finita não prova ausência de timeout. O resultado precisa orientar o failsafe para saída normal, travamento e morte do processo. |
| E0 grava flash? E1 é necessário? | O serviço separa nome e dados; o laço observado não envia B6. Isso não prova a implementação interna do firmware nem a necessidade de E1. | Conferir transporte e chamadas; comparar E0 com/sem E1 em condições iniciais conhecidas e registrar o limite da conclusão sobre persistência. |

Cada execução que escrever no aparelho depende de aval específico do usuário,
com sequência completa de comandos, duração, curva inicial, monitoramento e
restauração apresentados previamente. Não há aval registrado para novos envios
E0/E1 nesta investigação. O slot Customized registrado acima é plano; não serve
para discriminar temperaturas. Restaurar apenas o preset não restaura o slot
Customized sobrescrito, conforme o ensaio anterior.

O observador existente `scripts/diag/waterforce_da_sniff.py` abre hidraw somente
para leitura e registra Tctl e bytes 2–13 do DA; a leitura de hwmon provoca o
pedido DA pelo kernel. Não foi executado nesta investigação. Para o ensaio,
preservar saída bruta e horários de cada envio; duty da fan é `data[8]`, RPM da
fan é `data[2..4]` little-endian, conforme a seção do driver acima. O script
atual assume `temp1_input` do k10temp: conferir `temp*_label = Tctl` antes de
usar esse valor como referência.

Pontos de integração observados, ainda sem decisão de design:
`crates/halo-daemon/src/ticks.rs` faz leituras de sysfs a cada 2 s e não abre
hidraw; `worker.rs` concentra operações de backend; `bus.rs` publica telemetria.
`crates/halo-core/src/hid/hidraw.rs`, `Hidraw::open`, toma `flock` exclusivo
com prazo. O envio periódico terá de conviver com esse lock e com operações da
CLI; o período de telemetria existente não determina o período seguro de E0.

Complemento recebido: o aceite exige reação real à carga, curvas com degraus,
testes de ritmo/proteção com backend falso, log de temperatura enviada versus
DA, ensaio final de carga e `/cross-review` antes de implementar. Continuam
pendentes a definição exata da propriedade D-Bus e detalhes truncados; o painel
fica fora do escopo. O design permanece condicionado às medições acima.

Leitura local em 2026-10-25: `/proc/cpuinfo` identifica AMD Ryzen 5 5600G,
12 threads e 6 núcleos; o hwmon de nome `k10temp` tem `temp1_label = Tctl`.
Esses índices são apenas o estado observado, não caminhos fixos para o design.
O serviço `halod.service` está ativo. `stress-ng` não foi encontrado; a tentativa
autorizada de `sudo apt install stress-ng` não instalou o pacote porque o sudo
exigiu autenticação em terminal. Não foi iniciada carga na CPU.

Precisão adicional do decompilado: `SendCoolerData` chama o campo do byte 8 de
`VRAMTemperature`, enquanto a tabela de protocolo o chama de VRM. Não usar essa
divergência para escolher um sensor: o significado permanece pendente de
análise. `CoolerApi.Send` apenas completa o output report com zeros e chama
`HidDriver.WriteByte`; não acrescenta `B6`. Isso confirma a ausência de save
explícito nesse transporte, mas não prova que o firmware nunca grave E0.

Leitura autorizada do aparelho em 2026-10-25:
`target/debug/halo speed waterforce status`, executado fora do sandbox após
o sandbox não expor o hidraw. Resultado: modelo X 240 (`DE = 0`), fan
Performance (`5`), bomba Turbo (`4`); slot da fan 0/30/50/80 °C em 1500 RPM,
slot da bomba nas mesmas temperaturas em 2600 RPM. São slots inativos.
Enviados somente `DE 00`, `DD`, `D9 01` e `D9 02`; nenhuma configuração alterada.
O protocolo do primeiro ensaio está em
[`experimentos.md`](../openspec/changes/archive/2026-10-27-waterforce-cpu-feed/experimentos.md).

### E0 como referência da curva: confirmado no aparelho (2026-10-25)

Ensaio autorizado com E1 (`Ryzen 5 5600G`), E0 e curva temporária da fan
0:2200/30:2200/50:2350/80:2500; bomba sem alterações. Payloads, sequência,
proteção e limitações no protocolo acima; dados brutos em
[`experiment1.jsonl`](../openspec/changes/archive/2026-10-27-waterforce-cpu-feed/experiment1.jsonl).
Nenhum B6 enviado, nenhuma carga de CPU iniciada.

| E0 (°C) | Duty DA após reação | Primeiro DA com esse duty após envio | RPM no fim dos 12 s |
|---|---|---|---|
| 80 | 100 | 3,780 s | 2574 |
| 50 | 94 | 1,819 s | 2706 |
| 30 | 88 | 5,853 s | 2243 |
| 80 | 100 | 3,835 s | 2574 |

O Tctl real ficou em ~37,6–40,5 °C durante o ensaio. Os duties correspondem
exatamente aos alvos 2500/2350/2200 na escala de 2500 RPM: **E0 altera a
referência da curva Customized**, com E1 prévio e demais campos controlados.
O byte 8 bruto do DA está em escala compatível com porcentagem; os valores
de duty 0–255 das tabelas antigas vêm de pwm1 do hwmon. Os atrasos incluem
a leitura do DA (~2 s) e não estabelecem uma cadência segura de envio.
O RPM oscila e teve inércia: o patamar de 50 °C não mostrou redução de RPM
dentro dos 12 s, embora o duty tenha caído para 94.

O último DA antes de restaurar mostrou duty 100, **70,48 s após o último
E0 de 80 °C**, sem fallback observado. Não se demonstrou ausência de timeout
posterior, nem comportamento após último E0 frio. Esse segundo caso é o
próximo ensaio proposto, ainda sem autorização.

DE/DD/D9 confirmaram a restauração de modos e slots originais (Performance,
Turbo; 1500 ×4 e 2600 ×4). O duty final permaneceu 100: restaurar modo/slot
não desfaz a última temperatura enviada, mantida intencionalmente em 80 °C.
O ensaio confirma a hipótese de referência, mas não encerra as pré-condições
do design: E1 obrigatório, persistência do E0, proteção e cadência continuam
pendentes. O byte do líquido em zero não impediu a resposta da curva neste
ensaio; isso não prova ausência de outros efeitos.

### Interrupção após E0 frio (2026-10-25)

Ensaio 2 autorizado separadamente, com a mesma curva conservadora, sem E1
novo, B6 ou carga de CPU. Dados em
[`experiment2.jsonl`](../openspec/changes/archive/2026-10-27-waterforce-cpu-feed/experiment2.jsonl).
O E1 do ensaio anterior continuava sendo uma condição inicial conhecida;
portanto isso não testa sua necessidade após inicialização limpa.

O E0 de 30 °C levou o duty a 88 em 4,06 s até o primeiro DA correspondente.
O último DA antes do E0 alto, **70,62 s após o envio frio**, ainda mostrava
duty 88 e fan 2297 RPM, com Tctl de 40,75 °C. Depois da reação inicial,
todos os DA dessa janela mantiveram duty 88: não houve fallback observado
para 50 °C nem para velocidade máxima. Não extrapolar para retenção infinita.

O E0 final de 80 °C levou a duty 100 no primeiro DA correspondente após
2,44 s. DE/DD/D9 conferiram a restauração de Performance/Turbo e dos slots
1500 ×4/2600 ×4. Tctl no ensaio: 40,125–43 °C; nenhum abort. Último E0
mantido alto, com duty final 100. A bomba não foi reconfigurada.

Consequência: não há timeout curto de segurança demonstrado. Enviar E0 alto
apenas no encerramento normal não protege contra morte abrupta antes desse
envio. É preciso resolver a proteção independente e o ciclo de vida fora da
sessão, ou restringir curvas por um piso cuja suficiência térmica seja
validada. O piso de 2200 RPM usado para observar o comportamento não recebeu
essa validação e não deve ser declarado seguro para qualquer carga.

Decisão posterior do usuário para `waterforce-cpu-feed`: não incluir serviço
de sistema/root. O design passa a usar proteção no SIGTERM, ExecStopPost com
comando independente, watchdog de progresso e Restart=on-failure na unidade
de usuário. Isso cobre falhas do principal enquanto o gerenciador, hardware e
permissões permitem executar a recuperação; não cobre a morte do gerenciador,
kernel travado ou garante entrega antes da suspensão. O protetor de sistema
foi descartado porque acrescentaria só a cobertura da morte do gerenciador,
não a de kernel travado, e exigiria root, outra unidade e hidraw fora da
regra udev da sessão. Nenhuma dessas camadas novas foi implementada ou testada
no aparelho nesta fase de artefatos.

A leitura do DA após corte de energia foi feita em 2026-10-27, depois dos
ensaios do feed: ver "DA depois de corte de energia, sem halod" abaixo.

Consolidação de 2026-10-27, sem envio novo ao aparelho: a matriz fonte/byte
do E0 e do E1 (GCC, design do halo e bytes enviados nos ensaios) está em
[Matriz fonte/byte](protocols/castor3.md#matriz-fontebyte-do-e0-e-do-e1-change-waterforce-cpu-feed).
Os dois JSONL não têm `B6`, e o transporte do GCC não acrescenta `B6`; isso
não prova ausência de gravação interna na flash. **E1 obrigatório continua
não comprovado**: o ensaio 2 funcionou sem E1 novo, mas com o E1 do ensaio 1
ainda valendo. O GCC encurta o nome para no máximo 30 caracteres em
`GetCpuShortName`; o ensaio usou `Ryzen 5 5600G` (13 bytes).

### Feed do halod e parada com proteção: ensaio 6.1 (2026-10-27)

Ensaio autorizado com o par novo instalado (`packaging/install.sh`), o
halod sob `WatchdogSec=20s` e `HALOD_TRACE_FEED=1` (cada envio no journal).
Observador só de leitura: `scripts/diag/waterforce_da_sniff.py 900 0 --jsonl`,
com o Tctl achado pelo rótulo. Dados em
[`ensaio-6.1.jsonl`](../openspec/changes/archive/2026-10-27-waterforce-cpu-feed/ensaio-6.1.jsonl),
separados dos ensaios 1 e 2. Sem carga na CPU.

Início do halod (10:21:31): sessão nova com `E0` de 80 °C, mais três de 80 °C
na janela segura, e o Tctl a partir de 10:21:39 (8,1 s depois), a cada ~2,02 s.
`CpuFeedActive=true` logo depois.

**Performance com o halo enviando o Tctl:** com a fan em Performance e o Tctl
em ~34–35 °C, o `DA` mostrou duty 81–82 (~2076–2244 RPM). Sem ninguém enviar
`E0`, o mesmo preset ficava em 88 (seção "Velocidade com `halo speed waterforce`" acima). O preset
também segue o `E0`.

Curva gravada pelo `SetFanCurve` (10:22:23,8; um `B6`):
0:2200/30:2200/50:2350/80:2500. O duty seguiu o Tctl enviado:

| Tctl (°C) | Duty no `DA` | Esperado pela curva |
|---|---|---|
| ~34 | 89 (a partir de +2,2 s) | 89 (2230 RPM) |
| 46–50 (atividade espontânea) | 92 → 94 | 92–94 |
| 41–42 (descendo) | 91 | 91 |

Parada com `systemctl --user stop halod.service` (10:23:26,467):

| Instante | Evento (journal ou `DA`) |
|---|---|
| 10:23:24,578 | último `E0` frio: 40 °C |
| 26,472 | SIGTERM recebido |
| 26,519 | `E0` de 80 °C do principal ("antes de sair") |
| 26,580 | `DE`/`E1`/`E0` de 80 °C do `halod --waterforce-cpu-safe` (ExecStopPost) |
| +2,6 s | `DA` ainda em 90 |
| +5,1 s | `DA` em **100** (2574 RPM) |
| +5,1 a +45,4 s | 100 em todos os `DA`, com Tctl de 35–40,6 °C (a curva daria ~90) |
| 34,581 | unidade parada (`Result=success`, 8,1 s de parada) |

Resultado: o último `E0` foi de 80 °C e o duty foi ao ponto de 80 °C da curva
preparada, e não só status 0. A parada normal passa pelas duas camadas
(SIGTERM e helper), então este ensaio não isola qual delas causou o 100: essa
distinção é o objetivo do 6.2 (SIGKILL) e do 6.3 (travamento).

Restauração: `halo speed waterforce restore` (snapshot anterior, um `B6`)
voltou a fan a Performance e a bomba a Turbo (`DD` `05 04`). O slot
Customized da fan, inativo, ficou com 2200/2200/2350/2500, e não com o
1500 ×4 anterior: o `restore` não regrava o slot de um lado que estava em
preset (limitação já registrada acima). O último `E0` ficou em 80 °C, com o
halod parado.

### SIGKILL no principal: ensaio 6.2 (2026-10-27)

Ensaio autorizado, com a mesma curva do 6.1 gravada pelo `SetFanCurve`
(10:28:35,6; um `B6`) e o mesmo observador. Dados em
[`ensaio-6.2.jsonl`](../openspec/changes/archive/2026-10-27-waterforce-cpu-feed/ensaio-6.2.jsonl).
Patamar frio: Tctl de 30,4–33,4 °C, `E0` de 31–32 °C, duty 88–89 por mais de
40 s (ponto 30:2200 da curva). Morte com
`systemctl --user kill --kill-whom=main -s KILL halod.service`, que atinge só o
principal; sem `--kill-whom=main`, o SIGKILL vai ao cgroup inteiro e mata
também o helper recém-iniciado (visto no `packaging/tests/systemd-harness.sh`).

| Instante | Evento (journal ou `DA`) |
|---|---|
| 10:29:28,753 | último `E0` frio do principal antigo (PID 165762): 32 °C |
| 28,875 | SIGKILL no principal; sai com `status=9/KILL`, sem nenhum envio |
| 28,931 | `DE`/`E1`/`E0` de 80 °C do helper (PID 166269), 56 ms depois da morte |
| 30,922 (+2,05 s) | `DA` ainda em 88 |
| **33,439 (+4,57 s)** | `DA` em **100** (2585 RPM) |
| 35,959 e 38,480 | `DA` em 100 |
| 36,932 | fim do helper (8 s); unidade `Failed with result 'signal'` |
| 39,112 | restart (RestartSec=2, contador 1); principal novo PID 166399 |
| 39,171 | primeiro `E0` do principal novo: sessão nova, 80 °C |
| 39,2 a 45,2 | janela segura do principal novo; `DA` em 100 |
| 47,243 | primeiro `E0` de Tctl do principal novo: 32 °C |
| 49,246 | `DA` volta a 88 |

Resultado: **três `DA` com duty 100 entre o `E0` do helper e o primeiro `E0`
do principal novo** (10:29:28,931 a 39,171). A proteção depois da morte
abrupta veio só do helper do ExecStopPost; a janela segura do principal novo
não entra nessa prova. O atraso até o primeiro `DA` em 100 foi de 4,5 s depois
do envio, dentro da faixa já observada (1,8–5,9 s).

Restauração com o halod rodando (`halo speed waterforce restore`, um `B6`):
fan em Performance e bomba em Turbo (`DD` `05 04`); o slot inativo da fan
continua com 2200/2200/2350/2500. O halod ficou ativo, alimentando o Tctl.

### Principal congelado (SIGSTOP): ensaio 6.3a (2026-10-27)

Ensaio autorizado, com a mesma curva (`SetFanCurve` às 10:32:18,5; um `B6`) e
o mesmo observador. Dados em
[`ensaio-6.3a.jsonl`](../openspec/changes/archive/2026-10-27-waterforce-cpu-feed/ensaio-6.3a.jsonl).
Patamar frio: Tctl de ~30–32 °C, duty 88 por mais de 40 s. Congelamento com
`systemctl --user kill --kill-whom=main -s STOP halod.service`.

| Instante | Evento (journal ou `DA`) |
|---|---|
| 10:33:09,189 | último `E0` frio do principal (PID 166399): 33 °C |
| 10,472 | SIGSTOP no principal; nenhum envio nem ping depois |
| 11,2 a 28,8 | `DA` em 88: o firmware mantém o último `E0` frio |
| 29,362 | `Watchdog timeout (limit 20s)!`; SIGKILL no principal (~20,2 s depois do último envio, e do ping que o seguiu) |
| 29,422 | `DE`/`E1`/`E0` de 80 °C do helper (PID 167828), 60 ms depois do kill |
| **31,417 (+2,0 s)** | `DA` em **100** |
| 33,939, 36,461 e 38,988 | `DA` em 100 |
| 37,423 | fim do helper; `Failed with result 'watchdog'` |
| 39,612 | restart (contador 2); principal novo PID 167865 |
| 39,673 | primeiro `E0` do principal novo: sessão nova, 80 °C |
| 47,746 | primeiro `E0` de Tctl (31 °C); `DA` de volta a 88 às 51,773 |

Resultado: o watchdog detectou o principal parado e o matou com SIGKILL, sem
SIGTERM nem core dump; **quatro `DA` com duty 100 entre o `E0` do helper e o
primeiro `E0` do principal novo** (10:33:29,422 a 39,673). A janela com o
`E0` frio retido, do último envio ao primeiro `DA` em 100, foi de 22,2 s:
20 s de watchdog, 0,06 s até o helper e 2,0 s até o `DA`. É a janela de
rotação baixa prevista no design, não um limite garantido.

O caso do worker preso com os ticks vivos (6.3b) não foi feito no aparelho: o
binário instalado não tem como prender só a thread do Waterforce. Ele está
coberto pelos testes automáticos (`cpu_feed_watchdog_tick_vivo_com_worker_preso`)
e pelo cenário "sem pings" do `packaging/tests/systemd-harness.sh`.

Restauração com o halod rodando (`restore`, um `B6`): fan em Performance,
bomba em Turbo. O halod ficou ativo, alimentando o Tctl.

### Feed com a CLI e lock externo: ensaio 6.4 (2026-10-27)

Ensaio autorizado, sem `B6` e sem mudar modo ou curva (fan em Performance).
DA em [`ensaio-6.4.jsonl`](../openspec/changes/archive/2026-10-27-waterforce-cpu-feed/ensaio-6.4.jsonl)
e horários da CLI e do lock em
[`ensaio-6.4-eventos.jsonl`](../openspec/changes/archive/2026-10-27-waterforce-cpu-feed/ensaio-6.4-eventos.jsonl).
O lock externo usou `flock /dev/hidraw2 sleep N`, que toma o mesmo `flock` do
halo sem enviar nada ao aparelho.

- **CLI em sequência:** cinco `halo speed waterforce status` (DE/DD/D9), de
  ~80 ms cada, todos completos (rc 0). Nenhum `E0` caiu dentro de uma
  transação da CLI; os `E0` saíram antes ou depois delas, na cadência de
  ~2,02 s.
- **Lock de 3 s** (10:59:33,962 a 36,967): o tick que venceu no meio esperou;
  um único `E0` (35 °C, Tctl relido) saiu 20 ms depois da liberação, e o
  seguinte 1,5 s depois. Um período perdido, sem rajada.
- **Lock de 8 s** (10:59:48,967 a 56,971): o feed desistiu com Busy aos
  5,0 s do pedido (55,638), tentou a proteção numa transação nova, que pegou o
  lock na liberação: `E0` de 80 °C aceito às 57,048. Sessão nova às 57,092
  (outro 80 °C, 44 ms depois; corrigido no código depois do ensaio para não
  sair colado), janela de 8 s em 80 °C e Tctl atual (34 °C) às 11:00:06,8,
  sem replay das amostras perdidas. Último `E0` frio antes da espera:
  10:59:48,612; sem envio por 8,4 s.
- **DA:** duty 81–82 no Performance com o Tctl de ~34 °C; ficou em 81 durante
  os locks (o firmware manteve o último `E0`), foi a 100 às 11:00:00,7 (3,6 s
  depois do `E0` de 80 °C) e voltou a 81 às 11:00:10,8, 4 s depois do primeiro
  `E0` de Tctl. O `DA` continuou chegando durante os locks: o pedido é do
  driver do kernel, que não usa o `flock` do halo.
- **Watchdog:** nenhum restart (`NRestarts` igual antes e depois). A
  contenção de 8 s não gerou falso positivo.

### Carga real com `stress-ng`: ensaio 6.5 (2026-10-27)

Ensaio autorizado, com o par instalado às 11:06:46 (inclui a correção do
`E0` colado depois da recuperação) e a mesma curva (`SetFanCurve` às
11:08:15,2; um `B6`). Carga: `stress-ng --cpu 0 --timeout 60s` (12 threads),
com aborto automático se o Tctl chegasse a 90 °C (não chegou). DA em
[`ensaio-6.5.jsonl`](../openspec/changes/archive/2026-10-27-waterforce-cpu-feed/ensaio-6.5.jsonl),
fases em
[`ensaio-6.5-eventos.jsonl`](../openspec/changes/archive/2026-10-27-waterforce-cpu-feed/ensaio-6.5-eventos.jsonl);
o `E0` enviado vem do journal (`HALOD_TRACE_FEED=1`). Tempos relativos ao
início da carga (11:08:51,9).

| Instante | Tctl | `E0` enviado | Duty no `DA` | RPM da fan |
|---|---|---|---|---|
| −30 a 0 s (repouso) | 37–41 °C | 37–42 | 89–91 | 2230–2541 |
| +2,0 s | 54,5 | 41 (ainda o anterior) | 91 | 2541 |
| +6,0 s | 62,5 | 61 | 91 | 2541 |
| +8,0 s | 63,2 | 63 | **96** | 2442 |
| +20 a +60 s | 65–66 | 65–66 | 97 | 2442–2772 |
| +64,6 s (fim da carga) | 61,5 | 64 | 97 | 2508 |
| +80,8 s | 50,0 | 52 | 94 | 2475 |
| +100,9 s | 42,5 | 44 | 91 | 2368 |
| +121 a +137 s | 38–38,6 | 38–39 | 90 | 2298–2409 |

- **Duty seguiu o Tctl** nos dois sentidos, nos pontos da curva: 66 °C dá
  2430 RPM pela interpolação entre 50:2350 e 80:2500, ou 97 %; 50 °C dá 94;
  38–40 °C, 90–91. O primeiro `DA` com o duty novo veio ~2 s depois do `E0`
  que o causou.
- **RPM:** média de 2392 no repouso, 2534 sob carga (10–61 s) e 2363 no
  repouso seguinte (100–140 s). O RPM oscila ±150 entre leituras, então a
  subida e a descida aparecem na média, não leitura a leitura.
- **Cadência:** 99 `E0`, intervalos de 2,012 a 2,037 s (mediana 2,020 s),
  sem rajada nem lacuna. Tctl máximo de 66,0 °C; `E0` máximo de 66.
- **Watchdog:** nenhum restart.

Com esta curva, o 5600G sob carga total ficou em ~66 °C e a fan não chegou ao
ponto de 80 °C. Restauração com o halod rodando (`restore`, um `B6`): fan em
Performance, bomba em Turbo.

### DA depois de corte de energia, sem halod: tarefa 1.2 (2026-10-27)

Leitura autorizada. Antes do corte: `systemctl --user disable --now
halod.service` às 11:17:58,7, com o último `E0` de 80 °C enviado pelo
SIGTERM (58,995) e pelo helper (59,058). O PC desligou às 11:21:10, com corte
de energia pelo usuário, e ligou às 11:24:50. O halod não rodou no boot novo
(unidade desabilitada, sem entradas no journal, nenhum processo). Às
11:26:27, o observador só de leitura registrou 16 `DA` em 40 s; depois,
`halo speed waterforce status` (DE/DD/D9). Dados em
[`baseline-pos-corte.jsonl`](../openspec/changes/archive/2026-10-27-waterforce-cpu-feed/baseline-pos-corte.jsonl).

| Leitura | Resultado |
|---|---|
| Modos (`DD`) | fan Performance (5), bomba Turbo (4), os gravados com `B6` |
| Slot da fan (`D9 01`) | 0:2200/30:2200/50:2350/80:2500, o último gravado com `B6` |
| Slot da bomba (`D9 02`) | 2600 ×4 |
| `DA` da fan | duty **88** em todas as leituras, 2217–2272 RPM, com Tctl de 38,8–44,1 °C |

Com o último `E0` em 80 °C, o Performance deu duty 100 nos ensaios. Depois
do corte deu 88, o mesmo valor do Performance antes de o halo enviar `E0`
(seção "Velocidade com `halo speed waterforce`"). Conclusão limitada: **o
`E0` de 80 °C não sobreviveu ao corte de energia**, enquanto modos e slots
gravados com `B6` sobreviveram. O duty 88 bate com a hipótese da referência
padrão de ~50 °C no Performance (ponto 50:2200 da tabela do GCC), mas o `DA`
não lê a referência interna: isso continua hipótese. A leitura também não
prova ausência de escrita interna na flash (só que o valor usado depois do
boot não é o último `E0`) nem identifica a origem "Windows" do padrão. A
ordem prevista (ler antes de instalar o feed) não foi seguida: o feed já
tinha rodado, e o último `E0` antes do corte era conhecido (80 °C).

Consequência para o halo: depois de um corte de energia, enquanto o halod
não sobe, a curva usa a referência própria do firmware, não o último `E0`.
O halod abre cada sessão com `E0` de 80 °C, então nada muda no fluxo.

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

**Segunda ocorrência, depois de logout e login (2026-10-27, ~22:39–22:45).** O usuário saiu e entrou na sessão para carregar a extensão nova (change `gnome-panel-cpu-source`). Com a sessão, o `halod` parou às 22:39:17 (`E0` de 80 °C do próprio daemon e do `ExecStopPost`) e subiu às 22:39:37 (`E1`, janela de 8 s em 80 °C e depois o Tctl a cada 2 s, sem `B6`). O journal do `halod` só tem o stop e o start nesse intervalo, sem nenhum pedido de velocidade ou LED. A barra do painel e o LCD do Waterforce mostraram o líquido alto logo depois do login, depois descendo e passando por `0°`. Leituras só do sysfs, sem enviar nada:

```
hora        temp1_input  Tctl (m°C)
22:41:35    32000        36125      (halod: CoolantMilliCelsius 32000)
22:41:45    48000        35000      48000 por ~17 s, depois 32000 e 48000 de novo
22:44:08    16000        32750      fan 2244, bomba 3232
22:44:14        0        32625      0 por ~6 s
22:44:20    16000        32250
```

Captura passiva do `99 DA` às 22:45:56–22:46:23, com `scripts/diag/waterforce_da_sniff.py 40 14` (só leitura, quem pede o `DA` é o driver). A leitura já tinha voltado ao normal:

```
hora     temp1   Tctl | b2..13 do DA
22:45:56   26000  31.4 | 14 08 00 a9 0c 00 50 64 47 38 01 19
22:46:07   26000  31.6 | b4 06 00 b7 0c 00 50 64 34 38 01 19
22:46:17   25000  34.2 | 61 08 00 85 0c 00 51 64 41 38 01 1a
22:46:23   25000  35.0 | fe 07 00 b0 0c 00 52 64 3e 2f 01 19
```

- Os valores errados (0, 16, 32, 48 °C, ou seja, `00`, `10`, `20`, `30`) têm o nibble baixo em `0`, como o `e0`/`f0` de 2026-10-20. Como byte com sinal, a primeira ocorrência ficou entre −32 e −16, e a segunda entre 0 e 48. Isso reforça a hipótese de um valor que não é temperatura, mas continua sendo hipótese.
- Desta vez o erro **passou sozinho** em uns 6 minutos, sem corte de energia, e o byte 13 voltou a `19`/`1a` (25–26 °C), estável. Depois disso, o usuário ainda desligou o PC, tirou da tomada por alguns segundos e religou (boot às ~22:59). Às 23:06, o líquido estava em 26 °C, coerente com o Tctl (31–37 °C), com a `amdgpu` do 5600G (edge 36 °C) e com o `gigabyte_wmi`. O corte veio depois da normalização, então não foi o que a resolveu.
- O LCD mostrou o mesmo valor que o hwmon, então o erro está no aparelho, e não no driver nem no halo. Fan e bomba seguiram normais (2045–2244 e 3220–3290 RPM), e o duty da fan (byte 8, 80–82 %) acompanhou o Tctl enviado.
- As duas ocorrências vieram depois de comandos do host: um `E5`/`E6` na primeira e, na segunda, o par stop/start do `halod` (`E0` de 80 °C, `E1`, `E0`), sem pedido de velocidade. Duas ocorrências não provam a causa. O `halod` alimenta o `E0` desde 2026-10-27 de manhã, com reinícios nos ensaios 6.1–6.5, e só este episódio foi visto.
- **GCC revisto (C# do `ucCooler` e do `AorusLcdService`, sem nada enviado):** nenhum código trata, valida ou recupera o byte 13. O único pedido de `DA` é o `DeviceApi.GetRPMSpeed`, que só lê o RPM. Os "Reset" do GCC voltam o LED e a fan ao padrão, e nenhum reinicia o Castor3. No LCD, o líquido é só o bit `Display.LiquidTemperature` (`0x40`), e o firmware desenha o valor. No Windows, um erro desses apareceria do mesmo jeito no LCD, sem correção. A diferença de envio que mais chama atenção é que o GCC espera 20 s depois de achar o aparelho antes do `E1` (`CoolerDataApi.GetCoolerData`), e o `halod` manda o `E1` ao iniciar. Ela ainda não foi testada como causa.
- **Lacuna do halo:** 0 a 48 °C passam pelo critério de sanidade (líquido > 100 °C é inválido), então a barra mostrou `0°` em vez de `--`. Detectar isso exigiria outro critério, por exemplo um salto impossível entre leituras, o que é trabalho para outra change.
- **Se acontecer de novo:** anotar o horário, rodar `python3 -I scripts/diag/waterforce_da_sniff.py 120 0 --jsonl <arquivo>` durante o erro e cruzar com `journalctl --user -u halod -o short-iso-precise`.

**LCD durante a proteção de 80 °C (2026-10-27, 23:12).** Num logout, o usuário viu o LCD do Waterforce mostrar a CPU em **80 °C** e a fan acelerar. O journal mostra o `E0` de 80 °C do `halod` (23:12:10,951) e do `ExecStopPost` (23:12:11,017), com a unidade parada às 23:12:19 e de volta às 23:12:35 (mais 8 s em 80 °C antes do Tctl).

- O LCD desenha o byte da temperatura da CPU do `E0`, o mesmo que o firmware usa como referência da curva. Por isso, durante a proteção (stop, logout, falha e os 8 s de cada início), o LCD mostra 80 °C. Isso é esperado, e não uma leitura errada. O `CpuMilliCelsius` do D-Bus continua vindo do Tctl e não mostra o 80.
- Entre o logout e o login, ninguém envia `E0`, e o aparelho fica no último valor (80 °C). A unidade é de usuário (`PartOf=graphical-session.target`), e o acesso ao hidraw vem do `uaccess` (ACL da sessão ativa no assento), então o `halod` não roda na tela de login. No Windows, o `AorusLcdService` é um serviço do sistema (`MainService : ServiceBase`), que roda antes do login e não para no logout. Lá, o LCD mostra a CPU real também na tela de login.

**Bloqueio e logout medidos (2026-10-27, 23:27–23:32).** Captura passiva do `DA` com `scripts/diag/waterforce_da_sniff.py` (como usuário no bloqueio; como root no logout, por `sudo systemd-run --unit=halo-da-capture`, porque na tela de login o `uaccess` tira a ACL do hidraw), cruzada com o `LockedHint` do logind e o journal do `halod`. Fan em Performance, bomba em Turbo.

| Estado | Janela | Tctl | Duty da fan | RPM da fan |
|---|---|---|---|---|
| Sessão ativa | 23:27:16–23:27:48 | 30,8–32,6 °C | 80–81 % | 2009–2211 |
| Tela bloqueada (`LockedHint=yes`) | 23:27:48–23:28:23 | 37,0–43,6 °C | 80–85 % | 2076–2244 |
| Sessão ativa, antes do logout | 23:31:09–23:31:23 | 34,0–35,4 °C | 81–82 % | 2178–2211 |
| Logout: `E0` de 80 °C (23:31:23,867 e ,932) | 23:31:26–23:31:31 | 34,3–35,3 °C | **100 %** | 2178–2541 |
| Tela de login, sem `halod` | 23:31:31–23:32:06 | 34,3–46,0 °C | **100 %** | 2541–2607 |
| Login: janela de 8 s em 80 °C | 23:32:06–23:32:14 | 44,1–50,0 °C | **100 %** | 2508–2541 |
| Sessão ativa, com o Tctl | 23:32:16–23:32:40 | 41,4–51,4 °C | 88 → 85 % | 2173–2541 |

- **Bloquear a tela não muda nada:** o `halod` continua ativo, o journal não tem nenhuma entrada, e a fan segue a CPU.
- **O "modo game" é o logout:** do `E0` de 80 °C até o primeiro Tctl depois do login, a fan ficou ~50 s em duty 100 com a CPU entre 34 e 50 °C. São ~35 s na tela de login, mais a parada e a janela de 8 s do início. Um logout mais longo deixa a fan no máximo pelo tempo todo da tela de login. A bomba não mudou (Turbo).
- Boot até o login: não medido. Pela mesma lógica, o aparelho fica com o último `E0` (os 80 °C do desligamento), a não ser que perca energia. Depois de um corte de energia, o Performance deu duty 88, compatível com a referência padrão de ~50 °C.

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
