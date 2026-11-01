# Tasks

O halo não envia nenhum comando ao SMBus nesta change. O `halo probe ram` só lê os arquivos `eeprom` que o driver `ee1004` do kernel publica. A troca de página do SPD (send byte em `0x36`/`0x37`) é feita pelo próprio kernel, como no boot (`docs/hardware.md`, "Primeiro boot com o lax"). O critério de suporte vem do `FUN_1000af50` e do `CheckDRAMSupport` do `FuryCTRL.dll` 2.0.66.0, analisados com o REA (`docs/hardware.md`, "Software oficial: FURY CTRL").

## 1. Decodificação no core

- [ ] 1.1 Criar `crates/halo-core/src/ram.rs` (registrado em `lib.rs`) com `decode(&[u8]) -> Result<SpdInfo, DecodeError>`. Ele devolve o erro de tamanho com o número de bytes quando o SPD não tem 512 bytes. Os campos e o veredito seguem a design: `NotDdr4`, `NotKingston`, `Supported { brand, model, name }` e `NotSupported { reason }`, este citando o byte que falhou. O comentário do módulo cita as funções do `FuryCTRL.dll` e as seções do `docs/hardware.md`. Verificar com testes de unidade em `ram.rs`, com um SPD de 512 bytes montado com os bytes reais de 2026-10-31 (2 = `0x0C`, 320–321 = `01 98`, 329–348 = `KF3200C16D4/16GX`, 330 = `0x46`, 359 = `0x04`, 360 = `0x23`): o pente desta máquina dá Fury Beast Refresh suportado. Variações do mesmo SPD cobrem byte 360 = `0x00` (não suportado, cita o 360), marca `H` com modelo 3 e 360 = `0x10` (suportado, modelo desconhecido `0x03`), byte 330 = `0x41` (não suportado, cita o 330), byte 359 = `0x05` (cita o 359), fabricante `80 2c` (não é Kingston), byte 2 = `0x0B` (não é DDR4) e um SPD de 256 bytes (erro com 256), em `cargo test -p halo-core ram`

## 2. Busca no sysfs

- [ ] 2.1 Em `ram.rs`, acrescentar `find(sys_root) -> Result<Vec<SpdSlot>, FindError>`. Ele percorre `<sys>/bus/i2c/devices/i2c-*`, fica com os adaptadores cujo `name` começa com `SMBus PIIX4 adapter` e, dentro deles, com os filhos `<n>-00XX` cujo `name` é `ee1004` e que têm `eeprom`. Cada `SpdSlot` leva o nome do adaptador, a instância, o endereço e o resultado da leitura com `fs::read`: bytes, `NoResponse` com `ENXIO`, `PermissionDenied` com `EACCES`, ou outro erro. O `FindError` distingue "sem adaptador PIIX4" de "adaptador sem ee1004". Verificar com testes de unidade num sysfs falso em `tempfile`:
  - adaptador `SMBus PIIX4 adapter port 0 at 0b00` como `i2c-7`, com `7-0052` e `7-0053`: os dois são achados;
  - adaptador da GPU com um filho `ee1004`: é ignorado;
  - sem adaptador PIIX4: erro de adaptador;
  - adaptador sem filhos: erro de ee1004;
  - um filho com `name` = `spd`: é ignorado.

  O `ENXIO` não se reproduz num arquivo comum, então a conversão de `io::ErrorKind`/`raw_os_error` para `NoResponse` ganha um teste próprio. Tudo em `cargo test -p halo-core ram`

## 3. Comando na CLI

- [ ] 3.1 Em `crates/halo-cli/src/main.rs`, acrescentar `ProbeDevice::Ram { raw }`, cuja ajuda diz que o comando só lê o sysfs do `ee1004` e não abre `/dev/i2c`. A formatação segue a spec "Campos do SPD" e "Veredito": "sem resposta" para `NoResponse`, "sem permissão" para `PermissionDenied`, `--raw` com 32 linhas de 16 bytes e offsets `000`–`1f0`, e os códigos de saída com a precedência da design (`3`; `0` com algum SPD decodificado; senão `4` com algum `EACCES`, ou `5`). A escolha do código fica numa função pura testável. As mensagens de "sem adaptador" e "sem ee1004" citam `acpi_enforce_resources=lax` e a seção de RAM do `docs/hardware.md`. Verificar com testes em `crates/halo-cli/tests/probe.rs`, com `--sys-root` num sysfs falso:
  - dois pentes reais: saída com `KF3200C16D4/16GX`, Kingston, DDR4, "Fury Beast Refresh" e código 0;
  - sem adaptador: código 3 e `acpi_enforce_resources=lax` no stderr;
  - adaptador sem ee1004: código 3 e `ee1004` no stderr;
  - um `eeprom` de 256 bytes ao lado de um bom: o curto aparece com "256 bytes" e o código é 0;
  - só o curto: código 5;
  - `eeprom` com modo `000` ao lado de um bom: "sem permissão" e código 0; só o `000`: código 4. Se o teste rodar como root, onde o modo não bloqueia, ele é pulado com aviso, e a precedência fica coberta pelo teste de unidade da função de código de saída (`[Decoded, PermissionDenied]` → 0, `[PermissionDenied, NoResponse]` → 4, `[NoResponse, Short]` → 5);
  - `--raw`: 32 linhas começando em `000` e `1f0`;
  - `probe ram --help` aparece no teste `ajuda_dos_comandos`.

  Tudo em `cargo test -p halo-cli --test probe`

## 4. Documentação

- [ ] 4.1 Atualizar o `README.md` (comando `halo probe ram` na tabela de hardware e na lista de comandos, com a nota de que exige o lax) e o `docs/roadmap.md` (Fase 5, item 1: a ferramenta só de leitura existe). Verificar que o comando citado aparece no `halo --help` e que o README não fala mais em "Adiado" para a leitura

## 5. Verificação

- [ ] 5.1 Rodar `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` e `openspec validate ram-spd-probe --strict`, todos passando
- [ ] 5.2 No aparelho, no terminal do usuário e com o aval dele, sem sudo: `target/debug/halo probe ram` e `target/debug/halo probe ram --raw`. Esperado: os pentes `0-0052` e `0-0053` como Kingston DDR4 `KF3200C16D4/16GX`, suportados, Fury Beast Refresh, a instância `2-0051` como "sem resposta" (o adaptador `SMBus PIIX4 adapter port 1 at 0b20` entra pelo prefixo) e código 0. Conferir com o `strace -f -e trace=openat` que nenhum `/dev/i2c-*` é aberto. Registrar a saída em `docs/hardware.md` ("Primeiro boot com o lax" ou uma seção nova)

## Workflow follow-up

- `/cross-review` da branch antes do sync e do archive.
- Sync da spec `ram-probe`, archive e commit, só a pedido.
