# Spec Delta

## MODIFIED Requirements

### Requirement: Simulação
Com `--dry-run`, o `halo led ram` SHALL mostrar as transações do comando, uma por linha, com o endereço, a operação, o registrador e o valor, usando os pentes `0x5A` e `0x5B` e os índices calculados para eles (`0x01` e `0x00`). Ele SHALL terminar com código 0 sem ler o sysfs e sem abrir nenhum `/dev/i2c-*`. No `restore`, a simulação SHALL mostrar as transações do estado salvo (requisito "Reaplicar o estado" da `ram-persistence`), lendo e validando só o arquivo de estado; no `suspend`, as transações do `off`.

#### Scenario: Simulação do apagado
- **WHEN** o usuário executa `halo led ram off --dry-run`
- **THEN** a saída lista a moldura e as escritas do apagado com `0x0B`=`0x01` e `0x00`, sem nenhuma leitura, e nenhum nó é aberto

#### Scenario: Simulação do estático
- **WHEN** o usuário executa `halo led ram static --color 12ab34 --brightness 50 --dry-run`
- **THEN** a saída lista as duas molduras do estático com `0x20`=`0x32`, `0x31`=`0x12`, `0x32`=`0xAB` e `0x33`=`0x34` em cada pente, sem nenhuma leitura, e nenhum nó é aberto

#### Scenario: Simulação do restore
- **WHEN** o estado salvo é `static` `ffffff` com brilho 80 e o usuário executa `halo led ram restore --dry-run`
- **THEN** a saída diz que reaplicaria `static ffffff, brilho 80` e lista as duas molduras com `0x31`=`0xFF`, `0x32`=`0xFF` e `0x33`=`0xFF`, e nenhum nó é aberto

#### Scenario: Modo inválido
- **WHEN** o usuário executa `halo led ram purple`
- **THEN** o código de saída é 2 e nenhum `/dev/i2c-*` é aberto

### Requirement: Entrada recusada antes de qualquer acesso
O sistema SHALL terminar com código 2, sem ler o sysfs e sem abrir nenhum `/dev/i2c-*`, quando: `static` vier sem `--color`; a cor não for `RRGGBB` (com ou sem `#`); o brilho não for um inteiro de 0 a 100; ou `--color` ou `--brightness` vierem com `off`, `on`, `default`, `restore` ou `suspend`. O sistema MUST NOT trocar um valor fora da faixa por um padrão.

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

#### Scenario: Cor no restore
- **WHEN** o usuário executa `halo led ram restore --color 0000ff`
- **THEN** o código de saída é 2, o arquivo de estado não é lido e nenhum `/dev/i2c-*` é aberto
