# Proposal

## Why

O `halo led ram` hoje só apaga, reacende e volta ao rainbow de fábrica. Ainda não dá para escolher a cor. O modo estático do FURY CTRL já foi lido inteiro no código, com origem para cada byte (`docs/hardware.md`, "Modo estático: sequência e proveniência"). É o primeiro controle de verdade da RAM, e o caminho é curto e sem leituras, como o `off`/`on` que já funcionou no aparelho.

## What Changes

- Novo comando `halo led ram static --color RRGGBB [--brightness N]`: uma cor fixa nos dois pentes, com brilho de 0 a 100 (padrão 80, o do FURY CTRL). Ele envia a sequência do `applyDRAMLEDmode` do FURY CTRL 2.0.66.0 em **duas molduras**:
  - na primeira, só o índice de cada pente (`0x0B`);
  - na segunda, por pente: estilo `0x09`=0, direção `0x0C`=1, atraso `0x0D`=0, velocidade `0x0E`=0, número de cores `0x30`=1, número de pentes `0x27`, brilho `0x20` e a cor 0 em `0x31`/`0x32`/`0x33` (R, G, B).

  Com os dois pentes desta máquina, são 30 escritas e nenhuma leitura.
- Cor e brilho são conferidos antes de qualquer acesso. Uma cor que não seja `RRGGBB`, um brilho acima de 100 ou um `--color`/`--brightness` passado a `off`, `on` ou `default` terminam com código 2, sem ler o sysfs nem abrir nenhum `/dev/i2c-*`. O FURY CTRL troca o brilho fora da faixa por 80 sem avisar; o halo recusa.
- `--dry-run` passa a mostrar também as duas molduras do estático, com a cor e o brilho pedidos.
- Os registradores `0x30`, `0x27` e `0x31`–`0x33` entram na lista de escritas permitidas, só com os valores do estático. O resto do protocolo não muda: só `0x58` + slot dos pentes da API v2, nenhuma leitura, 30 tentativas por escrita.
- Fora de escopo:
  - o modo independente (estilo 16, uma cor por LED, `SetKingstonFuryDDR5LedIndex`);
  - outros efeitos e as cores de fundo;
  - mais de uma cor na tabela;
  - o modo de economia de energia;
  - acesso sem root;
  - reaplicar no boot e ao retomar da suspensão (o pente não guarda o estado);
  - daemon, D-Bus e painel;
  - DDR5.

## Capabilities

### New Capabilities
(nenhuma)

### Modified Capabilities
- `ram-led`:
  - "Escritas permitidas" ganha `0x30`, `0x27` e `0x31`–`0x33`;
  - novo requisito para a cor estática em duas molduras;
  - novo requisito de validação da cor e do brilho antes de qualquer acesso;
  - "Simulação" passa a cobrir o estático.

## Impact

- `crates/halo-core` (`ram::controller`):
  - registradores novos no `WriteReg`;
  - o `RamCommand` passa a carregar a cor e o brilho no estático;
  - o `plan` monta uma ou duas molduras conforme o comando;
  - testes golden com o `FakeBus`.
- `crates/halo-cli`: o `led ram` ganha `static`, `--color` e `--brightness`, com testes em `tests/led.rs`.
- Docs: `docs/hardware.md` (o ensaio do estático no aparelho), `README.md` e `docs/roadmap.md` (Fase 5).
- Sem dependências novas.
