# Design

## Context

O `ram::controller` (`crates/halo-core/src/ram/controller.rs`) já tem o necessário para escrever nos pentes sem ler nada:

- `Controller` só sai de um SPD confirmado;
- `WriteReg` fecha os registradores;
- `RamCommand` (`Off`, `On`, `Default`) traz uma tabela fixa de escritas por pente;
- `plan()` monta **uma** moldura (`0x53` do maior slot para o menor, 50 ms, as escritas de cada pente do menor para o maior, `0x44` do menor para o maior, 50 ms);
- `Plan` é opaco, e só o `apply()` o executa, com 30 tentativas e 5 ms depois de cada escrita;
- o `FakeBus` dos testes grava as transações.

O `off`/`on` veio do `setDRAMDDR5_AllOff`, que usa uma moldura só. O estático vem de outro fluxo, o `applyDRAMLEDmode`, que usa duas. A sequência inteira, com a fonte de cada byte, está em `docs/hardware.md`, "Modo estático: sequência e proveniência (2026-11-01)". O resumo das fontes está abaixo.

| Escrita | Fonte no FURY CTRL 2.0.66.0 | `FuryCTRL.dll` (Ghidra/REA) | OpenRGB `72262b8` |
|---|---|---|---|
| moldura 1: `0x08`=`0x53`, `0x0B`=índice, `0x08`=`0x44` | `KSDRAMSMBusCtrlObj.cs:221–290` (`applyDRAMLEDmode`, `MasterSlaveRole` na L280) | `CmdStart@0x100187b0`, `MasterSlaveRole@0x100192f0`, `CmdDone@0x100188c0` | `FURY_REG_APPLY`, `FURY_REG_INDEX` |
| `0x09`=`0x00` | `ddr5_static_color_mode.cs:33` | `Style@0x10018f30` | `FURY_REG_MODE`, `FURY_MODE_STATIC` |
| `0x0C`=`0x01` | `ddr5_static_color_mode.cs:7`, `:36` | `Direction@0x10019410` | `FURY_DIR_BOTTOM_TO_TOP` (`RGBController_KingstonFuryDRAM.cpp:449`) |
| `0x0D`=`0x00` | `:9`, `:40` | `IRDelay@0x10019530` | `FURY_REG_DELAY` (`:450`) |
| `0x0E`=`0x00` | `:11`, `:44` | `Speed@0x10019650` | `FURY_REG_SPEED` (`:451`) |
| `0x30`=`0x01` | `:15`, `:48` | `SelectColor@0x1001ad60` | `FURY_REG_NUM_COLORS` |
| `0x27`=número de pentes | `:50`, `KSDRAMSMBusCtrlObj.cs:308` | `NumDimm@0x1001a800` | `FURY_REG_NUM_SLOTS` |
| `0x20`=brilho | `:63`, `Miscellaneous.cs:941` | `Brightness@0x1001a2a0` | `FURY_REG_BRIGHTNESS` |
| 10 ms, depois `0x31`/`0x32`/`0x33` = R/G/B | `Miscellaneous.cs:855`, `:862` (`setDDR5ColorsTable`) | `ColorIndex@0x1001ae80` (`'1' + 3i`, `'2' + 3i`, `'3' + 3i`) | `FURY_REG_MODE_BASE_RED/GREEN/BLUE` (`KingstonFuryDRAMController.cpp:290`) |

## Goals / Non-Goals

**Goals:**
- O plano do estático é a sequência acima, byte a byte, provada por um teste golden contra o `FakeBus`.
- Entrada inválida para antes de qualquer acesso ao sysfs e ao `/dev/i2c-*`.
- O `off`, o `on` e o `default` continuam com exatamente as mesmas transações de hoje.

**Non-Goals:**
- O cache do FURY CTRL (escrever só o que mudou). O halo não guarda estado e sempre manda a sequência inteira, que é o caminho da primeira aplicação do serviço.
- Generalizar a tabela de cores para mais de uma cor ou para o estilo 16.
- Tirar o `Rgb` do módulo do Waterforce e levá-lo para um módulo comum.

## Decisions

- **Duas molduras, como o `applyDRAMLEDmode`.** É o único fluxo do serviço que envia o estático.
  - *Alternativa:* uma moldura com o `0x0B` junto, como o `off`/`on`. Descartada: é uma sequência que o FURY CTRL nunca envia, e economizaria só 6 escritas e cerca de 100 ms.
  - *Consequência:* o `plan()` passa a montar uma lista de molduras. `off`, `on` e `default` continuam com uma; o estático tem duas. A falha em qualquer escrita para tudo, sem `0x44` e sem a moldura seguinte (como hoje e como o FURY CTRL, que não manda o `CmdDone` depois de uma falha).
- **Ordem dentro da moldura igual à do FURY**: todas as escritas de um pente e depois as do outro, e não um registrador por vez em todos os pentes, como faz o OpenRGB. A pausa de 10 ms antes da cor 0 também vem do serviço (`Miscellaneous.cs:862`). As outras pausas são as que o `apply()` já faz.
- **`RamCommand::Static { color, brightness }`**, ainda `Copy`.
  - A tabela de escritas por pente deixa de ser um `&'static [...]`: passa a ser calculada a partir do comando e dos controladores.
  - O valor do `0x27` vem de um novo `Value::DimmCount`, o número de controladores do plano limitado de 1 a 4, como o `CheckValueRange(iNumDimm, 4, 1, 4)`. A cor e o brilho vêm do próprio comando.
  - *Alternativa:* um plano separado só para o estático. Descartada: duplicaria a moldura, as pausas e a ordem, que hoje estão num lugar só.
- **Tipos fecham a faixa.** O brilho é um `RamBrightness` que só se constrói de 0 a 100, e a cor reusa o `waterforce::led::Rgb` (`RRGGBB`, com ou sem `#`, já testado). O `WriteReg` ganha `ColorCount` (`0x30`), `DimmCount` (`0x27`), `Color0Red` (`0x31`), `Color0Green` (`0x32`) e `Color0Blue` (`0x33`), só a cor 0, porque a change só usa uma.
- **Validação na CLI, antes de tudo.**
  - O `--brightness` usa um `value_parser` de 0 a 100 do clap, e o `--color` usa o `parse_color` existente. Os dois dão código 2.
  - As combinações (`static` sem `--color`; `--color` ou `--brightness` com `off`, `on` ou `default`) são conferidas no começo do `led_ram`, antes do `--dry-run` e do sysfs, e também saem com código 2.
- **`--dry-run` e mensagem de sucesso.** O cabeçalho da simulação e o "aplicado em" passam a dizer a cor e o brilho no estático, por exemplo `static 0000ff, brilho 80`.

## Risks / Trade-offs

- [O pente pode não aceitar o estático, ou só parte dele] → O ensaio confere tudo no aparelho, pelo olho e pelo `strace`. O `halo led ram default` volta ao rainbow, e um desligamento com corte de energia volta ao padrão de fábrica (`docs/hardware.md`, "Escritas sem leitura").
- [A cor 0 de fábrica é vermelho (`Miscellaneous.cs:14`), então um estático que perdeu as escritas da cor também fica vermelho] → O ensaio começa por `0000ff` e `00ff00`. O `ff0000` vem por último, e só confirma a ordem dos canais.
- [Moldura 1 completa e moldura 2 interrompida deixam o índice escrito e o modo antigo] → É o mesmo caso de "Falha durante a moldura": a mensagem manda repetir o comando, que refaz as duas molduras.
- [O `dDRAMInfos.Count` do FURY CTRL pode contar pentes que o halo não endereça] → Neste PC os dois pentes são controláveis e o valor é 2 nos dois casos. Um PC com um pente sem RGB ao lado fica sem prova; a design fica com o número de pentes controláveis, que é o que o OpenRGB faz (`SetNumSlots`).

## Migration Plan

Nada a migrar. Os comandos atuais não mudam, e reverter é tirar o `static`.
