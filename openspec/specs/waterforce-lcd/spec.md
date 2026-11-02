# waterforce-lcd Specification

## Purpose
Ler e, nas próximas changes, controlar o LCD 320×320 do AORUS Waterforce (Castor3, `1044:7a4d`) com os mesmos pedidos que o GIGABYTE Control Center envia, começando só pela leitura do estado da tela.

## Requirements

### Requirement: Leituras por modo
O `halo probe waterforce-lcd` SHALL enviar primeiro `99 E8` e, depois, só as leituras que o `LcdControl` do GCC 26.09.10.01 faz para o modo lido (`r[2] - 1`), nesta ordem: modos 0 a 3 e 5, `DF` e `EA <modo+1>`; modo 4, `E9` e `EA <modo+1>`; modo 6, `EA <modo+1>`; modos 7 a 9, a lista de mídia (`F3`, `F4`, `F5`), `FD <modo+1>` e `FA <disco>`; modo 11, `FC`. Em qualquer outro modo, SHALL enviar só o `E8`.

#### Scenario: Modo 3
- **WHEN** a resposta do `E8` é `99 e8 04`
- **THEN** o sistema envia exatamente `99 E8`, `99 DF` e `99 EA 04`, nesta ordem, e nada mais

#### Scenario: Modo 4
- **WHEN** a resposta do `E8` é `99 e8 05`
- **THEN** o sistema envia exatamente `99 E8`, `99 E9` e `99 EA 05`

#### Scenario: Modo de imagem
- **WHEN** a resposta do `E8` é `99 e8 08` (modo 7) e o `F4` informa 2 itens
- **THEN** o sistema envia `99 E8`, `99 F3 01`, `99 F4 01`, `99 F5 01 00`, `99 F5 01 01`, `99 FD 08` e `99 FA 42`, nesta ordem

#### Scenario: Modo sem leituras
- **WHEN** a resposta do `E8` é `99 e8 0b` (modo 10)
- **THEN** o sistema envia só o `99 E8`, diz que o GCC não lê mais nada nesse modo e termina com código 0

### Requirement: Pedidos permitidos
Os únicos pacotes SHALL ser os do requisito "Leituras por modo", cada um como output report `0x99` completado com zeros até 6144 bytes. O sistema MUST NOT enviar `E7`, `E2`, `E4`, `FB`, `F0`, `F1`, `F2`, `F6`, `FE`, `B6`, nem nenhum comando de LED, fan, bomba ou dados de CPU. O `F3` MUST ser enviado só nos modos 7 a 9.

#### Scenario: Nenhuma escrita
- **WHEN** o probe termina, em qualquer modo
- **THEN** nenhum pacote enviado tem como comando `E7`, `E2`, `E4`, `FB`, `F0`, `F1`, `F2`, `F6`, `FE` ou `B6`

### Requirement: Lista de mídia como no GCC
Nos modos 7 a 9, a lista SHALL ser lida como no `LcdApi.GetMediaList`: `99 F3 <modo-6>`, uma espera de 500 ms sem ler resposta, `99 F4 <modo-6>` com a quantidade em `r[3]`, e, para cada item `i`, `99 F5 <modo-6> <i>`, aceitando só a resposta que repete `99 F5 <modo-6> <i>`, com a marca de loop em `r[4]` e o nome UTF-8 a partir de `r[6]`, até o primeiro byte zero. O disco do `FA` SHALL ser `B` (`0x42`) nos modos 7 e 8 e `E` (`0x45`) no modo 9 (`LcdApi.GetDisk`).

#### Scenario: Resposta de outro item
- **WHEN** chega `99 f5 01 00 ...` enquanto o pedido é `99 F5 01 01`
- **THEN** o sistema descarta essa resposta e espera a do item 1

#### Scenario: Vídeo
- **WHEN** o modo lido é 9
- **THEN** o espaço livre é pedido com `99 FA 45`

### Requirement: Resposta filtrada pelo comando
Cada pedido SHALL aceitar só o input report cujo primeiro byte de dados é o comando enviado, descartando os outros (como os `DA` de status do kernel), por até 2 segundos. Se o `E8` não responder, o sistema MUST NOT enviar mais nada, SHALL dizer que o modo não respondeu e SHALL terminar com código 5. Se outra leitura não responder, o sistema SHALL mostrar as outras e terminar com código 5.

#### Scenario: E8 sem resposta
- **WHEN** nenhum `99 e8` chega em 2 segundos
- **THEN** só o `99 E8` foi enviado, a saída diz que o modo não respondeu e o código de saída é 5

#### Scenario: Status do kernel no meio
- **WHEN** chega `99 da ...` antes de `99 df ...`
- **THEN** o sistema descarta o `DA` e usa o `DF`

### Requirement: Nome do modo
O sistema SHALL mostrar o caminho do hidraw e o modo com o número e o nome do `DisplayMode` do GCC: 0 a 4, `Faith1` a `Faith5`; 5, `Function`; 6, `Text`; 7, `Image`; 8, `Gif`; 9, `Video`; 10, `Pet`; 11, `Carousel`; outro número, "desconhecido".

#### Scenario: Modo 3
- **WHEN** a resposta do `E8` é `99 e8 04`
- **THEN** a saída mostra o modo 3, `Faith4`

### Requirement: Itens exibidos
Para o `DF`, o sistema SHALL listar os itens ligados pelos bytes `r[2..10]` iguais a 1, com os nomes do `enum Display` do GCC na ordem dos bits (`FanRPM`, `CPUClock`, `PumpRPM`, `CPUName`, `CPUTemperature`, `CPUThread`, `LiquidTemperature`, `RAMTemperature`, `Fahrenheit`), dizer quando nenhum está ligado e mostrar o `r[11]` cru.

#### Scenario: Tela sem itens
- **WHEN** a resposta do `DF` tem `r[2..10]` todos em 0
- **THEN** a saída diz que nenhum item está ligado

#### Scenario: Itens ligados
- **WHEN** a resposta do `DF` tem 1 em `r[6]` e `r[8]`
- **THEN** a saída lista `CPUTemperature` e `LiquidTemperature`

### Requirement: Cores das zonas
Para o `EA`, o sistema SHALL mostrar as duas cores, `r[3..5]` e `r[6..8]`, em `RRGGBB` (`RGBUtil(r, g, b)` do `LcdApi.GetZoneColor`).

#### Scenario: Duas zonas
- **WHEN** a resposta do `EA` é `99 ea 04 ff 00 00 00 00 ff`
- **THEN** a saída mostra a zona 1 em `ff0000` e a zona 2 em `0000ff`

### Requirement: Saída bruta e permissão
Com `--raw`, o sistema SHALL mostrar também os primeiros 64 bytes de cada resposta recebida. Sem acesso ao hidraw, SHALL dizer que a regra udev do halo precisa ser instalada e terminar com código 4. Sem o aparelho, SHALL terminar com código 3 sem enviar nada.

#### Scenario: Sem o Waterforce
- **WHEN** não há hidraw do `1044:7a4d`
- **THEN** nada é enviado e o código de saída é 3

### Requirement: Tempo máximo com o aparelho
O probe SHALL manter o hidraw aberto, e com ele o lock exclusivo, por no máximo 10 segundos, para que o feed e a proteção do `halod` (lock de 5 s, watchdog de 20 s) consigam o aparelho a tempo. Depois do `E8`, cada pedido SHALL sair só se ainda houver prazo, com a espera limitada ao que sobrar. Esgotado o prazo, o sistema SHALL mostrar o que leu, dizer que o prazo acabou e terminar com código 5.

#### Scenario: Prazo esgotado
- **WHEN** o prazo acaba antes das leituras do modo
- **THEN** nenhum pedido seguinte é enviado, a saída diz que o prazo acabou e o código de saída é 5

### Requirement: Item de mídia sem resposta
Na lista de mídia, o primeiro `F5` sem resposta SHALL encerrar a lista; o sistema SHALL seguir com o `FD` e o `FA` e terminar com código 5.

#### Scenario: Lista muda
- **WHEN** o `F4` informa 30 arquivos e o primeiro `F5` não responde
- **THEN** só um `F5` é enviado, o `FD` e o `FA` seguem, e o código de saída é 5
