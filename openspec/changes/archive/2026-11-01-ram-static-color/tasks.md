# Tasks

Esta change acrescenta o `halo led ram static --color RRGGBB [--brightness N]`, para os pentes Kingston Fury DDR4 da API v2 (`0x58` + slot; nesta máquina, `0x5A` e `0x5B`). O comando envia a sequência do `applyDRAMLEDmode` + `ddr5_static_color_mode.set` do FURY CTRL 2.0.66.0, em duas molduras, com 30 escritas e nenhuma leitura. A fonte de cada byte está na tabela de `design.md` e em `docs/hardware.md`, "Modo estático: sequência e proveniência". Os comandos que tocam o barramento só rodam no terminal do usuário, com `sudo` e com o aval dele.

## 1. Plano do estático no core

- [x] 1.1 Em `crates/halo-core/src/ram/controller.rs`, acrescentar:
  - ao `WriteReg`: `ColorCount` (`0x30`, `SetKingstonFuryDDR5SelectColor@0x1001ad60`), `DimmCount` (`0x27`, `NumDimm@0x1001a800`) e `Color0Red`/`Color0Green`/`Color0Blue` (`0x31`/`0x32`/`0x33`, `ColorIndex@0x1001ae80`), com o comentário de cada um citando a função do DLL e a constante do OpenRGB;
  - o `RamBrightness`, que só se constrói de 0 a 100, e o `RamCommand::Static { color: Rgb, brightness: RamBrightness }`, com `name()` = `static`.

  Verificar com testes em `cargo test -p halo-core ram`: `RamBrightness` aceita 0, 80 e 100 e recusa 101; os `code()` novos dão `0x30`, `0x27`, `0x31`, `0x32` e `0x33`.
- [x] 1.2 Reescrever o `plan()` para montar uma lista de molduras, cada uma com o `0x53` do maior slot para o menor e 50 ms, as escritas de cada pente do menor slot para o maior, e o `0x44` do menor para o maior e 50 ms. O `Static` tem duas molduras:
  - a primeira só com o `0x0B` = índice;
  - a segunda com `0x09`=0, `0x0C`=1, `0x0D`=0, `0x0E`=0, `0x30`=1, `0x27`=número de pentes (1 a 4), `0x20`=brilho, uma pausa de 10 ms e `0x31`/`0x32`/`0x33` = R/G/B.

  `Off`, `On` e `Default` continuam com uma moldura. Verificar com o `FakeBus` em `cargo test -p halo-core ram`:
  - teste golden: `static 0000ff` com brilho 80 em `0x5A`/`0x5B` dá exatamente as 30 escritas da seção de `docs/hardware.md`, na ordem, sem nenhuma leitura;
  - `ff1a1b` escreve `0xFF`, `0x1A` e `0x1B` sem escala, e o brilho 0 escreve `0x20`=`0x00`;
  - com um pente só (slot 3), `0x27`=`0x01` e `0x0B`=`0x00`;
  - uma falha de 30 tentativas no `0x5B` `0x0B` não envia o `0x44` nem a segunda moldura;
  - os testes atuais do `off`, do `on` e do `default` passam sem mudar as sequências esperadas;
  - a pausa medida antes do `0x31` é de pelo menos 10 ms.

## 2. Comando na CLI

- [x] 2.1 Em `crates/halo-cli/src/main.rs`, acrescentar ao `led ram`:
  - `static` na lista de comandos;
  - `--color RRGGBB` pelo `parse_color` existente;
  - `--brightness` com um `value_parser` de 0 a 100.

  No começo do `led_ram`, antes do `--dry-run` e do sysfs, recusar com código 2 o `static` sem `--color` e o `--color` ou `--brightness` com `off`, `on` ou `default`. O cabeçalho do `--dry-run` e a mensagem de sucesso dizem a cor e o brilho, e a ajuda explica o `static` e a faixa do brilho. Verificar com testes em `crates/halo-cli/tests/led.rs`:
  - `static --color 12ab34 --brightness 50 --dry-run` sem sysfs: as duas molduras com `0x20`=`0x32`, `0x31`=`0x12`, `0x32`=`0xAB` e `0x33`=`0x34`, e código 0;
  - `--brightness 101`, `--color 00ff0`, `static` sem cor e `off --color 0000ff`: código 2, mesmo com o nó falso em modo `000` (que daria 4 se fosse aberto);
  - sem pente suportado: código 3, sem abrir o nó;
  - `led ram --help` cita o `static`.
- [x] 2.2 Acrescentar o `static` ao `README.md` (exemplo e parágrafo da RAM) e ao `docs/roadmap.md` (Fase 5: cor estática feita; modo independente, outros efeitos, reaplicar no boot e acesso sem root pendentes). Verificar que o comando citado sai igual no `halo led ram --help` e que `halo led ram static --color 0000ff --dry-run` roda como está escrito

## 3. Ensaio no aparelho

- [x] 3.1 No terminal do usuário, com o aval dele, sem nenhum probe entre os comandos. O comando é o `halo led ram static`, com a sequência do `applyDRAMLEDmode` + `ddr5_static_color_mode.set` do FURY CTRL 2.0.66.0, `docs/hardware.md`, "Modo estático: sequência e proveniência". Nesta ordem:
  - `sudo target/debug/halo led ram static --color 0000ff`: os dois pentes ficam azuis e parados, código 0;
  - `sudo target/debug/halo led ram static --color 00ff00`: verde nos dois;
  - `sudo target/debug/halo led ram static --color ff0000`: vermelho nos dois (com o azul, confirma que R e B não estão trocados);
  - `sudo target/debug/halo led ram static --color 0000ff --brightness 20`: azul mais fraco;
  - `sudo strace -f -e trace=openat,ioctl target/debug/halo led ram static --color 0000ff`: só o `/dev/i2c-0` aberto, `I2C_SLAVE` só com `0x5a` e `0x5b`, exatamente 30 `I2C_SMBUS`, todos escritas, na ordem do `--dry-run`;
  - `sudo target/debug/halo led ram default`: o rainbow volta animado e sincronizado nos dois.

  Se alguma cor não sair como pedida, parar e registrar sem tentar outros bytes.
- [x] 3.2 Registrar o ensaio da 3.1 em `docs/hardware.md`, numa seção nova depois de "Modo estático: sequência e proveniência": cores vistas, códigos de saída, contagem do `strace` e o que o `default` fez. Verificar que a seção cita a data e os comandos exatamente como foram rodados

## 4. Verificação

- [x] 4.1 Rodar `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` e `openspec validate ram-static-color --strict`, todos passando

## Workflow follow-up

- `/cross-review` da branch antes do sync e do archive.
- Sync da spec `ram-led`, archive e commit, só a pedido.
