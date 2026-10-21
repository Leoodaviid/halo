# Proposal

## Why

A análise do GIGABYTE Control Center (`docs/protocols/castor3.md`) confirmou no código oficial como o Waterforce X 240 (`1044:7a4d`) recebe cor e efeito do anel e das fans. Até agora o halo só lê o aparelho, e as fans continuam no modo aleatório de fábrica. Esta change valida o protocolo do GCC no aparelho com leituras e depois dá ao halo o primeiro controle de LED.

## What Changes

- O `halo probe waterforce` passa a enviar também os pedidos de leitura do GCC: `DD` (modo da fan e da bomba), `D9 01` e `D9 02` (curvas), `C7` (quantidade de fans RGB) e `E8` (modo do LCD). Assim dá para conferir, sem escrever nada, que o firmware 1.6 entende o protocolo do GCC.
- O probe passa a mostrar o byte do `DE` como **modelo** (0 = X 240, 1 = X 280, 2 = X 360), como faz o GCC, e não mais como versão do OpenRGB. **BREAKING** na saída do probe.
- Novo comando `halo led waterforce <efeito>`, que aplica um efeito com cor, paleta, brilho e velocidade. Ele envia só `C9`, `CD`, `B0`–`B3` e `B6`, no layout de bytes da `GcLedLib.dll` do GCC: byte 2 do `C9` = brilho, byte 3 = velocidade (o OpenRGB inverte os dois).
- Antes de escrever, o comando confirma o modelo com `DE 00`.
- Cada execução manda um único `B6` no fim, e não existe modo repetido ou animação feita pelo software.
- `--dry-run` mostra os pacotes sem abrir o aparelho.
- Os efeitos `cycle` e `rainbow` servem para voltar a um modo de cores variadas parecido com o de fábrica, já que o protocolo não tem leitura do estado do LED.

## Capabilities

### New Capabilities
- `waterforce-led`: aplicar cor e efeito ao anel e às fans RGB do Waterforce pelo hidraw, com os comandos e o layout confirmados no GCC.

### Modified Capabilities
- `waterforce-probe`: aceita os pedidos de leitura do GCC (`DD`, `D9`, `C7`, `E8`), além de `D6` e `DE 00`, e apresenta o `DE` como modelo.

## Impact

- `crates/halo-core/src/waterforce/`: probe com mais pedidos e um módulo novo de LED, que monta os pacotes e envia.
- `crates/halo-cli`: novo subcomando `led waterforce` e saída do probe atualizada, com os testes de integração.
- `docs/hardware.md`, `docs/protocols/castor3.md` e `README.md`: respostas reais das leituras novas, resultado visual do LED e comandos de uso.
- Hardware: a primeira escrita no Waterforce. O estado aleatório de fábrica não pode ser lido antes nem restaurado com exatidão.
