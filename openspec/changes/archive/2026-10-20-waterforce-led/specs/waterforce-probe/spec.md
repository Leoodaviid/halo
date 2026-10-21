# Spec Delta

## MODIFIED Requirements

### Requirement: Pedidos permitidos
O probe SHALL enviar ao Castor3 apenas pedidos de leitura, nesta ordem: `D6` (driver `gigabyte_waterforce` do Linux), `DE 00`, `DD`, `D9 01`, `D9 02`, `C7` e `E8` (`DeviceApi` e `LcdApi` do `ucCooler.dll` do GCC 26.09.10.01). Cada um vai como output report `0x99` completado com zeros até 6144 bytes. O probe MUST NOT enviar comandos de LED, LCD, fan ou bomba.

#### Scenario: Probe com sucesso
- **WHEN** o usuário executa o probe do Waterforce e o hidraw está acessível
- **THEN** o sistema envia exatamente sete output reports, `99 D6`, `99 DE 00`, `99 DD`, `99 D9 01`, `99 D9 02`, `99 C7` e `99 E8`, cada um com 6144 bytes, e nada mais

#### Scenario: Estado preservado
- **WHEN** o probe termina
- **THEN** a cor do anel, a tela, os modos e as rotações continuam como antes

### Requirement: Resposta filtrada pelo comando
O sistema SHALL aceitar como resposta de cada pedido só o input report cujo primeiro byte de dados é igual ao comando enviado, e descartar os outros, como as respostas de status `DA` pedidas pelo kernel, até o tempo limite de 2 segundos por pedido.

#### Scenario: Status do kernel no meio da leitura
- **WHEN** chega um input report `99 DA ...` antes da resposta `99 D6 ...`
- **THEN** o sistema descarta o `DA` e usa o `D6`

#### Scenario: Sem resposta
- **WHEN** nenhuma resposta com o comando esperado chega em 2 segundos
- **THEN** o sistema informa que aquele pedido não teve resposta, mostra o resultado dos outros pedidos e termina com código diferente de zero

### Requirement: Saída bruta
O sistema SHALL oferecer uma opção para mostrar os primeiros 64 bytes de cada resposta em hexadecimal.

#### Scenario: Pedido de saída bruta
- **WHEN** o usuário executa o probe com a opção de saída bruta
- **THEN** a saída inclui o hexdump de cada uma das sete respostas que chegaram

## ADDED Requirements

### Requirement: Campos do probe
O sistema SHALL mostrar o caminho do hidraw, o nome HID, o firmware (`r[2].r[3]` do `D6`, contando o report ID como `r[0]`), o modelo pelo `r[2]` do `DE`, os modos da fan e da bomba (`r[2]`, `r[3]` do `DD`), as curvas de 4 pontos do `D9`, a quantidade de fans RGB (`r[2]` do `C7`) e o modo do LCD (`r[2] - 1` do `E8`).

#### Scenario: Firmware e modelo
- **WHEN** os input reports são `99 D6 01 06 ...` e `99 DE 00 ...`
- **THEN** a saída mostra firmware 1.6 e modelo Waterforce X 240

#### Scenario: Modelo desconhecido
- **WHEN** o `r[2]` do `DE` não é 0, 1 nem 2
- **THEN** a saída mostra o valor bruto e indica modelo desconhecido

#### Scenario: Modos e curva
- **WHEN** a resposta do `DD` é `99 DD 02 00 ...` e a do `D9 01` traz em `r[4..15]` os pontos `00 04 B0`, `1E 04 B0`, `32 05 78`, `41 07 08`
- **THEN** a saída mostra a fan no modo Default, a bomba em Balanced e a curva da fan 0 °C 1200 RPM, 30 °C 1200 RPM, 50 °C 1400 RPM, 65 °C 1800 RPM

#### Scenario: Modo FixedRPM lido como Balanced
- **WHEN** o `r[2]` ou o `r[3]` do `DD` é 3
- **THEN** a saída mostra o modo como Balanced, como o GCC faz, e mostra o valor bruto 3

#### Scenario: Quantidade de fans e LCD
- **WHEN** as respostas são `99 C7 02 ...` e `99 E8 08 ...`
- **THEN** a saída mostra 2 fans RGB e o LCD no modo 7

## REMOVED Requirements

### Requirement: Campos apresentados
**Reason**: o GCC mostra que o byte seguinte ao `DE` é o índice do modelo (`DeviceApi.GetDeviceModelName`), e não a versão de firmware que o OpenRGB lê. O probe também passa a mostrar modos, curvas, fans RGB e modo do LCD.
**Migration**: substituído por "Campos do probe". O firmware continua no `D6`, agora como maior.menor (1.6 em vez de 16).
