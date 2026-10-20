# Spec Delta

## Purpose

Identificar o controlador Castor3 do AORUS Waterforce e ler o firmware dele pelo hidraw, sem alterar o estado do aparelho, para decidir se o protocolo de anel do OpenRGB se aplica a este modelo.

## ADDED Requirements

### Requirement: Pedidos permitidos
O probe SHALL enviar ao Castor3 apenas os pedidos de leitura `D6` (fonte: driver `gigabyte_waterforce` do Linux) e `DE 00` (fonte: `GigabyteCastor3Controller` do OpenRGB), cada um como output report `0x99` completado com zeros até 6144 bytes. O probe MUST NOT enviar comandos de LED, LCD, fan ou bomba.

#### Scenario: Probe com sucesso
- **WHEN** o usuário executa o probe do Waterforce e o hidraw está acessível
- **THEN** o sistema envia exatamente dois output reports, `99 D6` e `99 DE 00`, cada um com 6144 bytes, e nada mais

#### Scenario: Estado preservado
- **WHEN** o probe termina
- **THEN** a cor do anel, a tela e as rotações continuam como antes

### Requirement: Resposta filtrada pelo comando
O sistema SHALL aceitar como resposta de cada pedido só o input report cujo primeiro byte de dados é igual ao comando enviado (`D6` ou `DE`), e descartar os outros, como as respostas de status `DA` pedidas pelo kernel, até o tempo limite de 2 segundos por pedido.

#### Scenario: Status do kernel no meio da leitura
- **WHEN** chega um input report `99 DA ...` antes da resposta `99 D6 ...`
- **THEN** o sistema descarta o `DA` e usa o `D6`

#### Scenario: Sem resposta
- **WHEN** nenhuma resposta com o comando esperado chega em 2 segundos
- **THEN** o sistema informa que aquele pedido não teve resposta, mostra o resultado do outro pedido e termina com código diferente de zero

### Requirement: Campos apresentados
O sistema SHALL mostrar o caminho do hidraw, o nome HID, a versão de firmware segundo o kernel (`data[2] * 10 + data[3]` do input report `99 D6`, contando o report ID como `data[0]`) e a versão segundo o OpenRGB (o byte seguinte ao `DE`).

#### Scenario: Duas versões de firmware
- **WHEN** os input reports são `99 D6 02 05 ...` e `99 DE 02 ...`
- **THEN** a saída mostra firmware 25 (kernel) e 2 (OpenRGB)

### Requirement: Saída bruta
O sistema SHALL oferecer uma opção para mostrar os primeiros 64 bytes de cada resposta em hexadecimal.

#### Scenario: Pedido de saída bruta
- **WHEN** o usuário executa o probe com a opção de saída bruta
- **THEN** a saída inclui o hexdump das respostas `D6` e `DE`

### Requirement: Permissão
O sistema SHALL indicar, quando o hidraw do Waterforce não abrir por falta de permissão, que a regra udev do halo precisa ser instalada.

#### Scenario: Regra não instalada
- **WHEN** o hidraw do `1044:7a4d` existe mas o usuário não pode abri-lo
- **THEN** o sistema informa a falta de permissão, o caminho e onde está a regra udev do halo
