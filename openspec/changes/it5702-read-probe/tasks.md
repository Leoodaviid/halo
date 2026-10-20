# Tasks

## 1. Toolchain e workspace

- [x] 1.1 Instalar o Rust estável via rustup no perfil do usuário e verificar com `cargo --version` e `rustc --version`
- [x] 1.2 Criar o workspace (`Cargo.toml` na raiz, `crates/halo-core`, `crates/halo-cli` com binário `halo`) e verificar que `cargo build` passa
- [x] 1.3 Adicionar `LICENSE` (GPL-2.0), `license = "GPL-2.0-or-later"` nos `Cargo.toml` e `.gitignore` com `target/`, e verificar com `cargo metadata --format-version 1 | grep GPL-2.0-or-later`

## 2. Descoberta de dispositivos (`device-discovery`)

- [x] 2.1 Implementar o parser de `HID_ID` do `uevent` e das Usage Pages das coleções de topo do `report_descriptor`, com testes unitários usando os descritores reais (IT5702 → `[0xFF89, 0xFF89]`, Castor3 → `[0x0000]`); verificar com `cargo test -p halo-core`
- [x] 2.2 Implementar a enumeração de `/sys/class/hidraw/*` que filtra por VID:PID e usage page só com leitura de sysfs, com teste sobre uma árvore sysfs falsa em diretório temporário; verificar com `cargo test`
- [x] 2.3 Implementar a abertura do hidraw com erros distintos para não encontrado e sem permissão, com teste do mapeamento de `EACCES`/`EPERM`; verificar com `cargo test`

## 3. Protocolo IT5702 (`it5702-probe`)

- [x] 3.1 Implementar o montador do pedido `CC 60` (fonte: `SendCCReport(0x60, 0x00)` / `SendReport()` em `GigabyteRGBFusion2USBController.cpp`) com teste que confere os 64 bytes exatos; verificar com `cargo test`
- [x] 3.2 Implementar `decode()` do `IT8297Report` (offsets do design, `LedCountFromEnum`) com testes para firmware `a.b.c.d`, chip id em hex, string com `\0`, nibbles de LED (`0x10` → 32/64), valor desconhecido → 1024 e resposta curta → erro; verificar com `cargo test`
- [x] 3.3 Implementar o transporte com `HIDIOCSFEATURE`/`HIDIOCGFEATURE` (64 bytes, report ID `0xCC`), sem nenhum outro comando, e verificar por revisão de código que `CC 60` é o único comando enviado
- [x] 3.4 Implementar `halo probe it5702 [--raw]` com saída rotulada e códigos de saída distintos, e verificar `halo probe it5702 --help` e a mensagem de não encontrado com um VID:PID inexistente injetado nos testes

## 4. Execução real e documentação

- [x] 4.1 Executar `halo probe it5702 --raw` na placa (comando enviado: só `CC 60`, fonte `RefreshHardwareInfo()` do OpenRGB), conferir que as luzes não mudaram e que os campos batem com o esperado (descrição do produto, 2 fitas ARGB)
  - Executado em 2026-10-20. Conferência visual não se aplica: não há fita RGB ligada à placa. Saída registrada em `docs/hardware.md`.
- [x] 4.2 Atualizar `docs/hardware.md` com os valores reais lidos (firmware, chip id, `device_num`, contagem de LEDs, flags e bytes brutos) e `README.md`/`docs/roadmap.md` com o estado da Fase 1; verificar que o comando documentado roda como escrito

## Workflow follow-up

- Arquivar a change com `/opsx:archive` depois da revisão.
- Próxima change: cor estática por zona (`D_LED1`, `D_LED2`, `LED_C1`, `LED_CPU`, `LED_C2`).
