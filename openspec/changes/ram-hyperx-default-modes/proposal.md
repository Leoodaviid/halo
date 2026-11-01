# Proposal

## Why

O primeiro plano desta change usava o controlador da API v1 do FURY CTRL em `0x27`, mas o receive byte em `0x27` deu `ENXIO` no aparelho (2026-11-01). Relendo o serviço e o `FuryCTRL.dll` 2.0.66.0, o Fury Beast Refresh DDR4 destes pentes usa a **API v2**: um controlador por pente em `0x58` + slot do SPD (aqui `0x5A` e `0x5B`), com os registradores do `KingstonFuryDRAMController` do OpenRGB (`docs/hardware.md`, "Receive byte em `0x27`" e "Protocolo do Beast Refresh"). Falta saber se esses controladores respondem às leituras do FURY CTRL e se a moldura de escrita dele funciona neste pente. Esta change responde as duas, de novo com o menor conjunto de transações.

## What Changes

- Novo comando só de leitura `halo probe ram-controller`. Para cada pente Kingston suportado (pelo SPD lido no sysfs), ele lê o controlador em `0x58` + slot com as mesmas leituras do FURY CTRL: read word data nos registradores `0x0B` (papel mestre/escravo), `0x09` (estilo do efeito) e `0x0E` (velocidade), com o valor no byte alto. Mostra os valores, com o nome do estilo quando ele é conhecido. Não escreve nada.
- Novo comando de escrita `halo led ram <off|on|default>`. Os três usam a moldura do FURY CTRL, `0x08` = `0x53` em cada pente, as escritas e depois `0x08` = `0x44` em cada pente:
  - `off`: `setDRAMDDR5_AllOff`, ou seja, o papel lido de volta em `0x0B` e o brilho `0x20` = 0. O estilo atual não é tocado;
  - `on`: a mesma moldura com o brilho `0x20` = 80, o brilho padrão do FURY CTRL, para voltar ao efeito que estava;
  - `default`: `setDRAMDDR5_DefaultRainbow`, o efeito que o FURY CTRL aplica ao "restaurar o padrão" na API v2.

  Com `--dry-run`, mostra as transações sem abrir o nó.
- Antes de qualquer escrita, o comando lê o papel (`0x0B`) de cada pente. Sem pente suportado, ou sem resposta de algum controlador, ele não escreve nada.
- Os endereços saem só dos slots de SPD com pente suportado (`0x58` + slot, de `0x58` a `0x5B`). Os registradores e os valores escritos são fixos no código. Não há varredura, endereço vindo do usuário nem `I2C_SLAVE_FORCE`.
- Os comandos abrem o `/dev/i2c-N` e rodam com `sudo`. As permissões e a regra 72 não mudam.
- Fora de escopo:
  - os outros efeitos, cores, velocidade e direção;
  - o modo de economia de energia;
  - o `0x27` e a API v1;
  - daemon, D-Bus, painel e perfis;
  - acesso sem root;
  - reaplicar o efeito no boot;
  - DDR5.

## Capabilities

### New Capabilities
- `ram-led`: leitura do controlador RGB de cada pente Kingston Fury DDR4 da API v2 (`0x58` + slot) e as três escritas mínimas do FURY CTRL (apagar, reacender e voltar ao padrão).

### Modified Capabilities
(nenhuma; a `ram-probe` continua só lendo o sysfs)

## Impact

- `crates/halo-core`: o `ram::SpdSlot` passa a levar o número do adaptador. Módulo novo para o controlador, com o acesso ao `/dev/i2c-N` atrás de um trait, para testar sem hardware. Sem dependências novas além do `libc`.
- `crates/halo-cli`: subcomandos `probe ram-controller` e `led ram`, com testes num sysfs falso e um nó falso.
- Docs: `docs/hardware.md` (as leituras, os ensaios de escrita e a persistência), `docs/permissions.md` (por que `sudo`), `README.md` e `docs/roadmap.md` (Fase 5, item 2).
- Hardware: só transações em `0x58` + slot dos pentes suportados (hoje `0x5A` e `0x5B`), por um processo root iniciado pelo usuário. O resto do barramento (VRM em `0x60`, DACs em `0x10` e `0x15`, SPD, `0x27`) nunca é endereçado.
- O nome da change ainda diz `hyperx`, do plano antigo. O conteúdo é o da API v2 da Fury.
