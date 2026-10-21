# Design

## Context

- O protocolo está em `docs/protocols/castor3.md`, analisado no GCC 26.09.10.01:
  - leituras: `ucCooler.dll`, `DeviceApi` e `LcdApi`;
  - LED: `GcLedLib.dll` (`block-006`, sha256 `f2f36a21…acb9b3`), funções `dllexp_GvAIOV2LedSet` → `FUN_1800402a0` → `FUN_180040fc0` (`C9`), `FUN_180041470` (`CD`), `FUN_1800410d0` (`B0`–`B3`) e `FUN_180040780` (`B6`), com Ghidra 12.1.4 pelo REA.
- Já existem:
  - `waterforce::probe`, que envia `D6` e `DE 00` e filtra a resposta pelo comando (`wait_response`);
  - `Hidraw::write_output_report`;
  - a regra udev;
  - a CLI `halo probe waterforce [--raw]`.
- O aparelho tem firmware 1.6 e respondeu `DE` → `00` (X 240). O GCC não tem nenhum comando que leia o estado do LED.

## Goals / Non-Goals

**Goals:**
- Montar os pacotes de LED numa função pura, testável sem hardware, e conferir byte a byte contra as tabelas do `castor3.md`.
- Validar o protocolo do GCC no aparelho primeiro só com leituras, e só depois escrever.

**Non-Goals:**
- Quantidade de fans RGB (`C8`). Fica só a leitura `C7`. O GCC mostra 3 como padrão da interface, e escrever um valor errado pode apagar fans.
- Sincronização em tempo real (`GvSetRealTimeColor`), daemon, perfis e interface gráfica (Fase 2).
- Velocidade, bomba e LCD (próximas changes).

## Decisions

### Probe: uma lista de pedidos de leitura no lugar do `FirmwareQuery`

O enum de pedidos vira a lista completa: `D6`, `DE 00`, `DD`, `D9 01`, `D9 02`, `C7` e `E8`. Cada item sabe o próprio pacote e decodifica a resposta. Os bytes de cada um são os do GCC (`DeviceApi.GetMode`, `GetSpeedCurve`, `GetFanQty`, `LcdApi.GetMode`) e do kernel (`D6`). O `wait_response` continua igual: filtra pelo byte 1 e descarta `DA`.

O `D9` repete o byte do tipo na resposta? O GCC só confere os bytes 0 e 1 (`nRecvCheckLength = 1`), então o filtro também não confere o tipo. Os dois `D9` são enviados um de cada vez, cada um esperando a própria resposta.

*Alternativa:* manter o probe de firmware e criar `probe waterforce --gcc`. Rejeitada porque todos os pedidos são de leitura, e um probe só, completo, é o que valida o aparelho.

O `DE` passa a ser mostrado como modelo (`DeviceApi.GetDeviceModelName`). A leitura do OpenRGB como versão estava errada para este aparelho.

### LED: tipos fechados e montagem pura

- `Effect`: um enum com os 14 efeitos da interface (`GLed_0x4016_240.LedEffectId`) e mais o desligado. Os números de fio vêm do `LedEffectConvert`: 01 static, 02 pulse, 03 cycle, 04 flash, 05 double-flash, 06 gradient, 07 color-shift, 08 wave, 0A rainbow, 0B tri-color, 0C spin, 0D switch, 0E lego, 0F aurora. O efeito 09 não aparece na interface e fica de fora.
- `Brightness(1..=10)` e `Speed(1..=5)` são validados na construção. No fio vão `× 10` (`LedApi.LedSettingToConfig`) e `× 2` (`dllexp_GvAIOV2LedSet`).
- `packets(effect, params) -> Vec<Vec<u8>>` devolve os payloads com o report ID `0x99`. A sequência de cada efeito segue o `switch` de `FUN_1800402a0` (tabela "Sequência por efeito" do `castor3.md`), sempre com `nGlobalClr = 1` e `nArea = 1`, como o C# do GCC:
  - `CD` nos efeitos 01, 02, 04 e 05;
  - `CD` e um segundo `C9` com índice `0A` no 06;
  - `C9` com índice `08` e área `02` no 07;
  - `B0`–`B3` no 07 e do 0B ao 0F;
  - só `C9` no 03, 08 e 0A.
- Cores padrão por efeito: as de `CoolerLedProperty.InitLedProperty` (por exemplo, `#ff6600` para cor única).

*Alternativa:* portar o `RGBController_GigabyteCastor3` do OpenRGB. Rejeitada porque ele inverte brilho e velocidade no `C9` e não tem as sequências do 06 e do 07.

### Paleta curta completada com a última cor

O GCC sempre manda 8 slots (`ClrCount = 8`, vindos da interface). Quando o usuário informa menos cores, o halo repete a última, como o OpenRGB faz. *Alternativa:* zerar os slots. Rejeitada porque preto no meio de um efeito de paleta aparece como um buraco apagado. Isso afeta só a aparência, e o aparelho valida qual dos dois funciona melhor.

### Envio

- O comando abre o hidraw do `1044:7a4d` pelo `find_hidraw` e envia `DE 00`.
- Só depois de `r[2] ∈ {0, 1, 2}` ele escreve cada pacote, completado até 6144 bytes, com 1 ms entre eles. É o mesmo intervalo do `Sleep(1)` do GCC.
- O GCC não lê resposta depois dos comandos de LED (`CoolerApi.Send`), e o halo também não lê.
- A escrita fica atrás de uma trait `OutputReports`, para testar a sequência completa com um falso, do mesmo jeito que o `InputReports` do probe.

### CLI

`halo led waterforce <efeito> [--color RRGGBB]... [--brightness 1-10] [--speed 1-5] [--dry-run]`.

- `--color` repetido monta a paleta. Nos efeitos de cor única, só uma cor é aceita.
- `--dry-run` imprime os pacotes e não toca em `/sys` nem em `/dev`.
- Os códigos de saída são os do probe: 3 não encontrado, 4 sem permissão, 5 falha. O 2 (argumento inválido) fica com o clap.

## Risks / Trade-offs

- [A primeira escrita pode não ter o efeito esperado no firmware 1.6, por exemplo se o GCC mandar outra coisa que o Ghidra não mostrou] → a fase de leitura vem antes: se `DD`, `D9`, `C7` ou `E8` não responderem como o GCC espera, a change para antes de qualquer `C9`. A primeira escrita é a cor estática, a mais simples.
- [Não dá para restaurar o modo aleatório de fábrica com exatidão, porque não há leitura de LED] → `cycle` e `rainbow` são os candidatos mais próximos. O primeiro teste no aparelho registra qual dos dois se parece com o original.
- [O `B6` pode gravar na flash] → um `B6` por execução, sem laço, e os testes no aparelho limitados a poucos comandos.
- [O anel e as fans podem não acender juntos, o que foi só inferido no `castor3.md`] → o teste visual da cor estática verifica isso e registra o resultado no `hardware.md`.
- [Input reports `DA` do kernel enchem a fila durante a escrita] → as respostas não são lidas depois dos comandos de LED, e só a do `DE` é lida, pelo filtro existente.

## Migration Plan

Não há estado persistente no halo. Para desfazer no aparelho, aplicar `cycle` ou `rainbow`, ou um efeito qualquer pelo GCC no Windows, se houver.
