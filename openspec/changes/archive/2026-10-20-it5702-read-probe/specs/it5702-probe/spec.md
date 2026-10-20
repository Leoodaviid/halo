# Spec Delta

## Purpose

Ler e mostrar a configuração do controlador ITE IT5702 da placa-mãe sem alterar o estado dele, para validar a comunicação antes de qualquer comando de cor ou efeito.

## ADDED Requirements

### Requirement: Leitura somente-leitura
O probe SHALL enviar ao controlador apenas o pedido de configuração `CC 60` e depois ler o feature report `0xCC`. O probe MUST NOT enviar nenhum outro comando, inclusive os que o OpenRGB envia ao inicializar o driver (reset, beat, lamp array).

#### Scenario: Probe com sucesso
- **WHEN** o usuário executa o probe do IT5702 e o dispositivo está acessível
- **THEN** o sistema envia exatamente um feature report de saída, `CC 60` completado com zeros até 64 bytes, e um pedido de feature report `0xCC`

#### Scenario: Estado do RGB preservado
- **WHEN** o probe termina
- **THEN** as cores e os efeitos da placa continuam iguais aos de antes da execução

### Requirement: Resposta curta é rejeitada
O sistema SHALL tratar como erro uma resposta com menos de 64 bytes, informar quantos bytes recebeu e não mostrar campos decodificados parciais.

#### Scenario: Resposta truncada
- **WHEN** o feature report lido tem menos de 64 bytes
- **THEN** o sistema informa o erro e o tamanho recebido, e termina com código diferente de zero

### Requirement: Campos decodificados
O sistema SHALL mostrar a descrição do produto, a versão de firmware no formato `a.b.c.d`, o número do dispositivo, o byte de detecção de fitas, as flags de comandos suportados, o chip id em hexadecimal e os valores de calibração.

#### Scenario: Exibição dos campos
- **WHEN** o probe recebe uma resposta válida
- **THEN** a saída mostra cada um desses campos com rótulo

#### Scenario: Descrição com terminador nulo
- **WHEN** o texto do produto tem um byte nulo antes de completar 28 bytes
- **THEN** a descrição mostrada termina no primeiro byte nulo

### Requirement: Contagem de LEDs por fita
O sistema SHALL decodificar as seis contagens de fita ARGB (dois valores de 4 bits por byte, nibble baixo primeiro) e mostrá-las como o limite de LEDs correspondente: 32, 64, 256, 512 ou 1024. Um valor desconhecido SHALL ser mostrado como 1024, marcado como desconhecido.

#### Scenario: Fitas com limites diferentes
- **WHEN** o byte de contagem das fitas 0/1 vale `0x10`
- **THEN** a fita 0 aparece com 32 LEDs e a fita 1 com 64 LEDs

### Requirement: Saída bruta
O sistema SHALL oferecer uma opção para mostrar também os 64 bytes brutos da resposta em hexadecimal, para que a decodificação possa ser conferida e registrada na documentação.

#### Scenario: Pedido de saída bruta
- **WHEN** o usuário executa o probe com a opção de saída bruta
- **THEN** a saída inclui os 64 bytes da resposta em hexadecimal, além dos campos decodificados
