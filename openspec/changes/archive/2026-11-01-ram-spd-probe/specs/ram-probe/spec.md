# Spec Delta

## Purpose

Ler, só pelo sysfs do kernel, o SPD dos pentes DDR4 ligados ao SMBus da placa e dizer se o RGB deles é do tipo que o software oficial da Kingston controla, antes de qualquer comando ao controlador RGB.

## ADDED Requirements

### Requirement: Só leitura pelo sysfs
O comando `halo probe ram` SHALL obter os dados apenas lendo os arquivos `eeprom`, `name` e os vínculos de driver que o kernel publica no sysfs. Ele MUST NOT abrir nenhum `/dev/i2c-*`, MUST NOT escrever em nenhum arquivo e MUST NOT exigir root nem o grupo `i2c`.

#### Scenario: Nenhum nó de barramento é aberto
- **WHEN** o usuário executa `halo probe ram`
- **THEN** o processo não abre nenhum caminho `/dev/i2c-*` e só lê arquivos sob a raiz do sysfs

#### Scenario: Usuário comum
- **WHEN** o usuário, fora do grupo `i2c`, executa o comando com os pentes visíveis no sysfs
- **THEN** o comando lê o SPD e termina com código 0

### Requirement: Barramento achado pelo nome do adaptador
O sistema SHALL considerar só as instâncias do driver `ee1004` cujo adaptador I2C pai tem um nome começando com `SMBus PIIX4 adapter`. Ele MUST NOT depender do número `i2c-N` do adaptador e SHALL mostrar, para cada pente, o nome do adaptador e o nome da instância (`<barramento>-<endereço>`).

#### Scenario: Numeração diferente
- **WHEN** o adaptador `SMBus PIIX4 adapter port 0 at 0b00` aparece como `i2c-7` em vez de `i2c-0`
- **THEN** o comando acha e lê os mesmos pentes

#### Scenario: Instância em outro adaptador
- **WHEN** existe uma instância do `ee1004` num adaptador cujo nome não começa com `SMBus PIIX4 adapter`
- **THEN** o comando a ignora

### Requirement: SMBus indisponível
Quando nenhum adaptador `SMBus PIIX4 adapter` existe, o sistema SHALL explicar que o SMBus não está registrado e citar o parâmetro `acpi_enforce_resources=lax` e a seção de RAM de `docs/hardware.md`, e terminar com o código 3. Quando o adaptador existe mas nenhuma instância do `ee1004` está ligada a ele, o sistema SHALL dizer que o driver `ee1004` não está carregado ou não achou pentes, e terminar com o código 3. Nos dois casos, a mensagem MUST NOT sugerir comandos que escrevam no barramento.

#### Scenario: Lax desligado
- **WHEN** o sysfs não tem nenhum adaptador `SMBus PIIX4 adapter`
- **THEN** a mensagem cita `acpi_enforce_resources=lax` e o código de saída é 3

#### Scenario: Sem ee1004
- **WHEN** o adaptador existe e o driver `ee1004` não tem nenhuma instância nele
- **THEN** a mensagem cita o `ee1004` e o código de saída é 3

### Requirement: Instância que não responde
O sistema SHALL mostrar como "sem resposta" uma instância cujo `eeprom` falha na leitura com `ENXIO` (como a `2-0051` do barramento ASF) e seguir com as outras. Uma instância cujo `eeprom` tem menos de 512 bytes SHALL aparecer como erro, com o tamanho lido e sem campos decodificados. Uma instância cujo `eeprom` falha com `EACCES` SHALL aparecer como "sem permissão".

#### Scenario: Fantasma no ASF
- **WHEN** a leitura do `eeprom` de uma instância dá `ENXIO` e a de outra instância funciona
- **THEN** a primeira aparece como "sem resposta", a segunda é decodificada e o código de saída é 0

#### Scenario: SPD curto
- **WHEN** o `eeprom` de uma instância tem 256 bytes
- **THEN** a instância aparece como erro com "256 bytes" e nenhum campo dela é mostrado

### Requirement: Código de saída com leituras parciais
Quando o adaptador e o `ee1004` existem, o código de saída SHALL ser 0 se pelo menos um pente foi lido e decodificado, mesmo que outras instâncias tenham falhado. Se nenhum foi, SHALL ser 4 quando alguma instância falhou com `EACCES` e 5 nos outros casos.

#### Scenario: Nada lido
- **WHEN** todas as instâncias falham na leitura, nenhuma com `EACCES`
- **THEN** o código de saída é 5

#### Scenario: Sem permissão em todas
- **WHEN** nenhuma instância é decodificada e pelo menos uma falhou com `EACCES`
- **THEN** essa instância aparece como "sem permissão" e o código de saída é 4

#### Scenario: Sem permissão junto de um SPD legível
- **WHEN** uma instância falha com `EACCES` e outra é decodificada
- **THEN** a primeira aparece como "sem permissão", a segunda é decodificada e o código de saída é 0

### Requirement: Campos do SPD
Para cada SPD de 512 bytes, o sistema SHALL mostrar:
- o endereço (`0x50`–`0x57`) e o índice de slot do SPD (endereço − `0x50`);
- o tipo de memória do byte 2 (`0x0C` = DDR4; outro valor em hexadecimal);
- o fabricante JEDEC dos bytes 320–321 (`01 98` = Kingston; outro valor em hexadecimal);
- o part number dos bytes 329–348, sem os espaços e nulos do fim.

#### Scenario: Pente desta máquina
- **WHEN** o SPD tem byte 2 = `0x0C`, bytes 320–321 = `01 98` e o part number `KF3200C16D4/16GX`
- **THEN** a saída mostra DDR4, Kingston e `KF3200C16D4/16GX`

#### Scenario: Não é DDR4
- **WHEN** o byte 2 não é `0x0C`
- **THEN** a saída mostra o valor em hexadecimal e não dá veredito de RGB

### Requirement: Veredito de RGB pelo critério do FURY CTRL
Para um SPD de DDR4 com fabricante `01 98`, o sistema SHALL aplicar o critério do `FUN_1000af50` do `FuryCTRL.dll` 2.0.66.0 (`docs/hardware.md`, "Software oficial: FURY CTRL"). O pente é suportado quando o byte 330 é `0x46` (`F`) ou `0x48` (`H`), o byte 359 está entre 0 e 4 e o byte 360 é `0x10`, `0x20`, `0x22` ou `0x23`. Em qualquer outro caso, ele não é suportado, e a saída SHALL dizer qual byte falhou. Um pente de outro fabricante SHALL aparecer como "não é Kingston", sem veredito.

#### Scenario: Pente suportado
- **WHEN** os bytes 330, 359 e 360 são `0x46`, `0x04` e `0x23`
- **THEN** a saída diz que o pente é suportado

#### Scenario: Byte 360 fora da lista
- **WHEN** a marca é `F`, o modelo é 1 e o byte 360 é `0x00`
- **THEN** a saída diz que o pente não é suportado e cita o byte 360

#### Scenario: Outro fabricante
- **WHEN** os bytes 320–321 não são `01 98`
- **THEN** a saída diz "não é Kingston" e não dá veredito

### Requirement: Nome do modelo
Para um pente suportado, o sistema SHALL mostrar o nome do modelo pela tabela do `CheckDRAMSupport` do `FuryCTRL.dll` 2.0.66.0:
- com `H`: 0 = HyperX Predator, 1 = HyperX Fury;
- com `F`: 0 = Fury Renegade, 1 = Fury Beast, 2 = Fury Beast SE, 3 = Fury Renegade Refresh, 4 = Fury Beast Refresh.

Uma combinação suportada sem nome SHALL aparecer como modelo desconhecido, com o valor do byte 359.

#### Scenario: Fury Beast Refresh
- **WHEN** os bytes 330, 359 e 360 são `0x46`, `0x04` e `0x23`
- **THEN** o modelo mostrado é Fury Beast Refresh

#### Scenario: HyperX sem nome
- **WHEN** os bytes 330, 359 e 360 são `0x48`, `0x03` e `0x10`
- **THEN** a saída diz que o pente é suportado, com modelo desconhecido `0x03`

### Requirement: Bytes brutos sob pedido
Com `--raw`, o sistema SHALL mostrar também, para cada SPD lido, os 512 bytes em hexadecimal, 16 por linha, com o offset no início de cada linha.

#### Scenario: Saída bruta
- **WHEN** o usuário executa `halo probe ram --raw` e um SPD é lido
- **THEN** a saída tem 32 linhas de 16 bytes para esse SPD, a primeira começando em `000` e a última em `1f0`
