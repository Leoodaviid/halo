# Proposal

## Why

Com o `acpi_enforce_resources=lax` ligado (2026-10-31), o kernel registra o SMBus da placa e instancia o `ee1004` nos dois pentes. Uma leitura manual do SPD mostrou que eles são Kingston Fury Beast Refresh, que o FURY CTRL controla (`docs/hardware.md`, seções "Software oficial: FURY CTRL" e "Primeiro boot com o lax"). A regra do projeto é que todo dispositivo novo começa com uma ferramenta que só lê. A Fase 5 precisa dessa ferramenta no halo, reproduzível, antes de qualquer conversa com o controlador RGB em `0x27`.

## What Changes

- Novo comando `halo probe ram`. Ele lista os pentes DDR4 lendo só o arquivo `eeprom` que o driver `ee1004` do kernel publica no sysfs. Não abre `/dev/i2c-*` e não precisa de root nem do grupo `i2c`.
- O barramento é achado pelo nome do adaptador (`SMBus PIIX4 adapter port 0 at 0b00`), nunca pelo número `i2c-N`, que muda com o lax.
- Para cada pente, mostra o endereço do SPD, o slot, o tipo (byte 2), o fabricante JEDEC (bytes 320–321), o part number (329–348) e o veredito de suporte RGB com o critério do FURY CTRL: byte 330 `F`/`H`, byte 359 = modelo, byte 360 ∈ {`0x10`, `0x20`, `0x22`, `0x23`}, com o nome do modelo.
- Mensagens e códigos de saída próprios para:
  - lax desligado ou `ee1004` ausente;
  - instância do `ee1004` que não responde (`ENXIO`), como a fantasma `2-0051` no barramento ASF;
  - SPD curto;
  - pente que não é da Kingston.
- Lógica de decodificação em `halo-core` (módulo novo `ram`), testada com SPDs de 512 bytes montados com os bytes reais lidos em 2026-10-31, e testes da CLI com um sysfs falso.
- Fora de escopo:
  - qualquer transação em `/dev/i2c-*`;
  - a sondagem em `0x27`;
  - escrita no controlador ou no SPD;
  - daemon, D-Bus e painel;
  - DDR5;
  - mudanças de regra udev ou de parâmetro de boot.

## Capabilities

### New Capabilities
- `ram-probe`: leitura, só pelo sysfs, do SPD dos pentes DDR4 no SMBus da placa, com o veredito de suporte ao RGB da Kingston.

### Modified Capabilities
(nenhuma)

## Impact

- `crates/halo-core`: módulo novo `ram` (busca no sysfs e decodificação do SPD), sem dependências novas.
- `crates/halo-cli`: subcomando `probe ram`, que reaproveita o `--sys-root` oculto e os códigos de saída `3` (não encontrado) e `5` (falha).
- `crates/halo-cli/tests/probe.rs`: testes com sysfs falso.
- `docs/hardware.md` e `docs/roadmap.md`: registro da primeira execução no aparelho.
- Hardware: nenhum byte enviado pelo halo. O kernel continua fazendo a troca de página do SPD ao ler o `eeprom`, como já faz no boot.
