# Spec Delta

## Purpose

Ler o controlador RGB de cada pente Kingston Fury DDR4 da API v2 do FURY CTRL (`0x58` + slot do SPD) e aplicar as escritas mínimas do FURY CTRL, sem tocar em nenhum outro endereço do barramento.

## ADDED Requirements

### Requirement: Endereços permitidos
Os comandos `halo probe ram-controller` e `halo led ram` SHALL endereçar apenas `0x58 + s`, onde `s` é o índice de um SPD (endereço do SPD − `0x50`, de 0 a 3) cujo pente foi lido e confirmado como um modelo da API v2 (requisito "Só pentes da API v2"). O sistema MUST NOT endereçar nenhum outro endereço, MUST NOT varrer o barramento, MUST NOT aceitar endereço ou registrador vindos do usuário e MUST NOT forçar um endereço ocupado por outro driver.

#### Scenario: Pentes desta máquina
- **WHEN** os SPDs `0-0052` e `0-0053` são suportados
- **THEN** o sistema só endereça `0x5A` e `0x5B`

#### Scenario: Endereço ocupado por um driver
- **WHEN** o kernel informa que `0x5A` está ocupado por outro driver
- **THEN** o sistema não faz nenhuma transação, informa o conflito e termina com código 5

### Requirement: Só pentes da API v2
O sistema SHALL tratar como pente controlável só um SPD Kingston DDR4 suportado pelo critério do FURY CTRL e com marca `F` (byte 330) e modelo 2, 3 ou 4 (byte 359): Fury Beast SE, Fury Renegade Refresh e Fury Beast Refresh, os modelos DDR4 que o `QueryDDRInformation` do FURY CTRL 2.0.66.0 põe na API v2. Os modelos da API v1 (HyperX Predator e Fury, Fury Renegade e Fury Beast) e as combinações sem nome MUST NOT ser endereçados.

#### Scenario: Pente da API v1
- **WHEN** o único SPD suportado é de um Fury Beast (marca `F`, modelo 1)
- **THEN** o sistema não abre nenhum `/dev/i2c-*`, diz que o pente usa a API v1 do FURY CTRL, fora deste comando, e termina com código 3

#### Scenario: HyperX
- **WHEN** o único SPD suportado é de um HyperX Fury (marca `H`, modelo 1)
- **THEN** o sistema não abre nenhum `/dev/i2c-*` e termina com código 3

### Requirement: Leituras permitidas
As únicas leituras SHALL ser read word data nos registradores `0x0B`, `0x09` e `0x0E` de cada endereço permitido, usando o byte alto como valor, como o `SmbusRead` do `KingstonFuryDRAMController` do OpenRGB `72262b8` e os `GetKingstonFuryDDR5*` do `FuryCTRL.dll` 2.0.66.0 (`docs/hardware.md`, "Protocolo do Beast Refresh").

#### Scenario: Probe só lê
- **WHEN** o usuário executa `halo probe ram-controller` com sucesso
- **THEN** as únicas transações são read word data em `0x0B`, `0x09` e `0x0E` de cada pente suportado

### Requirement: Leitura com tentativas
Cada leitura SHALL ser tentada até 5 vezes, como o `SmbusRead` do OpenRGB, com uma pausa crescente entre as tentativas, e só enquanto falhar: uma leitura bem-sucedida MUST NOT ser repetida. Uma tentativa que falha ou cujo byte alto é `0xFF` SHALL contar como malsucedida. Depois de 5 tentativas, a leitura SHALL falhar com a última causa: `ENXIO` quando todas foram `ENXIO`, ou "leitura inválida (`0xFF`)" quando a última devolveu `0xFF`.

#### Scenario: Lixo isolado
- **WHEN** a leitura de `0x09` em `0x5A` devolve byte alto `0xFF` e, na 2ª tentativa, `0x01`
- **THEN** o valor usado é `0x01`

#### Scenario: Sempre 0xFF
- **WHEN** as 5 tentativas devolvem byte alto `0xFF`
- **THEN** a leitura falha como leitura inválida, sem dizer que o controlador está ausente

### Requirement: Escritas permitidas
As únicas escritas SHALL ser write byte data em `0x08` com `0x53` ou `0x44`, em `0x0B` com o índice calculado do requisito "Índice do pente", e em `0x20`, `0x09`, `0x0D`, `0x0C`, `0x0E`, `0x10`, `0x11` e `0x1E` só com os valores dos requisitos "Apagar", "Reacender" e "Padrão do FURY CTRL" (funções `SetKingstonFuryDDR5*` do `FuryCTRL.dll` 2.0.66.0).

#### Scenario: Nenhum registrador fora da lista
- **WHEN** o usuário executa `halo led ram default` com sucesso
- **THEN** todas as escritas foram nos registradores da lista, com os valores do requisito "Padrão do FURY CTRL" ou da moldura

### Requirement: Adaptador dos pentes suportados
O sistema SHALL escolher o adaptador pelo critério da capability `ram-probe`: o adaptador `SMBus PIIX4 adapter*` com pelo menos um SPD de um pente da API v2 (requisito "Só pentes da API v2"). Ele SHALL abrir só o nó `/dev/i2c-N` desse adaptador. Sem adaptador, sem `ee1004` ou sem pente suportado, ele MUST NOT abrir nenhum `/dev/i2c-*` e SHALL terminar com código 3, dizendo o motivo. Se o nó do `i2c-dev` não existir, SHALL dizer isso e terminar com código 3.

#### Scenario: Pentes suportados
- **WHEN** o adaptador `SMBus PIIX4 adapter port 0 at 0b00` é o `i2c-0` e tem os SPDs `0-0052` e `0-0053` suportados
- **THEN** o sistema abre só o `/dev/i2c-0`

#### Scenario: Nenhum pente suportado
- **WHEN** os SPDs foram lidos e nenhum passa no critério do FURY CTRL
- **THEN** o sistema não abre nenhum `/dev/i2c-*`, diz que não há pente suportado e termina com código 3

### Requirement: SPD ilegível não é falta de suporte
Sem pente suportado confirmado, o sistema SHALL dizer que não há pente suportado (código 3) só se todo SPD que respondeu foi lido e decodificado. Uma instância sem resposta (`ENXIO`) não conta. Se algum SPD falhou com outro erro ou foi curto, ou se nenhum foi decodificado, o sistema SHALL listar as instâncias com a causa e terminar com código 4 se houver `EACCES`, ou 5 nos outros casos, sem abrir nenhum `/dev/i2c-*`.

#### Scenario: SPD sem permissão
- **WHEN** o `eeprom` do único pente falha com `EACCES`
- **THEN** a mensagem cita a instância e o `EACCES`, não diz que falta pente suportado, e o código de saída é 4

#### Scenario: Pente reprovado ao lado da fantasma
- **WHEN** um SPD é lido e reprovado no critério do FURY CTRL e a `2-0051` dá `ENXIO`
- **THEN** a saída diz que não há pente suportado e o código de saída é 3

### Requirement: Leitura dos controladores
O comando `halo probe ram-controller` SHALL ler, de cada pente suportado, os registradores `0x0B`, `0x09` e `0x0E`, e mostrar, por pente: a instância, o endereço, o índice (`0x0B`), o estilo (com nome para 0, 1, 3 e 4) e a velocidade, em hexadecimal. Ele MUST NOT escrever nada. O código de saída SHALL ser 0 quando todos os pentes responderem.

#### Scenario: Dois pentes respondem
- **WHEN** `0x5A` e `0x5B` respondem às três leituras
- **THEN** a saída tem índice, estilo e velocidade para cada um, e o código de saída é 0

### Requirement: Aviso contra leituras em sequência
A saída e a ajuda do `halo probe ram-controller` SHALL avisar para não rodar o comando em sequência, porque séries de leituras com o efeito animando travaram o rainbow e mudaram o índice (`docs/hardware.md`, "Teste de controle").

#### Scenario: Aviso na saída
- **WHEN** o probe termina com sucesso
- **THEN** a saída termina com o aviso para não repetir o comando em sequência

### Requirement: Falha na leitura do controlador
Só a falta de resposta (`ENXIO`) SHALL ser apresentada como controlador ausente: a saída diz que nada respondeu naquele endereço, sem sugerir varrer o barramento. Outra falha (por exemplo `EIO`, `ETIMEDOUT` ou `ENOTTY`) SHALL mostrar a causa original, sem dizer que o controlador está ausente. Se algum pente suportado falhar, o código de saída SHALL ser 3 quando todas as falhas forem `ENXIO`, e 5 nos outros casos.

#### Scenario: Ninguém responde
- **WHEN** as 5 tentativas da leitura de `0x0B` em `0x5A` e em `0x5B` dão `ENXIO`
- **THEN** a saída diz que nada respondeu nos dois endereços e o código de saída é 3

#### Scenario: Falha do adaptador
- **WHEN** a última tentativa da leitura de `0x0B` em `0x5A` falha com `EIO`
- **THEN** a saída mostra o erro original, não diz que o controlador está ausente, e o código de saída é 5

### Requirement: Moldura de escrita
Toda escrita do `halo led ram` SHALL seguir a moldura do FURY CTRL (`CommandStart` e `CommandDoneAndRun`): `0x08` = `0x53` em cada pente suportado, do maior slot para o menor, e uma pausa de 50 ms; as escritas de cada pente, do menor slot para o maior; `0x08` = `0x44` em cada pente, do menor slot para o maior, e uma pausa de 50 ms. Depois de cada escrita, SHALL haver uma pausa de pelo menos 5 ms, como no `FuryCTRL.dll`.

#### Scenario: Ordem nos dois pentes
- **WHEN** o usuário executa `halo led ram off` com os pentes em `0x5A` e `0x5B`
- **THEN** a primeira escrita é `0x5B` `0x08`=`0x53`, a segunda é `0x5A` `0x08`=`0x53`, e as duas últimas são `0x5A` `0x08`=`0x44` e `0x5B` `0x08`=`0x44`

### Requirement: Sem leitura antes de escrever
O `halo led ram` MUST NOT ler os controladores: ele só escreve, como o FURY CTRL depois da detecção (as séries de leituras do halo com o efeito animando travaram o rainbow; `docs/hardware.md`, "Teste de controle"). Se a primeira escrita da moldura falhar com `ENXIO` nas 30 tentativas, nada foi escrito: o sistema SHALL dizer que nada respondeu naquele endereço e terminar com código 3.

#### Scenario: Nenhuma leitura
- **WHEN** o usuário executa `halo led ram off` com sucesso
- **THEN** o processo não fez nenhum read word data

#### Scenario: Primeiro pente não responde
- **WHEN** a escrita `0x5B` `0x08`=`0x53` dá `ENXIO` nas 30 tentativas
- **THEN** nenhuma outra escrita é enviada, a mensagem diz que nada respondeu em `0x5b` e o código de saída é 3

### Requirement: Índice do pente
O valor escrito em `0x0B` SHALL ser o índice de posição do pente, calculado pelos slots dos pentes controláveis em ordem decrescente: o maior slot recebe 0, o seguinte 1, e assim por diante (o FURY CTRL escreve os índices em ordem decrescente; com os pentes desta máquina dá os mesmos valores lidos logo depois do boot). O sistema MUST NOT escrever em `0x0B` um valor lido de `0x0B`.

#### Scenario: Pentes desta máquina
- **WHEN** os pentes controláveis estão nos slots 2 (`0x5A`) e 3 (`0x5B`), e o `0x0B` lido dá `0x0E` nos dois
- **THEN** o sistema escreve `0x0B` = `0x01` em `0x5A` e `0x0B` = `0x00` em `0x5B`

### Requirement: Apagar
O `halo led ram off` SHALL escrever, em cada pente e dentro da moldura, `0x0B` = índice do pente e `0x20` = `0x00`, como o `setDRAMDDR5_AllOff` do FURY CTRL, e nada mais.

#### Scenario: Escritas do apagado
- **WHEN** o usuário executa `halo led ram off` com os pentes em `0x5A` e `0x5B`
- **THEN** as escritas de pente são `0x5A` `0x0B`=`0x01`, `0x5A` `0x20`=`0x00`, `0x5B` `0x0B`=`0x00` e `0x5B` `0x20`=`0x00`

### Requirement: Reacender
O `halo led ram on` SHALL escrever, em cada pente e dentro da moldura, `0x0B` = índice do pente e `0x20` = `0x50` (80, o brilho padrão do FURY CTRL), sem tocar no estilo.

#### Scenario: Escritas do reacender
- **WHEN** o usuário executa `halo led ram on`
- **THEN** cada pente recebe só `0x0B` = índice do pente e `0x20` = `0x50` dentro da moldura

### Requirement: Padrão do FURY CTRL
O `halo led ram default` SHALL escrever, em cada pente e dentro da moldura, nesta ordem, como o `setDRAMDDR5_DefaultRainbow`: `0x09`=`0x01`, `0x0B`=índice do pente, `0x0D`=`0x00`, `0x0C`=`0x01`, `0x0E`=`0x19`, `0x20`=`0x50`, `0x10`=`0x00`, `0x11`=`0x00` e `0x1E`=`0x00`.

#### Scenario: Escritas do padrão
- **WHEN** o usuário executa `halo led ram default`
- **THEN** cada pente recebe as nove escritas na ordem acima, dentro da moldura

### Requirement: Falha durante a moldura
Cada escrita SHALL ser tentada até 30 vezes enquanto falhar, como no `FuryCTRL.dll`. Se uma escrita falhar nas 30 tentativas, o sistema MUST NOT enviar as escritas seguintes e SHALL dizer o endereço e o registrador que falharam, que a moldura ficou incompleta e que repetir o mesmo comando a refaz. O código de saída SHALL ser 5.

#### Scenario: Falha no brilho
- **WHEN** a escrita `0x5A` `0x20` falha nas 30 tentativas
- **THEN** nenhuma escrita seguinte é enviada, a mensagem cita `0x5A` e `0x20`, e o código de saída é 5

#### Scenario: Falha passageira
- **WHEN** uma escrita falha 2 vezes e dá certo na 3ª
- **THEN** o sistema segue com as próximas e termina com código 0

### Requirement: Simulação
Com `--dry-run`, o `halo led ram` SHALL mostrar as transações do comando, uma por linha, com o endereço, a operação, o registrador e o valor, usando os pentes `0x5A` e `0x5B` e os índices calculados para eles (`0x01` e `0x00`). Ele SHALL terminar com código 0 sem ler o sysfs e sem abrir nenhum `/dev/i2c-*`.

#### Scenario: Simulação do apagado
- **WHEN** o usuário executa `halo led ram off --dry-run`
- **THEN** a saída lista a moldura e as escritas do apagado com `0x0B`=`0x01` e `0x00`, sem nenhuma leitura, e nenhum nó é aberto

#### Scenario: Modo inválido
- **WHEN** o usuário executa `halo led ram purple`
- **THEN** o código de saída é 2 e nenhum `/dev/i2c-*` é aberto

### Requirement: Permissão
Quando abrir o `/dev/i2c-N` falhar por falta de permissão, o sistema SHALL terminar com código 4 e dizer que o comando precisa de `sudo`. A mensagem MUST NOT sugerir o grupo `i2c` nem mudar as regras udev.

#### Scenario: Sem root
- **WHEN** o usuário, sem `sudo`, executa `halo probe ram-controller` com os pentes suportados
- **THEN** a mensagem cita o `sudo`, não cita o grupo `i2c`, e o código de saída é 4

### Requirement: Um processo do halo por vez no barramento
Ao abrir o `/dev/i2c-N`, o sistema SHALL tomar um lock exclusivo no nó e esperar até 5 segundos se outro processo do halo estiver com ele. Depois disso, SHALL desistir sem nenhuma transação e terminar com código 5.

#### Scenario: Nó ocupado
- **WHEN** outro processo do halo segura o lock do `/dev/i2c-0` por mais de 5 segundos
- **THEN** o sistema não faz nenhuma transação e termina com código 5
