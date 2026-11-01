# Spec Delta

## MODIFIED Requirements

### Requirement: Escritas permitidas
As únicas escritas SHALL ser write byte data em `0x08` com `0x53` ou `0x44`, em `0x0B` com o índice calculado do requisito "Índice do pente", em `0x20`, `0x09`, `0x0D`, `0x0C`, `0x0E`, `0x10`, `0x11` e `0x1E` só com os valores dos requisitos "Apagar", "Reacender", "Padrão do FURY CTRL" e "Cor estática", e em `0x30`, `0x27`, `0x31`, `0x32` e `0x33` só com os valores do requisito "Cor estática" (funções `SetKingstonFuryDDR5*` do `FuryCTRL.dll` 2.0.66.0).

#### Scenario: Nenhum registrador fora da lista
- **WHEN** o usuário executa `halo led ram default` com sucesso
- **THEN** todas as escritas foram nos registradores da lista, com os valores do requisito "Padrão do FURY CTRL" ou da moldura

#### Scenario: Estático só com os registradores dele
- **WHEN** o usuário executa `halo led ram static --color 0000ff` com sucesso
- **THEN** todas as escritas foram em `0x08`, `0x0B`, `0x09`, `0x0C`, `0x0D`, `0x0E`, `0x30`, `0x27`, `0x20`, `0x31`, `0x32` e `0x33`

### Requirement: Simulação
Com `--dry-run`, o `halo led ram` SHALL mostrar as transações do comando, uma por linha, com o endereço, a operação, o registrador e o valor, usando os pentes `0x5A` e `0x5B` e os índices calculados para eles (`0x01` e `0x00`). Ele SHALL terminar com código 0 sem ler o sysfs e sem abrir nenhum `/dev/i2c-*`.

#### Scenario: Simulação do apagado
- **WHEN** o usuário executa `halo led ram off --dry-run`
- **THEN** a saída lista a moldura e as escritas do apagado com `0x0B`=`0x01` e `0x00`, sem nenhuma leitura, e nenhum nó é aberto

#### Scenario: Simulação do estático
- **WHEN** o usuário executa `halo led ram static --color 12ab34 --brightness 50 --dry-run`
- **THEN** a saída lista as duas molduras do estático com `0x20`=`0x32`, `0x31`=`0x12`, `0x32`=`0xAB` e `0x33`=`0x34` em cada pente, sem nenhuma leitura, e nenhum nó é aberto

#### Scenario: Modo inválido
- **WHEN** o usuário executa `halo led ram purple`
- **THEN** o código de saída é 2 e nenhum `/dev/i2c-*` é aberto

## ADDED Requirements

### Requirement: Cor estática
O `halo led ram static --color RRGGBB` SHALL escrever, em cada pente e dentro da segunda moldura, nesta ordem, como o `ddr5_static_color_mode.set` do FURY CTRL no modo `ctrl_color`: `0x09`=`0x00`, `0x0C`=`0x01`, `0x0D`=`0x00`, `0x0E`=`0x00`, `0x30`=`0x01`, `0x27`=número de pentes, `0x20`=brilho, e então `0x31`=R, `0x32`=G e `0x33`=B da cor pedida, sem escala.

#### Scenario: Escritas de um pente
- **WHEN** o usuário executa `halo led ram static --color 0000ff` com os pentes em `0x5A` e `0x5B`
- **THEN** a segunda moldura tem, para `0x5A` e depois para `0x5B`, `0x09`=`0x00`, `0x0C`=`0x01`, `0x0D`=`0x00`, `0x0E`=`0x00`, `0x30`=`0x01`, `0x27`=`0x02`, `0x20`=`0x50`, `0x31`=`0x00`, `0x32`=`0x00` e `0x33`=`0xFF`

#### Scenario: Cor sem escala
- **WHEN** o usuário pede a cor `ff1a1b`
- **THEN** os valores escritos são `0x31`=`0xFF`, `0x32`=`0x1A` e `0x33`=`0x1B`

### Requirement: Estático em duas molduras
O `halo led ram static` SHALL enviar duas molduras completas (requisito "Moldura de escrita"), como o `applyDRAMLEDmode` do FURY CTRL: a primeira só com `0x0B` = índice de cada pente; a segunda com as escritas do requisito "Cor estática". A segunda moldura MUST NOT começar se alguma escrita da primeira falhar. Com dois pentes, são 30 escritas.

#### Scenario: Ordem das molduras
- **WHEN** o usuário executa `halo led ram static --color 0000ff` com os pentes em `0x5A` e `0x5B`
- **THEN** as seis primeiras escritas são `0x5B` `0x08`=`0x53`, `0x5A` `0x08`=`0x53`, `0x5A` `0x0B`=`0x01`, `0x5B` `0x0B`=`0x00`, `0x5A` `0x08`=`0x44` e `0x5B` `0x08`=`0x44`, seguidas da segunda moldura, num total de 30 escritas

#### Scenario: Falha na primeira moldura
- **WHEN** a escrita `0x5B` `0x0B` falha nas 30 tentativas
- **THEN** nenhuma escrita seguinte é enviada, nem o `0x44` nem a segunda moldura, e o código de saída é 5

### Requirement: Número de pentes
O valor escrito em `0x27` SHALL ser o número de pentes controláveis endereçados pelo comando, de 1 a 4, como o `CheckValueRange(dDRAMInfos.Count, 4, 1, 4)` do FURY CTRL.

#### Scenario: Dois pentes
- **WHEN** os pentes controláveis são os slots 2 e 3
- **THEN** cada pente recebe `0x27` = `0x02`

#### Scenario: Um pente
- **WHEN** o único pente controlável é o slot 3
- **THEN** o pente recebe `0x27` = `0x01` e `0x0B` = `0x00`

### Requirement: Brilho do estático
O brilho do `halo led ram static` SHALL vir de `--brightness`, um inteiro de 0 a 100 escrito sem escala em `0x20`. Sem `--brightness`, SHALL ser 80 (`0x50`), o `_brightness` do FURY CTRL.

#### Scenario: Sem brilho
- **WHEN** o usuário executa `halo led ram static --color 00ff00`
- **THEN** cada pente recebe `0x20` = `0x50`

#### Scenario: Brilho zero
- **WHEN** o usuário executa `halo led ram static --color 00ff00 --brightness 0`
- **THEN** cada pente recebe `0x20` = `0x00`

### Requirement: Entrada recusada antes de qualquer acesso
O sistema SHALL terminar com código 2, sem ler o sysfs e sem abrir nenhum `/dev/i2c-*`, quando: `static` vier sem `--color`; a cor não for `RRGGBB` (com ou sem `#`); o brilho não for um inteiro de 0 a 100; ou `--color` ou `--brightness` vierem com `off`, `on` ou `default`. O sistema MUST NOT trocar um valor fora da faixa por um padrão.

#### Scenario: Brilho acima de 100
- **WHEN** o usuário executa `halo led ram static --color 0000ff --brightness 101`
- **THEN** o código de saída é 2 e nenhum `/dev/i2c-*` é aberto

#### Scenario: Cor inválida
- **WHEN** o usuário executa `halo led ram static --color 00ff0`
- **THEN** o código de saída é 2 e nenhum `/dev/i2c-*` é aberto

#### Scenario: Estático sem cor
- **WHEN** o usuário executa `halo led ram static`
- **THEN** o código de saída é 2 e nenhum `/dev/i2c-*` é aberto

#### Scenario: Cor em outro comando
- **WHEN** o usuário executa `halo led ram off --color 0000ff`
- **THEN** o código de saída é 2 e nenhum `/dev/i2c-*` é aberto
