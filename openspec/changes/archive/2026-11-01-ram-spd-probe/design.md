# Design

## Context

O kernel 7.0, com o lax, registra o `piix4_smbus` e o `i2c_register_spd_write_enable()` instancia o `ee1004` em cada pente que o DMI lista. Layout real observado em 2026-10-31:

- `/sys/bus/i2c/drivers/ee1004/0-0052` → `/sys/devices/pci0000:00/0000:00:14.0/i2c-0/0-0052`, com `eeprom` (512 bytes, modo `444`) e `name` = `ee1004`;
- o diretório pai (`.../i2c-0`) tem `name` = `SMBus PIIX4 adapter port 0 at 0b00`;
- instâncias: `0-0052`, `0-0053` (os pentes) e `2-0051` (adaptador `SMBus PIIX4 adapter port 1 at 0b20`, o ASF), em que a leitura do `eeprom` dá `ENXIO`.

O critério de suporte vem da análise do `FuryCTRL.dll` 2.0.66.0 com o REA (Ghidra 12.1.4), em `tools/samples/kingston-fury-ctrl/analysis/decomp-FuryCTRL/functions/`:
- `0x1000af50.c` (`FUN_1000af50`): lê os bytes `0x14A`, `0x167` e `0x168` do SPD e aceita `0x10`, `0x20`, `0x22` e `0x23` no último;
- `0x1000f9a0.c` (`CheckDRAMSupport`): exige `KINGSTON` no fabricante e dá os nomes por marca e modelo.

Ambos estão resumidos em `docs/hardware.md`, "Software oficial: FURY CTRL". As CLIs atuais recebem a raiz do sysfs pelo `--sys-root` oculto (padrão `/sys`), e os testes de `crates/halo-cli/tests/probe.rs` montam árvores falsas em `tempfile`.

## Goals / Non-Goals

**Goals:**
- Decodificação pura e testável a partir de um `[u8; 512]`, separada da busca no sysfs.
- Mesma lógica de busca que funcione com o sysfs real e com uma árvore falsa sem symlinks.

**Non-Goals:**
- Ler o SPD por `/dev/i2c-*` ou por `i2c-tools`, mesmo como alternativa.
- Interpretar o resto do SPD (timings, XMP, byte 364 e os ASCII de 368–375).
- Instalar regra udev ou mexer no grupo `i2c`.

## Decisions

- **Módulo `halo_core::ram`, com duas partes: `find` (sysfs) e `decode` (bytes → `SpdInfo`).** Segue o padrão do `it5702` e do `g502`, em que a lógica fica no core e a CLI só formata. *Alternativa:* tudo na CLI. Descartada porque o daemon vai precisar da mesma busca quando a Fase 5 chegar a ele.
- **Busca a partir dos adaptadores, não do diretório do driver.** O sistema percorre `<sys>/bus/i2c/devices/i2c-*`, lê `name`, fica com os que começam por `SMBus PIIX4 adapter` e, dentro de cada um, com os filhos `<n>-00XX` cujo `name` é `ee1004` e que têm `eeprom`. Isso evita depender de symlinks (`driver`, `bus/i2c/drivers/ee1004`) que o sysfs falso teria de imitar, e casa com o requisito de achar o barramento pelo nome. *Alternativa:* listar `bus/i2c/drivers/ee1004/*` e resolver o pai com `canonicalize`. Descartada porque exige symlinks reais nos testes e falha de outro jeito quando o driver não está carregado. No sysfs real, `bus/i2c/devices/i2c-N` é um symlink para o adaptador, e `read_dir` e `join` funcionam igual.
- **Leitura com `std::fs::read`, sem `O_RDWR` e sem `mmap`.** O `ee1004` faz a troca de página (send byte em `0x36`/`0x37`) dentro do kernel, como já faz no boot. O halo não manda nada ao barramento por conta própria. Um `ENXIO` vira "sem resposta", e um `EACCES` vira "sem permissão" (improvável, porque o `eeprom` é `444`, mas possível com um sysfs montado de outro jeito). Os dois só afetam aquela instância.
- **Fabricante pelo par `01 98`, e não por uma tabela JEDEC.** O FURY CTRL usa uma tabela de nomes (`0x01;AMD;…`) e procura `KINGSTON`. Para o halo, basta o código de banco 2 com paridade (`01`) e `0x98`, que foi o lido nos dois pentes. Outros fabricantes aparecem em hexadecimal. *Alternativa:* embutir a tabela JEDEC. Descartada porque é escopo maior sem uso agora.
- **O veredito é um enum (`Supported { brand, model, name }`, `NotSupported { reason }`, `NotKingston`, `NotDdr4`).** Assim os testes do core comparam valores, e a CLI fica com o texto.
- **Códigos de saída, nesta ordem de precedência:**
  - `3` sem adaptador ou sem `ee1004`;
  - `0` se algum SPD foi decodificado, mesmo com outras instâncias falhando;
  - se nenhum foi, `4` quando alguma falhou com `EACCES`, e `5` nos outros casos.

  Assim um sucesso parcial não vira erro, e a falta de permissão continua usando o mesmo `4` das outras probes. São os mesmos `EXIT_*` de `main.rs`.
- **Fixtures:** dois SPDs de 512 bytes montados nos testes, com os bytes 2, 320–321, 329–348, 330, 359, 360 e 364 iguais aos lidos em 2026-10-31 e o resto zerado. Não é preciso guardar o dump real no repositório (ele tem número de série nos bytes 323–328).

## Risks / Trade-offs

- [O critério do FURY CTRL pode mudar em outra versão] → A spec e o código citam a versão 2.0.66.0 e as funções analisadas. Um veredito "não suportado" diz qual byte falhou, para facilitar a comparação.
- [Outro kernel pode não instanciar o `ee1004` (DMI sem o slot, módulo ausente)] → Mensagem própria com o código 3, sem fallback para `/dev/i2c-*`.
- [O nome do adaptador pode mudar entre versões do `i2c-piix4`] → O prefixo `SMBus PIIX4 adapter` existe desde o suporte ao SB800 e aparece em `piix4_add_adapter` (`i2c-piix4.c` v7.0). Se mudar, a falha é visível (código 3), e não silenciosa.
- [Ler o `eeprom` gera tráfego no SMBus feito pelo kernel] → É o mesmo caminho do boot e acontece fora das janelas do firmware (`docs/hardware.md`, "SMM do BIOS"). A ferramenta não tem modo de repetição.

## Migration Plan

Não há migração. O comando é novo e não muda os outros. Para desfazer, basta tirar o subcomando.
