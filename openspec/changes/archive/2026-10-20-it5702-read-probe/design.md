# Design

## Context

O repositório só tem documentação, e a máquina não tem toolchain Rust. A placa IT5702 (`048d:5702`) aparece como hidraw com permissão de leitura e escrita para o usuário via ACL, então não precisa de root. O descritor HID tem usage page `0xFF89` e dois feature reports: `0x5A` (16 bytes) e `0xCC` (63 bytes + ID). Ver `docs/hardware.md`.

Fonte do protocolo: `/mnt/workspace/tools/repos/openrgb/Controllers/GigabyteRGBFusion2USBController/` (commit `72262b8`).

## Goals / Non-Goals

**Goals:**
- Criar uma base de código que o daemon da Fase 2 reaproveite (descoberta e protocolo numa biblioteca, CLI fina por cima).
- Ler o IT5702 sem mudar nada no estado dele.

**Non-Goals:**
- Calibração `CC 61` (só existe no IT5711) e qualquer comando de escrita.
- Mapear o layout de zonas pelo nome DMI. Isso fica para a change de cor estática.
- D-Bus, daemon e extensão GNOME.

## Decisions

### Workspace com biblioteca e binário

```text
Cargo.toml            workspace
crates/halo-core/     biblioteca: descoberta HID, transporte hidraw, protocolo IT5702
crates/halo-cli/      binário `halo`
```

O daemon `haloD` da Fase 2 vai ser outro crate em cima de `halo-core`. *Alternativa:* um crate único agora, separado depois. Rejeitada porque a separação custa pouco agora e evita misturar a lógica com a CLI.

### Acesso direto ao hidraw, sem hidapi

Descoberta por sysfs: para cada `/sys/class/hidraw/hidrawN/device/`, ler `uevent` (campo `HID_ID=0003:0000048D:00005702`) e `report_descriptor`, e extrair a Usage Page de cada coleção de topo (item `06 lo hi` em vigor quando a coleção abre). Um dispositivo combina se qualquer coleção de topo tiver a usage page procurada. Os feature reports usam os ioctls `HIDIOCSFEATURE` e `HIDIOCGFEATURE` em `/dev/hidrawN`, via `nix` ou `libc`.

*Alternativa:* crate `hidapi`. Rejeitada porque traz uma dependência C (libudev/hidapi) e porque o modo linux-native dele também depende de libudev. O halo só roda em Linux, e a enumeração por sysfs abre só os metadados, o que atende ao requisito de não abrir outros dispositivos.

Para decidir pela usage page, o parser percorre os itens curtos (pulando itens longos), acompanha a Usage Page global (`0x05`/`0x06`/`0x07`) e a profundidade de Collection/End Collection, e registra a página de cada coleção de profundidade zero. Não é um parser HID completo, e não precisa ser. Olhar só a primeira página não basta: a interface 1 do teclado (`hidraw4`) começa com `0x0001` e só depois declara `0xFF00`.

### Sequência da leitura (portada do OpenRGB)

| Passo | Bytes | Fonte |
|---|---|---|
| 1. Set feature report, 64 bytes | `CC 60 00 00` + zeros | `RefreshHardwareInfo()` chama `SendCCReport(0x60, 0x00)`, que monta o buffer em `SendReport()` e envia com `hid_send_feature_report` em `SendPacket()` (`GigabyteRGBFusion2USBController.cpp`) |
| 2. Get feature report, 64 bytes | `buffer[0] = 0xCC` | `RefreshHardwareInfo()`, `hid_get_feature_report(dev, buffer, 64)` com `report_id = 0xCC` |

O construtor `RGBFusion2USBController::RGBFusion2USBController` chama depois `EnableLampArray(false)` (se `support_cmd_flag & 0x02`), `ResetController()` e `EnableBeat(false)`. O halo **não** porta esses passos.

### Decodificação como função pura

`IT8297Report` (`GigabyteRGBFusion2USBController.h`, dentro de `#pragma pack(push, 1)`), little-endian:

| Offset | Tamanho | Campo |
|---|---|---|
| 0 | 1 | `report_id` (`0xCC`) |
| 1 | 1 | `product` |
| 2 | 1 | `device_num` |
| 3 | 1 | `strip_detect` |
| 4 | 4 | `fw_ver`, mostrado como `b0.b1.b2.b3` (do byte menos ao mais significativo) |
| 8 | 1 | `argb01_count` (nibble baixo = fita 0, alto = fita 1) |
| 9 | 1 | `argb23_count` |
| 10 | 1 | `argb45_count` |
| 11 | 1 | `support_cmd_flag` |
| 12 | 28 | `str_product`, cortado no primeiro `\0` |
| 40 | 4 | `cal_spare0` |
| 44 | 4 | `cal_strip0` |
| 48 | 4 | `cal_strip1` |
| 52 | 4 | `rgb_cali` |
| 56 | 4 | `chip_id`, mostrado como `0x%08X` |
| 60 | 4 | `cal_spare1` |

Total: 64 bytes. Contagem de LEDs: `enum LEDCount` (`LEDS_32 = 0` … `LEDS_1024 = 4`) e `LedCountFromEnum()`, em que valores fora da faixa viram 1024.

`decode(&[u8]) -> Result<It5702Report, DecodeError>` não faz I/O, então os testes unitários usam buffers fixos. Respostas com menos de 64 bytes viram erro, como o `res < sizeof(IT8297Report)` do OpenRGB.

### CLI

`halo probe it5702 [--raw]`, com `clap`. A saída é texto com rótulos, e `--raw` adiciona o hexdump dos 64 bytes. Os códigos de saída distinguem não encontrado, sem permissão e erro de protocolo.

### Licença

`LICENSE` com o texto da GPL-2.0, `license = "GPL-2.0-or-later"` nos `Cargo.toml` e cabeçalho SPDX nos arquivos que portam lógica do OpenRGB, citando a origem.

## Risks / Trade-offs

- [O ioctl de feature report com tamanho ou ID errado pode ser enviado ao dispositivo] → o buffer é montado por uma função testada que sempre gera exatamente `CC 60` + 62 zeros, e um teste confere esses bytes.
- [O parser de usage page simplificado erra em descritores atípicos] → só precisa funcionar para os descritores documentados em `docs/hardware.md`, que viram fixtures de teste.
- [Ler um campo mudar o estado do controlador] → `CC 60` é o mesmo pedido que o OpenRGB já fez a esta placa em 2026-10-17 sem efeito visível. Depois da primeira execução, conferir visualmente que as luzes não mudaram.
- [A ordem dos bytes do firmware ser ambígua] → mostrar também a saída bruta (`--raw`) e registrar os bytes reais em `docs/hardware.md`.

## Migration Plan

Não se aplica: é o primeiro código do projeto. Para reverter, basta remover o workspace.
