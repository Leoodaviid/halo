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
| Permissão | `crw-rw----+`, ACL dá `rw` ao usuário `bumblebee`. Não precisa de root |

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

Zonas da B550M AORUS ELITE (layout `it5702_24_device`, código de produto `0x0030005F`):

| Zona | Tipo |
|---|---|
| `D_LED1` | Fita endereçável (ARGB) |
| `D_LED2` | Fita endereçável (ARGB) |
| `LED_C1` | Cor única |
| `LED_CPU` | Cor única |
| `LED_C2` | Cor única |

O OpenRGB já detectou a placa por USB em 2026-10-17, então o comando de leitura `CC 60` já foi enviado a ela sem problemas.

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
