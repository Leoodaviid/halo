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

## AORUS Waterforce X II (Castor3)

| Item | Valor |
|---|---|
| USB | `1044:7a4d` (VID da Holtek, usado pela Gigabyte nos periféricos AORUS), "ITE Tech. Castor3", bcdDevice `0200` |
| hidraw | `/dev/hidraw2` |
| Permissão | `crw-------`, só root. Precisa de regra udev |
| Modelo | Waterforce X II **240** (confirmado pelo usuário) |

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

A tela LCD não está implementada no OpenRGB. A saída de 6 KB sugere que as imagens vão por esse canal. Investigar o GIGABYTE Control Center com o REA.

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

O driver não controla velocidade: o duty é só leitura. Não há protocolo documentado para mudar fan ou bomba.

**Modelo em dúvida.** O usuário confirmou X II 240, e o descritor HID bate com o driver X II do OpenRGB. Mas o VID:PID é o que o kernel registra para a primeira geração. Antes de usar o protocolo do anel do OpenRGB, confirmar se o firmware responde ao `DE 00` como o X II (change `waterforce-read-probe`).

## Mouse Logitech G502 SE HERO

`046d:c08b`, `/dev/hidraw0` e `/dev/hidraw1`, permissão liberada ao usuário. Suportado pelo driver Logitech do OpenRGB (`LOGITECH_G502_HERO_PID`). Detectado pelo OpenRGB em 2026-10-17.

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

O halo vai ter uma regra própria só para o que usa (change `waterforce-read-probe`).

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
