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

### Fita ARGB no `D_LED1` (2026-11-01)

O usuário ligou uma fita ARGB no `D_LED1`. O `CC 60` lido com a fita conectada é **idêntico byte a byte** ao de 2026-10-20: `strip_detect` continua `0x07` e as contagens continuam `0x00`. Ou seja, o `CC 60` não indica se há uma fita nem em qual porta.

Cor estática enviada pelo script provisório [`scripts/dev/it5702-dled1-color.py`](../scripts/dev/it5702-dled1-color.py), com os pacotes do caminho de efeito de hardware do OpenRGB (`SetStripBuiltinEffectState`, `SetLEDEffect` e `ApplyEffect`):

| Pacote | Bytes (o resto é zero) |
|---|---|
| `CC 32` | `cc 32 00`: máscara de efeitos desligados = 0, bit `0x01` = `D_LED1` |
| `CC 25` | `cc 25` + `zone0` = `0x20` (u32 LE) + `zone1` = 0 + reservado + tipo `01` (estático) + brilho máx. `ff` + mín. `00` + `color0` nos bytes 14–17 = `B G R 00` |
| `CC 28` | `cc 28` + `zone_sel0` = `0x20` (u32 LE) |

Resultado visual, confirmado pelo usuário: `ff0000` deixou a fita **vermelha** e `ffffff` deixou a fita **branca**. A ordem `R G B` pedida no `color0` sai certa na fita, e o firmware aplica sozinho a calibração `GRB`. Depois de enviar, o `CC 60` continuou respondendo normalmente. Nada foi gravado na flash (nem `CC 47` nem `CC 5E`), então a cor não deve sobreviver a um desligamento.

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
- Boot até o login: não medido nesta data (com o `halod` fora da sessão, ver abaixo). Pela mesma lógica, o aparelho fica com o último `E0` (os 80 °C do desligamento), a não ser que perca energia. Depois de um corte de energia, o Performance deu duty 88, compatível com a referência padrão de ~50 °C.

**Daemon fora da sessão gráfica (change `halod-outside-session`, 2026-10-29 e 2026-10-30).** Captura passiva do `DA` como root (`sudo systemd-run --unit=halo-da-capture ... waterforce_da_sniff.py 600 0 --jsonl`), cruzada com o journal do usuário (`halod`, `halod-session`, `dbus.service`) e o do logind. Linger ligado, usuário no `plugdev`. Fan em Performance, bomba em Turbo. Nenhum byte novo: só o `E1`/`E0` do feed.

- **O GNOME derruba o D-Bus da sessão em todo logout.** O `gnome-session-shutdown.target` tem `Wants=gnome-session-restart-dbus.service` (`gnome-session-ctl --restart-dbus`, `StopUnit dbus.service`), uns 5 s depois de o `graphical-session.target` parar. Na primeira rodada do 5.2 (22:41), o `halod` sem reconexão falhou em cada tick com `Broken pipe` e foi morto pelo watchdog às 22:42:11. Com a reconexão, o nome voltou em 14–21 ms em todos os logouts seguintes, no mesmo processo.
- **A regra nova só vale num nó criado de novo.** Depois de `udevadm trigger --action=change`, o nó era `crw-rw----+ root plugdev` com `group::---` e `mask::rw-`: num arquivo com a ACL da sessão, o `MODE="0660"` vira um `chmod` que muda só a máscara. Depois do boot, `group::rw-` (ver [permissions.md](permissions.md)).

Regra antiga (só `uaccess`), 2026-10-29, `MainPID` 179947 e `NRestarts` 0 nos dois logouts:

| Estado | Janela | Tctl | Duty da fan | Journal |
|---|---|---|---|---|
| Sessão ativa | 23:41:45–23:42:26 | 33,8–37,0 °C | 81–83 % | — |
| Logout: `End()` | 23:42:27,664 → ,713 | 37,6–43,8 °C | 100 % a partir de 23:42:32 | "acesso só pela ACL"; `E0` de 80 °C aceito; guarda parada (,7138) antes do alvo (,7140); fim da sessão no logind às 23:42:33,06 |
| D-Bus cai e volta | 23:42:33,047 → ,066 | — | — | "caiu; reconectando" e o nome de volta |
| Tela de login | 23:42:33–23:44:21 | 29,0–46,2 °C | **100 %** | feed pausado, sem watchdog nem restart |
| Login: E1 e 8 s em 80 °C | 23:44:22,76–23:44:31 | 38,5–50,9 °C | 100 % | "retomando o feed da CPU" |
| Sessão ativa, com o Tctl | 23:44:33– | 44–51 °C | 88 → 86 % | — |

Boot com a regra nova, 2026-10-30: kernel às 20:34:54; o `halod` sobe às 20:35:08,1 ("acesso pelo grupo plugdev", sem pausa), antes do greeter (20:35:09,7); ~2 min 10 s na tela de login, sem restart (com o feed ativo, o watchdog só é renovado por envios aceitos); login às 20:37:20, com o `Begin()` só respondendo. Não há `DA` dessa tela de login: a captura transitória não sobrevive ao boot.

Regra nova, 2026-10-30, `MainPID` 2025 e `NRestarts` 0 do boot ao fim dos dois logouts:

| Estado | Janela | Tctl | Duty da fan | Journal |
|---|---|---|---|---|
| Sessão ativa | 20:42:18–20:43:04 | 35,1–37,6 °C | 82–83 % | — |
| Logout 1: `End()` | 20:43:04,313 | — | segue o Tctl | "acesso pelo grupo plugdev", sem `E0` de 80 °C |
| D-Bus cai e volta | 20:43:09,517 → ,538 | — | — | reconexão no mesmo processo |
| Tela de login 1 | 20:43:09–20:45:24 | 30,4–49,0 °C | **80–87 %** | — |
| Login 1 | 20:45:26 | 30,6 → 51 °C | 80 → 88 % | `Begin()` só responde: sem E1 nem janela de 8 s |
| Logout 2 e tela de login 2 | 20:46:59–20:48:04 | 32,0–44,6 °C | 80–85 % | reconexão às 20:46:59,743 → ,779 |
| Login 2 | 20:48:06 | 32 → 52 °C | 80 → 90 % | `Begin()` só responde |

| Tctl | Duty, sessão ativa | Duty, tela de login |
|---|---|---|
| 35 °C | 82 % | 82 % |
| 36 °C | 82–83 % | 82–83 % |
| 37 °C | 82–83 % | 83 % |

- Com a regra nova, a tela de login fica como a sessão ativa: o duty segue o Tctl, igual ao da sessão com a mesma CPU, e o LCD mostra a CPU real. Antes, a fan ficava em 100 % a tela de login inteira (tabela de 2026-10-27 acima).
- Com a regra antiga, o comportamento é o da proteção: `E0` de 80 °C antes de a sessão terminar, pausa sem nenhum Tctl e E1 com 8 s em 80 °C no login.

### Lock externo e o Apply do painel: conferência 4.3 da `gnome-panel-profiles` (2026-10-31)

Com o `halod` da sessão (feed da CPU ativo) e a extensão instalada, o usuário
segurou o nó do Castor3 com `flock -x /dev/hidraw2 sleep 10`, que só abre o nó
e não envia nada, e escolheu um efeito no painel. Os horários vêm de um
`dbus-monitor` só de leitura (chamadas de `Led.Apply`, respostas e erros do
`halod`) e do journal da unidade.

- **O worker atende um pedido por vez, e o feed costuma estar na frente.** Com
  o lock externo, o tick do feed (a cada 2 s) entra no worker e espera o
  `flock`. Ele desiste com `Busy` depois de 5 s e tenta o `E0` de 80 °C, que
  também espera. Um `Apply` pedido nesse meio-tempo fica na fila e só começa a
  contar os 5 s dele depois. Na rodada das 10:40, o `Apply(static)` saiu às
  10:40:00,7, o feed registrou `Busy` às 10:40:02, o lock soltou por volta das
  10:40:07, a proteção foi aceita, e o `Apply` respondeu **com sucesso** às
  10:40:07,7, depois de 7 s na fila. Na rodada das 10:32, foi igual: 6,6 s e
  sucesso. Nenhum dos dois recebeu `Busy`.
- **O `Busy` só chega ao painel se o `Apply` passar à frente do feed.** Numa
  rodada sem o monitor, entre 10:01 e 10:08, o `Apply` chegou ao worker antes
  do tick, e o painel mostrou a faixa "O aparelho está em uso por outro
  processo", com os controles de volta ao estado aplicado. Com um lock de 10 s,
  isso depende de frações de segundo. Forçar o `Busy` pediria um lock de 15 a
  18 s, o feed e a proteção esperando antes do `Apply`, o que encosta no
  `WatchdogSec=20s` da unidade.
- **Não é uma falha do halod nem do painel.** Cada efeito escolhido virou um
  único `Apply` com um `99 B6`, mesmo esperando o lock. O "Tentar de novo"
  depois de um `Busy` real não foi observado isolado no aparelho. Numa rodada,
  ele caiu num segundo `flock` rodado logo em seguida. Ele foi conferido no
  Shell aninhado, com um único `Led.Apply` por clique.

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

## RAM Kingston DDR4 (adiada)

| Item | Valor |
|---|---|
| Modelo | Kingston `KF3200C16D4/16GX` (fabricante JEDEC Bank 2, `0x98`) |
| Protocolo | Depende do modelo (ver "Protocolo do Beast Refresh: `0x58` + slot", 2026-11-01). No FURY CTRL 2.0.66.0, o HyperX Predator e Fury, o Fury Renegade e o Fury Beast usam a API v1, no controlador em `0x27` (registradores do `HyperXDRAMController` do OpenRGB). O Fury Beast SE, o Fury Beast Refresh (estes pentes) e o Renegade Refresh usam a API v2, um controlador por pente em `0x58` + slot, a assinatura do `KingstonFuryDRAMController` do OpenRGB. Neste PC, nada responde em `0x27` |
| Barramento | SMBus do AMD FCH (`1022:790b` rev `51`, PCI `00:14.0`), base `0xB00` |

O SMBus não está disponível: o módulo `i2c_piix4` carrega, mas o kernel registra `ACPI Warning: SystemIO range 0x0B00-0x0B08 conflicts with OpRegion 0x0B00-0x0B0F (\GSA1.SMBI)` e `ACPI: OSL: Resource conflict; ACPI support missing from driver?`, e não cria o barramento. Os únicos barramentos I2C existentes (`/dev/i2c-0` a `i2c-8`) são da GPU. O OpenRGB também não achou nenhuma memória.

Liberar exige o parâmetro de boot `acpi_enforce_resources=lax`. O estudo abaixo mede quando o firmware usa o mesmo barramento.

### Quem usa o SMBus no firmware (estudo de 2026-10-31)

Só leitura: nenhum byte foi enviado ao SMBus e o boot não mudou. Firmware: B550M AORUS ELITE, BIOS `F20i` (09/03/2025). As tabelas vieram do `acpidump` (root) e foram descompiladas com o `iasl` 20251212. Hashes (sha256): DSDT `3872f710…5d1417a9`, SSDT8 `42ae591e…8b6f8f62`, SSDT9 `e00d598d…d5114ed0`.

A `\GSA1.SMBI` fica na SSDT9 (`GBT`/`GSWApp`), um aparelho WMI da Gigabyte (`PNP0C14`, `_UID` `GSADEV0`). O `GSA1` registra no `_WDG` os GUIDs `DEADBEEF-1000` (dados `AA`), `DEADBEEF-2001` (método `WMBB`, o mesmo GUID do `gigabyte_wmi`) e `DEADBEEF-4002` (evento `0xE2`). A região tem `HSTS`, `HCNT`, `HCMD`, `TXSA`, `DAT0`, `DAT1` e `HBDR`, e as primitivas `SMB0`–`SMBN` usam o protocolo do PIIX4: `HCNT = 0x44` byte, `0x48` byte data, `0x4C` word, `0x50` process call e `0x54` bloco. O endereço vai em 8 bits (`TXSA = addr`, ou `addr | 1` na leitura). Uma segunda região, `SMG0` em `0xB20` (ASF), tem o próprio mutex `SME3`.

Para reproduzir, a partir de uma pasta vazia:

```sh
sudo acpidump -o acpi.dat                 # root; o arquivo fica legível
acpixtract -a acpi.dat
iasl -e ssdt*.dat -d dsdt.dat && for f in ssdt*.dat; do iasl -d "$f"; done
F=HSTS,HCNT,HCMD,TXSA,DAT0,DAT1,HBDR
python3 -I scripts/re/aml_field_callers.py . $F        # 116 métodos: campos, chamadores
python3 -I scripts/re/aml_field_callers.py . $F roots  # raízes e um caminho de cada
```

O script é estático e casa nomes de 4 caracteres. Ele não vê chamadas dinâmicas nem o que roda em SMM. A tabela agrupa as raízes que ele devolve; os casos marcados como lidos à mão foram conferidos no `.dsl`.

| Raiz e caminho no AML | Toca `0xB00`? | Quem chama | Roda em uso normal? |
|---|---|---|---|
| `_PTS` → `_SB.MPTS` → `GSA1.MPTS` → `EZV8` → `EZVH`/`EZV3` → `ISMF`, `ISM5`, `XDP9`… → `SMM5`/`SMM2` → `SMB5`/`SMB2` | Sim. Lê o VRM em `0xC0` (7 bits: `0x60`) | O kernel, ao suspender. O `EZV8` não faz nada em S5 | Não. O Linux chama o `_PTS` no `prepare_late`, depois do `dpm_suspend_late`, com os processos congelados (`kernel/power/suspend.c`, `drivers/acpi/sleep.c`) |
| `_WAK` → `_SB.MWAK` → `GSA1.MWAK` → `EZV7` → `EZVI`/`EZV4` → `ISM6`, `ISMG`, `ISM3`, `XDPA`… → `SMB2`/`SMB4` | Sim. **Escreve** no VRM em `0xC0` (restaura o LLC e o modo do Vcore e do SOC, com o desbloqueio `0xA55A` no `XDPA`) | O kernel, ao retomar | Não. O Linux chama o `_WAK` no `.wake`, antes do `dpm_resume_noirq` |
| `GSA1.WMBB`, comandos `0x5E`–`0x6D` → `SMMx` → `SMBx` | Sim. SMBus genérico com qualquer endereço: `0x67` = write byte data, `0x68` = read byte data, `0x69`/`0x6A` = word, `0x65`/`0x66` = send/receive byte. O primeiro byte escolhe o barramento (`2` = `0xB00`, `3` = `0xB20`) | Só um cliente WMI | Não. No Linux, o único driver ligado ao `DEADBEEF-2001` é o `gigabyte_wmi` |
| `WMBB` `0x0E`/`0x0F` → `MPTS`/`MWAK` | Sim, como acima | Cliente WMI | Não |
| `WMBB` `0x130`/`0x136` → `DIMD`/`DIM1` (sensor de temperatura e SPD dos pentes) | Não neste BIOS. A tabela `PFMH` lista os SPD em `0xA0`/`0xA2`/`0xA4`/`0xA6` com as flags em `0`, e o `PFM0` que as preencheria está vazio. Volta `0xFFFFFFFF` antes de tocar o barramento (lido à mão) | Cliente WMI | Não |
| `WMBB` `0x125` → `ZRCD` → `SIO0` (temperatura do `gigabyte_wmi`) | Não. Lê a Super I/O pela região `SIOH` em `0x2E` | `gigabyte_wmi`, a cada leitura do hwmon | Sim, mas fora do SMBus |
| `ISK1`–`ISKA`, `ISME`, `ISMR`, `NCT0`, `SMB9`, `XDP8`, `XDPD`, `XDPJ`, `XDPU`–`XDPX` | Sim | Ninguém no AML | Não |
| `TPM._DSM` → `STRT` | Falso positivo: `HSTS`/`HCMD` de 32 bits da região `TPMR` (memória), outro objeto | — | — |
| Eventos GPE (`_L02`…`_L19`), `_Qxx` e o `_INI` do `GSA1` | Nenhum chega à `SMBI` | — | — |

A tabela `GSAT` do `GSA1` diz o que mora no barramento além da RAM: o VRM do Vcore e do SOC (`RAA229004`/`ISL69147`, `0xC0`) e dois DACs de tensão `NCT3933` em `0x20` (VDD18, VDDP) e `0x2A` (**tensão da DRAM**, VPP e terminação). Em 7 bits, isso dá `0x60`, `0x10` e `0x15`. O AML não troca a porta do SMBus: o `PMRG` não tem o campo de porta do KERNCZ, e o `SBSL` da SSDT8 (`PM+0x2E`) ninguém chama. Por isso, esses aparelhos provavelmente ficam na porta padrão, a mesma da RAM. Qualquer varredura do barramento passa por eles.

**Não existe lock comum.** O AML serializa com o mutex `SME0`, que só existe no AML. Ele não usa o semáforo do controlador (`SMBSLVCNT`, offset `0x08`, fora do campo `SMBI`). O `i2c-piix4` usa esse semáforo e não conhece o `SME0`. O `SRDY` do AML espera ~200 ms por um host ocupado e depois aborta a transação com `KILL` (`HCNT |= 0x02`). Hoje, a segurança depende só do momento: `_PTS` e `_WAK` rodam com os aparelhos do Linux parados.

No Windows, o `GLedApi.dll` do GCC (`nsis-gbt-cooler-named/`, sha256 `a9059a92…`) acessa o SMBus direto, com o mutex nomeado `SMBusCommandMutex` e o `gdrv3.sys`, e também não usa o mutex do AML. Pelas mensagens de erro, o caminho que aparece é o de um controlador Intel. Não foi analisado a fundo. O pacote do RGB Fusion em `samples/` ainda é um `setup.exe` fechado; não foi aberto.

### SMM do BIOS (estudo de 2026-10-31)

O SMM não aparece no AML, então foi analisado na imagem do BIOS, só lendo os arquivos. Nada foi gravado na placa. O F20i não está mais no site da Gigabyte, então a análise usou os vizinhos dele, o F20g e o F21 (`tools/samples/gigabyte-b550m-aorus-elite-bios/`, zips oficiais com os sha256 `bde5b73b…5d223` e `4276f4e8…2b1a`). A extração foi feita com o UEFIExtract NE A75 (`~/.local/opt/uefiextract-ne-a75/`), e os módulos foram decompilados com o Ghidra pelo REA (`analysis/smm-f21/decomp-*`).

- **O F21 serve de substituto.** A SSDT `GSWApp` do F21 só difere do F20i no endereço da `GGGT`, que o BIOS corrige no boot, e na string da versão. A do F20g também difere no número de build.
- **A triagem é igual no F20g e no F21** (`python3 -I scripts/re/smm_triage.py <imagem>.dump`): mesmos módulos, mesmos despachos e mesmos consumidores do SMBus, embora todos os binários mudem de hash entre as versões.
- **O `SmBusSmm` é o serviço de SMBus do SMM.** Ele instala o protocolo `72e40094-2ee1-497a-8f33-4c934a9e9c0c`, com o GUID montado com imediatos, e não registra nenhum tratador próprio. Só três módulos SMM citam esse protocolo:

| Módulo | Quando roda | O que faz no SMBus |
|---|---|---|
| `IT8297LedSmm` | Tratador de Sx para S3, S4 e S5, e do botão de energia | Escreve em `0x68`: write block com o comando passado pelo chamador e write byte no reg `0x28` (operações `9` e `5` do `EFI_SMBUS_OPERATION`). É o endereço do controlador RGB Fusion 2 por SMBus segundo o OpenRGB. Nesta placa, o RGB é o IT5702 por USB, então provavelmente não há nada em `0x68` |
| `ResetSmi` | SW SMI `0xBE`, que é o valor de reset da FADT (`0xBE` na porta `0xB2`): só quando o sistema reinicia | Read block em `0x68` reg `0x10` (operação `8`), além de acesso direto ao ASF em `0xB20` |
| `GbtCrbSmm` | Tratadores de Sx, do botão de energia e de um SW SMI | Localiza o protocolo e guarda o ponteiro, mas nenhuma função o chama |

- **Nenhum tratador periódico usa o SMBus.** Só o `FchSmmDispatcher`, que produz os despachos, e o `UsbRtSmm` (USB legado) citam o timer periódico, e nenhum dos dois consome o protocolo de SMBus. O tratador de GPI do `GbtCrbSmm` não chama o SMBus.
- O `PiSmmCpuDxeSmm` também tem um `mov edx, 0xB00`, mas não foi conferido; provavelmente é uma constante sem relação com o SMBus.

Conclusão: o SMM só mexe no SMBus ao suspender, desligar, reiniciar ou no botão de energia, e só no endereço `0x68`. Com o PC em uso, nada no SMM usa o SMBus. **Limites:** é uma análise estática do F21 e do F20g, não do F20i. Um módulo que calcule a base do SMBus na hora, sem o protocolo, não aparece na triagem. E o Ghidra não confirmou cada caminho até o tratador (o `IT8297LedSmm` chama o SMBus a partir do rótulo `LAB_00001648`, registrado para S3/S4/S5 e para o botão).

### Software oficial: FURY CTRL 2.0.66.0 (estudo de 2026-10-31)

Fonte: `tools/samples/kingston-fury-ctrl/`, com o zip oficial (sha256 `d366c5b3…f4508`), que nunca foi executado. Ele tem um Inno Setup 6.1, que leva a um msixbundle e a um msix. Dentro do msix, a interface .NET (`FURYCTRL.exe`) e o SDK `FuryCTRL_SDK.exe`, outro Inno Setup, versão 6.6.1. Esse SDK foi aberto com o `innoextract` 1.13.0 do fork crazy-max, compilado do código-fonte em `tools/repos/innoextract-crazy-max/build/`. O SDK tem:
- `FuryControllerService.exe`, o serviço .NET que a interface chama por `ws://127.0.0.1:<porta>` com JSON (`api`, `ctrl_settings`, `dram`);
- `FuryCTRL.dll`, a "MBAPI" nativa feita pela MSI (PDB `D:\MSI\MBAPI - FuryCTRL_1.0.0.34`);
- `SMBus_Engine*.dll` e o driver `NTIOLib*.sys`, que faz a E/S de porta.

O .NET foi decompilado com o ilspycmd, e o `FuryCTRL.dll` com o Ghidra pelo REA (`analysis/decomp-FuryCTRL/`).

**Identificação do pente: só pelo SPD.** O `CheckDRAMSupport(slot, …)` lê o SPD e exige `KINGSTON` no fabricante. Depois, o `FUN_1000af50` troca para a página 1 do SPD (write em `0x6E` em 8 bits, que é `0x37` em 7 bits) e lê três bytes:

| Byte do SPD | Leitura | Significado |
|---|---|---|
| 330 (`0x14A`) | read byte data `0x4A` na página 1 | `0x46` (`F`) ou `0x48` (`H`). É o 2º caractere do part number, que começa no byte 329: `KF…` ou `HX…` |
| 359 (`0x167`) | `0x67` na página 1 | Modelo. Com `H`: 0 = HyperX Predator, 1 = HyperX Fury. Com `F`: 0 = Renegade, 1 = Beast, 2 = Beast SE, 3 = Renegade Refresh, 4 = Beast Refresh |
| 360 (`0x168`) | `0x68` na página 1 | Precisa ser `0x10`, `0x20`, `0x22` ou `0x23`; senão, "NotSupport" |

Nenhum byte vai para o controlador RGB nessa etapa. Fora a troca de página, que o `ee1004` do kernel também faz, é só leitura do SPD.

**Controle do DDR4 (API v1): tudo no endereço `0x4E` em 8 bits, ou `0x27` em 7 bits.** Com o pente aceito (marca `H` ou `F`), o `DAT_1020b7e4 = 1` habilita as funções `Parameter*`, e cada uma faz um write byte data em `0x27`, com até 30 tentativas:
- `ParameterStart`: reg `0xE1` = `1`;
- `ParameterSet(reg, valor)`: escreve o registrador. O serviço usa `0xDD`–`0xE5`; por exemplo, `226` = `0xE2` e `229` = `0xE5` são os modos;
- `ParameterDone`: reg `0xE1` = `2`;
- `ParameterRun`: reg `0xE1` = `3`.

Isso bate com o `HYPERX_REG_APPLY = 0xE1` (1, 2, 3) e os registradores `0xD1`–`0xEE` do `HyperXDRAMController` do OpenRGB. O serviço (`KSDRAMSMBusCtrlObj.applyDRAMLEDmode`) espera o mutex do DRAM antes do `ParameterStart`. O `SMBus_Engine` serializa com os mutexes globais `Global\Access_SMBUS.HTP.Method` e `Global\Access_SMBUS.HTP.Renesas.Method`, a convenção dos fabricantes de placa no Windows. O firmware também não os respeita.

Consequências para o halo:
- **Dá para decidir sem escrever no controlador.** Com o lax, basta ler os bytes 330, 359 e 360 do SPD pelo sysfs do `ee1004`. Se o byte 360 não for `0x10`, `0x20`, `0x22` ou `0x23`, o pente não tem RGB controlável e a Fase 5 acaba aí.
- ~~**O protocolo a seguir é o do `0x27`**, com o conjunto de registradores do HyperX, e não a assinatura `FURY` em `0x58`.~~ **Errado para estes pentes** (2026-11-01): o Beast Refresh usa a API v2, em `0x58` + slot. Ver "Protocolo do Beast Refresh". Os valores de cada modo ficam nos `*_mode.cs` do serviço (`ilspy-FuryControllerService/`).
- ~~A assinatura da Fury em `0x58`, com duas escritas por slot, deixa de ser necessária.~~ Ela é o protocolo destes pentes.

### Transações da detecção da Kingston (OpenRGB `72262b8`)

| Passo | Endereço (7 bits) | Operação | Tipo |
|---|---|---|---|
| Kernel, sozinho, quando o barramento sobe: `i2c_register_spd_write_enable()` na porta 0 instancia o `ee1004` em cada pente que o DMI lista | `0x36`/`0x37`, `0x50`–`0x53` | Troca de página (send byte `0x00` em `0x36`/`0x37`) e read byte data; o nvmem é `read_only` | Escrita só no seletor de página |
| SPD pelo OpenRGB: usa o sysfs do `ee1004` se ele existir; senão, `DDR4DirectAccessor` | `0x36`, `0x50`+n | Send byte `0x00` em `0x36`, read byte data reg `0x00` (espera `0x23`) e write byte data `0x36`/`0x37` cmd `0x00` dado `0xFF` para trocar a página | Escrita no seletor |
| SPD pelo OpenRGB, quando o `DDR4DirectAccessor` falha (byte 0 diferente de `0x23` ou erro de leitura): `DDR5DirectAccessor::isAvailable` | `0x50`+n | Read byte data regs `0x00` e `0x01`; se não for um hub DDR5, read byte data reg `0x0B` e, se o valor estiver entre 1 e 15, **write byte data reg `0x0B` = `0x00`** | **Escrita no SPD**: num DDR4, o byte 11 é a tensão nominal (normalmente `0x03`) e só fica protegido se o pente tiver o write protect ligado |
| `HyperXDRAMController` (`JEDEC_KINGSTON`) | `0x27` | Receive byte | Leitura |
| `KingstonFuryDRAMController` DDR4, por slot | `0x58` + slot | Write byte data reg `0x08` = `0x53` (início); read word data regs `0x01`–`0x04`, com o byte alto igual a `F`,`U`,`R`,`Y` (até 3 tentativas cada); read word reg `0x06` (modelo: `0x23` Beast, `0x21` Beast White); write byte data reg `0x08` = `0x44` (fim). 10 ms entre cada uma, até 3 tentativas por slot | **Duas escritas por slot** |

A detecção da Fury, portanto, não é só leitura: identificar o controlador já exige escrever no pente. E rodar a detecção do OpenRGB sem o `ee1004` carregado pode gravar no próprio SPD, pelo caminho do DDR5. É mais um motivo para nunca rodar o OpenRGB com o barramento liberado.

### Recomendação

**Go condicional, só para leitura**, se valerem três condições antes do primeiro boot com o lax:

1. **Tirar o `uaccess` genérico de `i2c-*`** do `/etc/udev/rules.d/60-openrgb.rules` (linha 10, `KERNEL=="i2c-[0-99]*", TAG+="uaccess"`). Com o barramento criado, essa regra deixa qualquer processo da sessão, inclusive o AppImage do OpenRGB, escrever no VRM e no DAC da tensão da DRAM.
2. **Não carregar `it87` nem `nct6775`.** O lax desliga a checagem de conflito de todos os drivers, não só do `i2c-piix4`, e o AML lê a Super I/O em `0x2E` a cada leitura do `gigabyte_wmi`. Hoje nenhum dos dois está carregado.
3. **Aceitar o que sobra:** não há lock comum entre o firmware e o Linux. O momento protege `_PTS`/`_WAK` e os tratadores SMM de Sx, reset e botão de energia, mas um cliente WMI futuro não ficaria protegido. A análise do SMM é estática e feita no F21/F20g (ver "SMM do BIOS"). E o VRM fica no mesmo barramento.

Primeiro contato, sem nenhum código do halo no barramento:

- conferir `journalctl -k -b | grep -iE 'piix4|ee1004|spd'` e `ls /sys/bus/i2c/drivers/ee1004/`;
- ler `/sys/bus/i2c/drivers/ee1004/<N>-005{0..3}/eeprom` (512 bytes, nvmem só leitura). O kernel faz a troca de página em `0x36`/`0x37`;
- decodificar o byte 2 (`0x0C` = DDR4), os bytes 320–321 (fabricante) e 329–348 (part number), para confirmar o `KF3200C16D4/16GX`, e os bytes 330, 359 e 360, para aplicar o mesmo critério do FURY CTRL (ver "Software oficial").

A ferramenta do halo começaria lendo só esse sysfs, sem `/dev/i2c-*`. Escrever em `0x27` (protocolo do HyperX) fica para uma decisão à parte, e só se o SPD disser que o pente é suportado. A assinatura da Fury em `0x58` não é necessária. Varredura com `i2cdetect` nunca.

**Alternativa sem o lax:** chamar `\GSA1.WMBB` com os comandos `0x67`/`0x6A` deixa o firmware como único dono do barramento, sob o `SME0`. Isso exige um módulo fora da árvore (`acpi-call-dkms`; o Secure Boot está desligado) ou um driver WMI próprio, e o `acpi_call` dá ao root acesso a qualquer método ACPI. Não foi testado.

### Primeiro boot com o lax: leitura do SPD (2026-10-31)

O lax foi ligado a pedido do usuário. Antes do boot, ele instalou `/etc/udev/rules.d/72-halo-smbus-no-uaccess.rules` (`SUBSYSTEM=="i2c-dev", ATTR{name}=="SMBus PIIX4 adapter*", TAG-="uaccess"`), que roda antes do `73-seat-late.rules`, e pôs `acpi_enforce_resources=lax` no `GRUB_CMDLINE_LINUX_DEFAULT` com `update-grub`. Para desfazer: tirar o parâmetro, rodar `update-grub` de novo e **reiniciar**, porque o kernel em execução continua com o lax e os barramentos continuam registrados até o próximo boot. Depois, conferir que o `/proc/cmdline` não tem mais `acpi_enforce_resources=lax`. Nenhum programa do usuário falou com o barramento: só o kernel (`i2c_piix4` e `ee1004`). A leitura foi feita pelo sysfs.

- **Kernel:** os conflitos com `\GSA1.SMBI` (`0xB00`) e `\GSA1.SMG0` (`0xB20`) viraram avisos ("System may be unstable"), e o `piix4_smbus` registrou três barramentos: `i2c-0` = porta 0 em `0xB00`, `i2c-1` = porta 2 em `0xB00` e `i2c-2` = porta 1 do auxiliar em `0xB20`. A porta é escolhida pelo registrador `0x02` (KERNCZ). Os barramentos da GPU passaram a ser `i2c-3` em diante: **a numeração muda com o lax**, então o halo precisa achar o barramento pelo nome, nunca pelo número.
- **SPD:** o kernel instanciou o `ee1004` em `0-0052` e `0-0053` (512 bytes, só leitura) e também em `2-0051`, no ASF. Lá, o seletor de página não respondeu (`Failed to select page 0 (-6)`) e a leitura dá `ENXIO`: é uma instância fantasma, mas mostra que o kernel mandou um send byte em `0x36` no barramento ASF ao subir.
- **Permissões:** `/dev/i2c-0..2` são `root:i2c 660`, sem ACL do usuário (a regra 72 funcionou), e o usuário não está no grupo `i2c`. O `/dev/i2c-3` da GPU continua com `user:bumblebee:rw-`. Nem o `it87` nem o `nct6775` carregaram.

| Byte do SPD | `0-0052` | `0-0053` |
|---|---|---|
| 2 (tipo) | `0x0C` (DDR4) | `0x0C` |
| 320–321 (fabricante) | `01 98` (Kingston) | `01 98` |
| 329–348 (part number) | `KF3200C16D4/16GX` | `KF3200C16D4/16GX` |
| 330 | `0x46` (`F`) | `0x46` (`F`) |
| 359 | `0x04` | `0x04` |
| 360 | `0x23` | `0x23` |
| 364 | `0x43` | `0x42` |

Pelo critério do FURY CTRL, os dois pentes são **KingstonFury_Beast_Refresh** e são suportados: marca `F`, modelo 4 e byte 360 = `0x23`. O `0x23` também é o `FURY_MODEL_BEAST_DDR4` que o OpenRGB lê no reg `0x06` do controlador. Os pentes estão nos slots de SPD `0x52` e `0x53` (índices 2 e 3). O byte 364 é o único que difere entre os dois (`C`/`B`), e o significado dele não foi confirmado. Os bytes 368–375 trazem 8 dígitos ASCII diferentes em cada pente.

### Atualização da BIOS para a F22d (2026-11-01)

O usuário atualizou a BIOS para a **F22d** (18/08/2026). O zip oficial `mb_bios_b550m-aorus-elite_8arnr013_f22d.zip` (sha256 `84197d4c…3377`) está em `tools/samples/gigabyte-b550m-aorus-elite-bios/` e foi extraído com o UEFIExtract em `extracted/uefi-f22d/`. Tudo abaixo foi conferido só lendo a máquina e a imagem:

- **Máquina:** o lax continua no `/proc/cmdline`, o GRUB não foi afetado, e os avisos do kernel, os barramentos `i2c-0..2`, as instâncias do `ee1004` (`0-0052`, `0-0053`, `2-0051`) e as permissões (`root:i2c 660`, sem ACL do usuário) estão iguais. O SPD dos dois pentes não mudou (bytes 330, 359 e 360 = `F`, `0x04`, `0x23`).
- **ACPI (tabelas tiradas da imagem):** os quatro DSDT da F22d são idênticos aos do F21. A SSDT `GSWApp` cresceu de 47.058 para 47.688 bytes:
  - métodos novos `EM02`–`EM05` (formatação de texto), `EZW9`, `EZWA` e `MEMF`, e saíram o `EZW7` e o `EZW8`;
  - um comando novo no `WMBB` (`0x96`);
  - a `PFMH` (pentes) e a `GSAT` (VRM e DACs) não mudaram.

  O `aml_field_callers.py` dá o mesmo grafo nas duas versões (116 métodos, 25 raízes, os mesmos caminhos de `_PTS`, `_WAK` e `WMBB` até a `SMBI`), e nenhum método novo chega à região.
- **SMM:** o `smm_triage.py` dá a mesma lista no F21 e na F22d. O `GbtCrbSmm`, o `IT8297LedSmm` e o `ResetSmi`, os únicos que usam o protocolo de SMBus, são idênticos byte a byte aos do F21. Só o `SmBusSmm` mudou de hash, e ele continua sem registrar tratador nenhum.

As conclusões das seções "Quem usa o SMBus no firmware" e "SMM do BIOS" valem para a F22d. A diferença é que agora elas se apoiam na imagem da mesma versão que está instalada, e não num vizinho. As tabelas que rodam na máquina (`acpidump`) não foram tiradas de novo; o kernel, porém, dá os mesmos avisos e as mesmas regiões (`\GSA1.SMBI` e `\GSA1.SMG0`).

### Primeira execução do `halo probe ram` (2026-11-01)

O usuário rodou, no terminal dele e sem sudo, `target/debug/halo probe ram`, `halo probe ram --raw` e `strace -f -e trace=openat target/debug/halo probe ram`, com a BIOS F22d e o lax ligado. O halo não mandou nada ao barramento: só leu o sysfs, e a troca de página foi feita pelo kernel.

| Instância | Adaptador | Resultado |
|---|---|---|
| `0-0052` (SPD `0x52`, slot 2) | `SMBus PIIX4 adapter port 0 at 0b00` | DDR4 `0x0C`, Kingston `01 98`, `KF3200C16D4/16GX`, suportado pelo FURY CTRL: Fury Beast Refresh (marca `F`, modelo 4, byte 360 = `0x23`) |
| `0-0053` (SPD `0x53`, slot 3) | `SMBus PIIX4 adapter port 0 at 0b00` | Igual ao `0-0052` |
| `2-0051` (SPD `0x51`) | `SMBus PIIX4 adapter port 1 at 0b20` (ASF) | Sem resposta (`ENXIO`), como em 2026-10-31 |

- **Código de saída:** 0.
- **`strace`:** só aparecem `openat` em `/sys/bus/i2c/devices`: os diretórios, os arquivos `name` de todos os adaptadores e dos filhos de `i2c-0` e `i2c-2` (inclusive as instâncias `dummy` em `0x36`/`0x37`, descartadas pelo nome) e os três `eeprom`, todos com `O_RDONLY`. Nenhum `/dev/i2c-*` foi aberto. O `i2c-1` (porta 2) não tem filhos.
- **SPD bruto:** os dois pentes só diferem nos bytes 324–328 (fim da data de fabricação e número de série), 352 e 364 (`C`/`B` nos dois) e 370–376, os dígitos ASCII que começam no 369 (e não no 368, como dizia a seção de 2026-10-31). O dump não fica no repositório por causa do número de série.

### Receive byte em `0x27`: ninguém responde (2026-11-01)

Primeira transação do halo no SMBus, no terminal do usuário, com `sudo` (change `ram-hyperx-default-modes`, tarefa 3.1): `sudo target/debug/halo probe ram-controller`. Resultado: `nada respondeu em 0x27 em /dev/i2c-0`, código 3. O `strace -f -e trace=openat,ioctl` mostra que o halo leu só o sysfs e os três `eeprom`, abriu só o `/dev/i2c-0` (`O_RDWR`), fez um único `I2C_SLAVE` (`0x703`) com `0x27` e um único `I2C_SMBUS` (`0x720`), que voltou `ENXIO`. Nenhuma escrita.

### Protocolo do Beast Refresh: `0x58` + slot (2026-11-01)

Relendo o serviço e o `FuryCTRL.dll` depois do `ENXIO`, só lendo os arquivos:

- **`QueryDDRInformation`** (`FURYController_Service.cs`) monta o `DRAMInfoObj` pelo nome que o `CheckDRAMSupport` devolve. `HyperX_Predator`, `HyperX_Fury`, `KingstonFury_Renegade` e `KingstonFury_Beast` levam `type = 1` (DDR4) e **`api_ver = 1`**. `KingstonFury_Beast_SE`, **`KingstonFury_Beast_Refresh`** e `KingstonFury_Renegade_Refresh` levam `type = 1` e **`api_ver = 2`**, a mesma API do DDR5. Com `type = 1`, o `getBaseAddress` dá `iDDRBaseAddress = 88` (`0x58`); no DDR5, `96` (`0x60`).
- **A API v2 escreve em `0x58` + slot.** No caminho comum (sem o LPC Intel do X99/X299), o `SetKingstonFuryDDR5CmdStart` (`0x100187b0`) chama o motor com `(param_1 * 2, 0x08, 0x53, 0x1E)`: write byte data no endereço de 8 bits `2 × (0x58 + slot)`, registrador `0x08` = `0x53`, até 30 tentativas, e depois `Sleep(5)`. O `SetKingstonFuryDDR5Style` (`0x10018f30`) escreve o registrador `0x09`. É o início da assinatura do `KingstonFuryDRAMController` do OpenRGB (reg `0x08` = `0x53`).
- **O "model ID" vem do SPD.** O `GetKingstonFuryDDR5ModuleID(88 + slot)` (`0x1bf40`), no DDR4, só lê os bytes 327 e 328 do SPD do mesmo slot (página 1, com a troca em `0x36`/`0x37`). Ele não fala com o controlador.
- **Slot:** fora do X299, o `CheckDramSupport` percorre o SPD de 8 bits `0xA0` a `0xAE` com `j_index` de 0 a 7 (o índice do SPD), e o `address_offset` do Beast Refresh é o próprio `j_index`. Nos pentes `0x52` e `0x53`, os controladores ficam em **`0x5A` e `0x5B`**. A moldura `E1`/`E2` em `0x27` não se aplica a estes pentes, o que bate com o `ENXIO`.
- **Leituras (sem escrita antes):** `GetKingstonFuryDDR5MasterSlaveRole`, `Style`, `Speed`, `CmdValue` e `IRTransConfig` fazem read word data em `0x58` + slot, nos registradores `0x0B`, `0x09`, `0x0E`, `0x08` e `0x28`, guardam o byte alto (`valor >> 8`) e esperam 5 ms. O serviço lê o `0x0B` (papel mestre/escravo) já na detecção. A detecção do OpenRGB, ao contrário, escreve `0x08` = `0x53` antes de ler.
- **Escritas:** cada `SetKingstonFuryDDR5*` faz write byte data com até 30 tentativas e `Sleep(5)`. Os registradores, que batem com o OpenRGB:

  | Registrador | Função |
  |---|---|
  | `0x08` | `0x53` início, `0x44` fim, `0x42` bypass |
  | `0x09` | estilo |
  | `0x0B` | papel |
  | `0x0C` | direção |
  | `0x0D` | atraso |
  | `0x0E` | velocidade |
  | `0x10` | largura |
  | `0x11` | matiz |
  | `0x12`–`0x15` | tempos |
  | `0x16`–`0x19` | respiração |
  | `0x1E` | hyper speed |
  | `0x20` | brilho |
  | `0x21`/`0x22` | brilho máximo e mínimo |
  | `0x23`/`0x24` | cor de fundo |
  | `0x26` | LEDs |
  | `0x27` | pentes |
  | `0x28` | IR |
  | `0x30` | cor |

  Valores do estilo no `DDR5LEDmode`: 0 estático, 1 rainbow, 3 respiração, 4 dinâmico (o mesmo `FURY_MODE_*` do OpenRGB).
- **Molduras do serviço:**
  - o `CommandStart` envia `0x08` = `0x53` aos slots usados, do último para o primeiro, e espera 50 ms;
  - o `CommandDoneAndRun` envia `0x08` = `0x44` do primeiro para o último e espera 50 ms;
  - o `setDRAMDDR5_AllOff` é Start → por slot, `0x0B` = papel lido e `0x20` = 0 → Done. O serviço o chama ao suspender;
  - o `setDRAMDDR5_DefaultRainbow` é Start → por slot, `0x09`=1, `0x0B`=papel, `0x0D`=0, `0x0C`=1, `0x0E`=25, `0x20`=80, `0x10`=0, `0x11`=0 e `0x1E`=0 → Done;
  - o brilho padrão do serviço é 80.
- **Ainda não analisado:** o significado dos valores do papel e da velocidade; as sequências dos outros efeitos; e se existe leitura do brilho (não há `Get` para ele).

### Leitura dos controladores em `0x5A` e `0x5B` (2026-11-01)

Primeira leitura dos controladores da API v2, no terminal do usuário, com `sudo` (change `ram-hyperx-default-modes`, tarefa 3.1): `sudo target/debug/halo probe ram-controller`. Os dois responderam às três leituras do FURY CTRL (read word data, byte alto), e o código de saída foi 0:

| Pente (SPD) | Controlador | Índice (`0x0B`) | Estilo (`0x09`) | Velocidade (`0x0E`) |
|---|---|---|---|---|
| `0-0052` | `0x5A` | `0x01` | `0x01` (rainbow) | `0x19` (25) |
| `0-0053` | `0x5B` | `0x00` | `0x01` (rainbow) | `0x19` (25) |

- **O efeito de hoje é o padrão do FURY CTRL:** estilo 1 com velocidade 25, os mesmos valores que o `setDRAMDDR5_DefaultRainbow` escreve. Visualmente, a barra inteira fica de uma cor, que vai trocando.
- **Índices diferentes nos dois pentes** (`0x01` e `0x00`). Ver "Estudo do `0x0B`": é o índice de posição, e esse valor só vale logo depois de ligar o PC.
- **`strace -f -e trace=openat,ioctl`:** o único nó aberto foi o `/dev/i2c-0` (`O_RDWR`). Os `I2C_SLAVE` (`0x703`) só tiveram os argumentos `0x5a` e `0x5b`: dois na abertura, para conferir que nenhum driver ocupa os endereços, e um a cada troca de controlador. Houve exatamente seis `I2C_SMBUS` (`0x720`), todos de leitura. Nenhuma escrita.

### Primeiras escritas: `off` e `on` (2026-11-01)

No terminal do usuário, com `sudo` (change `ram-hyperx-default-modes`, primeira versão da tarefa 5.2), nesta ordem: `led ram off`, `probe ram-controller`, `led ram on`, `strace … led ram off`, `led ram on`, `probe ram-controller`.

- **O efeito visível funcionou.** O `off` apagou os dois pentes e o `on` os acendeu de novo, nas duas vezes. Os quatro comandos de escrita terminaram com código 0, sem nenhuma escrita repetida.
- **`strace` do `off`:** só o `/dev/i2c-0` foi aberto. Os `I2C_SLAVE` só tiveram `0x5a` e `0x5b`. Houve 10 `I2C_SMBUS`: as 2 leituras do `0x0B` e as 8 escritas do plano, na ordem dele.
- **Defeito do plano daquela versão:** ela relia o `0x0B` antes de cada comando e escrevia o valor lido de volta. Depois do primeiro `off`, o `0x5A` passou a ler `0x0E`, e os comandos seguintes escreveram `0x0E` no `0x0B` dele. O FURY CTRL, ao contrário, lê o `0x0B` uma vez, na detecção, e reusa o valor guardado.

### Leituras depois de ligar o PC: o `0x0B` muda sozinho (2026-11-01)

Dez leituras seguidas (`probe ram-controller` a cada segundo), ainda sem desligar: o `0x5A` leu `0x0E` em todas. O `0x5B` leu `0x00` três vezes e depois `0x0E`, **sem nenhuma escrita entre elas**. Uma leitura deu `nada respondeu em 0x5b`.

Depois, desligamento com corte de energia, boot, e 20 leituras a cada 3 s, sem nenhum `led ram`:

- as primeiras leituras deram o estado de fábrica: `0x5A` índice `0x01`, `0x5B` índice `0x00`, estilo `0x01` e velocidade `0x19` nos dois. O corte de energia desfez o `0x0E` escrito antes;
- **sem nenhuma escrita**, o `0x5B` passou a `0x0E` na 9ª leitura (uns 25 s) e o `0x5A` na 12ª (uns 35 s). Depois os dois ficaram em `0x0E`;
- 4 das 120 leituras deram lixo ou falha: estilo `0xFF` (três vezes), estilo `0x5A` (o próprio endereço) e um `ENXIO`. Cada uma apareceu isolada; a leitura seguinte veio normal;
- ao ligar, os pentes acenderam em rainbow, sincronizados. Algum tempo depois das leituras, o usuário viu os dois num **azul-claro estático**. Não se sabe ainda se isso veio das leituras do halo ou se o pente muda sozinho; o teste de controle (ligar e não rodar nada do halo) vai dizer.

### Estudo do `0x0B` (2026-11-01)

Só lendo o OpenRGB `72262b8`, o serviço do FURY CTRL e os DLLs, sem tocar no barramento:

- **É o índice de posição do pente, não um papel de mestre/escravo.** No OpenRGB, o `0x0B` é o `FURY_REG_INDEX`, e o `SendPreamble` diz: "The index tells physical location of the RAM slot from the border to the CPU slot. […] The official software writes the indices in decreasing order. Hardware effects seem to support only up to 4 sticks." O OpenRGB calcula o índice e o escreve, nunca o lê (sem o `FURY_SYNC`, escreve 0 em todos; com ele, `idx % 4`). Os valores lidos logo depois do boot (slot 2 → `1`, slot 3 → `0`) estão em ordem decrescente.
- **O FURY CTRL lê o índice uma vez e o reusa.** O `QueryDDRInformation` o lê na detecção, com o serviço recém-iniciado, e guarda em `bDRAMMasterSlave_v2`. O `AllOff` (que o serviço faz ao suspender), o `DefaultRainbow` e a reaplicação ao retomar escrevem esse valor guardado. Nada relê o `0x0B` depois.
- **As leituras do `FuryCTRL.dll` não passam por um read word data direto.** O motor do DLL é uma tabela montada com `GetProcAddress` no `SMBus_Engine.dll` (sha256 `96aa413f…05ecc`): `+0x04` = `SMB_ByteRead`, `+0x08` = `SMB_ByteWrite`, `+0x14` = `SMB_BlockRead2` (usado pelos `GetKingstonFuryDDR5*`), `+0x28` = `SMBus_SetSPDPage`. No `SMBus_Engine.dll`, o `SMB_BlockRead2` é a operação `4`, que o `FUN_10006f10` repassa ao método `+0x0C` do backend do chipset (classes `ATISMBus` para AMD e `IntelSMBus`). A vtable da `ATISMBus` (achada pelo RTTI, em `0x1015e11c`) tem o `+0x0C` em `0x10001f80`. Decompilado com o REA (`analysis/decomp-SMBusEngine/`), ele escreve nas portas do FCH `0xB04` = endereço de 8 bits + 1 (leitura), `0xB03` = registrador e `0xB02` = `0x4C` (início + word data), e lê 16 bits de `0xB05` (`DAT0` e `DAT1`). **É um SMBus read word data**, o mesmo protocolo (`0x4C`) que o `i2c-piix4` usa no `I2C_SMBUS_WORD_DATA` do halo. O `+0x04` (`0x100015a0`, escrita) usa `0xB05` = dado e `0xB02` = `0x48`, o write byte data. Se a transação falha, o motor reinicia o host (`+0x38`) e tenta mais uma vez. Nas transações, o halo e o FURY CTRL são iguais; o que difere é a frequência: o FURY CTRL só lê na detecção. O OpenRGB, no Linux, também usa read word data com o byte alto (`SmbusRead`).
- **O OpenRGB não aceita a primeira leitura.** O `SmbusRead` tenta até 5 vezes, com espera de `3 × tentativa × 10 ms`, e a detecção trata `0xFFFF` como erro e repete. A detecção do OpenRGB lê dentro da moldura (`0x08` = `0x53` … `0x44`); o FURY CTRL lê sem moldura.
- **Mais uma observação:** com os pentes no azul-claro parado, um único `probe ram-controller` (6 leituras) leu estilo `0x01` (rainbow), velocidade `0x19` e índice `0x0E` nos dois, e logo depois do comando o rainbow voltou a animar, sincronizado. As duas mudanças visíveis (congelar depois do laço de leituras, voltar depois de uma leitura) vieram logo depois de leituras do halo. O teste de controle mostrou que sem o halo nada muda (ver abaixo).
- **Hipótese não confirmada:** se o índice coordena o efeito sincronizado, os dois pentes passarem sozinhos a `0x0E` (fora de 0–3) pode estar ligado ao azul-claro estático.

### Teste de controle: sem o halo, nada muda (2026-11-01)

O PC foi desligado e ligado de novo, e nenhum comando do halo rodou por 5 minutos. O rainbow ficou animado e sincronizado o tempo todo. Depois disso, um único `sudo target/debug/halo probe ram-controller` (6 leituras) deu o estado de fábrica nos dois pentes: `0x5A` índice `0x01`, `0x5B` índice `0x00`, estilo `0x01` (rainbow) e velocidade `0x19`. O efeito continuou igual depois da leitura.

Conclusão: **o índice não muda sozinho com o tempo, e o rainbow não congela sozinho.** O `0x0E` no `0x0B` e o azul-claro parado apareceram durante as séries de leituras do halo (dezenas de read word data seguidos, com a animação rodando, fora de moldura). Uma leitura isolada não perturbou. O FURY CTRL lê cada pente uma única vez, na detecção, e depois só escreve; o OpenRGB lê dentro da moldura (`0x08` = `0x53` … `0x44`). Qual dos dois fatores protege o pente (poucas leituras ou leituras dentro da moldura) não foi separado.

### Escritas sem leitura: `off` e `on` (2026-11-01)

Com o plano corrigido (nenhuma leitura; índice `0x0B` calculado pelos slots: `0x5A` = `0x01`, `0x5B` = `0x00`), no terminal do usuário, com `sudo` e logo depois do teste de controle: `led ram off`, `led ram on`, `strace … led ram off`, `led ram on`.

- **Visual:** o `off` apagou os dois pentes nas duas vezes; o `on` trouxe o rainbow de volta animado, sincronizado e com o mesmo brilho de antes (80, o padrão do FURY CTRL). O efeito continuou estável depois, sem travar.
- **Códigos de saída:** 0 nos quatro comandos, sem nenhuma escrita repetida.
- **`strace` do `off`:** só o `/dev/i2c-0` aberto; `I2C_SLAVE` (`0x703`) só com `0x5a` e `0x5b`; exatamente 8 `I2C_SMBUS` (`0x720`), todos escritas, nenhuma leitura, na ordem do plano (`0x5B` início, `0x5A` início, índice e brilho, `0x5B` índice e brilho, `0x5A` fim, `0x5B` fim).
- **Persistência:** com os pentes em `off`, depois de um desligamento com corte de energia, eles acenderam de novo em rainbow. O estado escrito pelo halo não fica guardado no pente: cada boot volta ao padrão de fábrica. É o mesmo comportamento que explica o FURY CTRL reaplicar o perfil no boot e ao retomar; um controle que dure além do boot precisa que alguém (por exemplo o `halod`) reaplique o estado.

### Modo estático: sequência e proveniência (2026-11-01)

Estudo só de leitura de código para a change `ram-static-color`; nada foi enviado ao hardware. Fontes: `ilspy-FuryControllerService/FuryControllerService/` (`.cs`), o `FuryCTRL.dll` decompilado pelo Ghidra via REA (`analysis/decomp-FuryCTRL/functions/<endereço>.c`) e o `Controllers/KingstonFuryDRAMController/` do OpenRGB.

**Fluxo.** O estático (`DDR5LEDmode.static_color`, `ctrl_mode` = `ctrl_color`) só é enviado pelo `applyDRAMLEDmode` (`KSDRAMSMBusCtrlObj.cs:145`), que usa **duas molduras**. É diferente do `setDRAMDDR5_AllOff`, de onde vieram o `off` e o `on`, que usa uma só:

- moldura 1 (L221–290): `CommandStart` → por slot, `SetKingstonFuryDDR5MasterSlaveRole(índice guardado)` (L280) → `CommandDoneAndRun` (L290). O `IRTransConfig` (L256) só entra nas APIs 4 a 6;
- moldura 2 (L291–415): `CommandStart` → por slot, `ddr5_static_color_mode.set` (L308) → `CommandDoneAndRun` (L415).

O `CommandStart` (L670) escreve nos slots em ordem decrescente e espera 50 ms; o `CommandDoneAndRun` (L695) escreve em ordem crescente e espera 50 ms. Dentro da moldura 2, os slots vão em ordem crescente.

**Sequência para os pentes desta máquina** (`0x5A` com índice 1, `0x5B` com índice 0), com cor `R G B` e brilho `b`:

```
moldura 1:  0x5B 08=53, 0x5A 08=53, 50 ms
            0x5A 0B=01, 0x5B 0B=00
            0x5A 08=44, 0x5B 08=44, 50 ms
moldura 2:  0x5B 08=53, 0x5A 08=53, 50 ms
            0x5A 09=00 0C=01 0D=00 0E=00 30=01 27=02 20=b, 10 ms, 31=R 32=G 33=B
            0x5B 09=00 0C=01 0D=00 0E=00 30=01 27=02 20=b, 10 ms, 31=R 32=G 33=B
            0x5A 08=44, 0x5B 08=44, 50 ms
```

São 30 escritas (write byte data) e nenhuma leitura. No DLL, cada setter escreve no endereço de 8 bits `param_1 * 2`, com até 30 tentativas (`0x1E`), e espera 5 ms no fim. O `ColorIndex` espera 5 ms só depois das três escritas, e o serviço espera 10 ms antes de cada cor (`Miscellaneous.cs:862`).

| Registrador | Valor | Serviço (`.cs`) | `FuryCTRL.dll` | OpenRGB |
|---|---|---|---|---|
| `0x08` | `0x53` / `0x44` | `KSDRAMSMBusCtrlObj.cs:676`, `:701` | `SetKingstonFuryDDR5CmdStart@0x100187b0`, `CmdDone@0x100188c0` | `FURY_REG_APPLY`, `FURY_BEGIN_TRNSFER` / `FURY_END_TRNSFER` |
| `0x0B` | índice de posição | `KSDRAMSMBusCtrlObj.cs:280` | `MasterSlaveRole@0x100192f0` | `FURY_REG_INDEX` |
| `0x09` | `0` (estático) | `ddr5_static_color_mode.cs:33` | `Style@0x10018f30` | `FURY_REG_MODE`, `FURY_MODE_STATIC` |
| `0x0C` | `1` | `ddr5_static_color_mode.cs:7`, `:36` | `Direction@0x10019410` | `FURY_REG_DIRECTION`, `FURY_DIR_BOTTOM_TO_TOP` (`RGBController_KingstonFuryDRAM.cpp:449`) |
| `0x0D` | `0` | `:9`, `:40` | `IRDelay@0x10019530` | `FURY_REG_DELAY` (`:450`) |
| `0x0E` | `0` | `:11`, `:44` | `Speed@0x10019650` | `FURY_REG_SPEED` (`:451`) |
| `0x30` | `1` | `:15`, `:48` | `SelectColor@0x1001ad60` | `FURY_REG_NUM_COLORS` |
| `0x27` | número de pentes (2) | `:50` (`CheckValueRange(iNumDimm, 4, 1, 4)`), `KSDRAMSMBusCtrlObj.cs:308` (`dDRAMInfos.Count`) | `NumDimm@0x1001a800` | `FURY_REG_NUM_SLOTS` |
| `0x20` | 0–100 | `:63`, `Miscellaneous.cs:941` | `Brightness@0x1001a2a0` | `FURY_REG_BRIGHTNESS` (`Static.brightness_max = 100`) |
| `0x31 + 3i`, `0x32 + 3i`, `0x33 + 3i` | R, G, B da cor `i` | `Miscellaneous.cs:855` (`setDDR5ColorsTable`) | `ColorIndex@0x1001ae80`: `param_2 *= 3`, registradores `'1' + param_2`, `'2' + param_2`, `'3' + param_2` com `r`, `g`, `b` | `FURY_REG_MODE_BASE_RED/GREEN/BLUE + idx * 3` (`KingstonFuryDRAMController.cpp:290`) |

**Respostas:**

- **O `0x27` é um registrador da API v2** (`NumDimm`, `FURY_REG_NUM_SLOTS`), e não o endereço do controlador da API v1. O número coincide por acaso.
- **Faixas.** O brilho vai de 0 a 100. Fora disso, o FURY CTRL troca o valor por 80 sem avisar (`CheckValueRange(_obj.brightness, 80)`). Com `power_saving`, o brilho vai para 10. A cor vai crua para o registrador (`(byte)`), sem escala nem limite. A regra "acima de 26, divide por 10" só existe no `setDDR5BackgroundColor` (`Miscellaneous.cs:825`), que o estático não usa.
- **Leituras:** nenhuma. As condições do `set` (`_mode[]`, `_brightness[]`, `ddr5_color_table_cache`) comparam com caches em memória. O cache das cores é salvo em disco (`SaveDDR5ColorCache`) e recarregado na partida (`FURYController_Service.cs:1555`), e o FURY CTRL só escreve o que mudou. Sem cache, o halo escreve a sequência inteira, que é o caminho da primeira aplicação.
- **Cor 0 de fábrica:** vermelho (`ddr5_color_table_default["0"][0]` = `{255, 0, 0}`, `Miscellaneous.cs:14`). Um estático cujas escritas de cor falharam aparece vermelho. Por isso a checagem visual não pode começar pelo `ff0000`: a ordem é `0000ff`, `00ff00` e depois `ff0000`, e o par azul/vermelho mostra se o R e o B estão trocados.

**Divergências entre o FURY CTRL e o OpenRGB** (vale o FURY CTRL):

- **Ordem:** o FURY escreve todos os registradores de um pente e depois do outro. O OpenRGB escreve cada registrador em todos os pentes (`SetRegister`).
- **Início da moldura:** em ordem decrescente de slot no FURY, crescente no OpenRGB (`SendBegin`).
- **Índice `0x0B`:** o FURY o escreve numa moldura própria a cada aplicação. O OpenRGB só o escreve no `SendPreamble` e, sem o `FURY_SYNC`, com o valor 0.
- **Moldura 2:** no OpenRGB, ela vai na ordem modo, direção, atraso, velocidade, número de cores, cor, brilho, número de pentes. No FURY, o brilho e o número de pentes vêm antes da cor.
- **Pausas:** o OpenRGB usa `FURY_DELAY` = 10 ms depois de cada registrador. As do FURY estão acima.

### Primeiro ensaio do estático (2026-11-01)

No terminal do usuário, com `sudo`, na branch `feat/ram-static-color` (change `ram-static-color`, tarefa 3.1), sem nenhum probe entre os comandos e nesta ordem:

```
sudo target/debug/halo led ram static --color 0000ff
sudo target/debug/halo led ram static --color 00ff00
sudo target/debug/halo led ram static --color ff0000
sudo target/debug/halo led ram static --color 0000ff --brightness 20
sudo strace -f -e trace=openat,ioctl target/debug/halo led ram static --color 0000ff
sudo target/debug/halo led ram default
```

- **Visual:** o usuário conferiu tudo como esperado:
  - azul, verde e vermelho, estáticos nos dois pentes;
  - o azul com brilho 20, mais fraco;
  - o `default` trazendo o rainbow de volta.

  O azul e o verde provam que a cor 0 foi escrita, porque a de fábrica é vermelho. O par azul/vermelho mostra que `0x31` é o R e `0x33` é o B. A sequência e os registradores de "Modo estático: sequência e proveniência" valem neste pente.
- **Códigos de saída:** 0 nos seis comandos, sem nenhuma escrita repetida. Cada um terminou com `static <cor>, brilho <n> aplicado em 0-0052 (0x5a), 0-0053 (0x5b)`, e o último com `default aplicado em …`.
- **`strace`:**
  - o único nó aberto foi o `/dev/i2c-0` (`O_RDWR`); o resto foram leituras do sysfs (nomes dos adaptadores e os `eeprom` de `0-0052`, `0-0053` e `2-0051`);
  - os `I2C_SLAVE` (`0x703`) só tiveram `0x5a` e `0x5b`: um de cada na abertura e depois só nas trocas de controlador;
  - houve exatamente 30 `I2C_SMBUS` (`0x720`), com estas trocas de endereço: `0x5B` (1), `0x5A` (2), `0x5B` (1), `0x5A` (1), `0x5B` (2), `0x5A` (11), `0x5B` (10), `0x5A` (1), `0x5B` (1). É a ordem do `--dry-run`: início no `0x5B` e no `0x5A`, os índices, os fins, o início da segunda moldura, as 10 escritas de cada pente e os fins.

  O `strace` não decodifica a direção do `I2C_SMBUS`, que fica dentro da struct. Que são todas escritas vem do código (o `apply` não tem caminho de leitura) e do teste golden `ram_estatico_sequencia_exata`.

## Referências


- Código do OpenRGB clonado em `/mnt/workspace/tools/repos/openrgb` (commit `72262b8`). Arquivos relevantes em `Controllers/`:
  - `GigabyteRGBFusion2USBController/`: placa-mãe. Layout da B550M em `GigabyteFusion2USB_Devices.cpp`.
  - `GigabyteCastor3Controller/`: Waterforce X II.
  - `LogitechController/`: mouse.
  - `KingstonFuryDRAMController/`: RAM.
- O OpenRGB é GPL-2.0-or-later. Se código dele for portado, o halo também precisa ser GPL.
- Log antigo do OpenRGB: `~/.config/OpenRGB/logs/` (perfil real do usuário, `/home/bumblebee`).
