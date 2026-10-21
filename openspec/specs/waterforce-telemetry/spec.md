# waterforce-telemetry Specification

## Purpose

Mostrar a telemetria do AORUS Waterforce (rotação e duty da fan e da bomba, temperatura do líquido) a partir do hwmon do kernel, sem enviar nenhum comando ao aparelho.

## Requirements

### Requirement: Fonte da telemetria
O sistema SHALL ler a telemetria do Waterforce no hwmon cujo `name` é `waterforce` e cujo dispositivo pai é o HID `1044:7a4d`. O sistema MUST NOT abrir o hidraw nem enviar comandos para obter a telemetria.

#### Scenario: hwmon presente
- **WHEN** o driver `gigabyte_waterforce` está carregado e o aparelho está conectado
- **THEN** o sistema encontra o hwmon do Waterforce em qualquer `hwmonN`, sem depender do número

#### Scenario: Leitura não abre o hidraw
- **WHEN** o usuário pede a telemetria
- **THEN** o sistema lê só arquivos do sysfs do hwmon e não abre nenhum `/dev/hidrawN`

### Requirement: Valores apresentados
O sistema SHALL mostrar a rotação da fan e da bomba em RPM, o duty de cada uma como valor bruto (0 a 255) e em porcentagem, e a temperatura do líquido em °C. Uma temperatura do líquido fora da faixa plausível MUST aparecer como inválida, e não como temperatura.

#### Scenario: Conversão de unidades
- **WHEN** o hwmon informa `temp1_input = 29000`, `fan1_input = 2204` e `pwm1 = 224`
- **THEN** a saída mostra líquido a 29,0 °C, fan a 2204 RPM e duty da fan 224 (88%)

#### Scenario: Temperatura inválida não esconde fan e bomba
- **WHEN** o hwmon informa `temp1_input = 224000`, `fan1_input = 2256` e `pwm1 = 224`
- **THEN** a saída mostra o líquido como inválido, com fan a 2256 RPM e duty da fan 224 (88%)
- **AND** o código de saída é 0

### Requirement: Driver ausente
O sistema SHALL informar, com código de saída diferente de zero, quando não houver hwmon do Waterforce, e diferenciar "driver não carregado" de "aparelho não conectado" quando o sysfs permitir.

#### Scenario: Sem hwmon e sem aparelho
- **WHEN** não existe hwmon `waterforce` nem dispositivo HID `1044:7a4d`
- **THEN** o sistema informa que o Waterforce não foi encontrado

#### Scenario: Aparelho presente sem driver
- **WHEN** existe o dispositivo HID `1044:7a4d` mas nenhum hwmon `waterforce`
- **THEN** o sistema informa que o driver `gigabyte_waterforce` não está carregado

### Requirement: Leitura contínua
O sistema SHALL oferecer uma opção que repete a leitura em intervalo fixo, com intervalo mínimo de 2 segundos (validade do status no driver do kernel), até o usuário interromper.

#### Scenario: Intervalo abaixo do mínimo
- **WHEN** o usuário pede leitura contínua com intervalo de 1 segundo
- **THEN** o sistema recusa o valor e informa o mínimo de 2 segundos

### Requirement: Temperatura do líquido fora da faixa
O sistema SHALL considerar inválida a temperatura do líquido acima de 100 °C (100000 m°C no `temp1_input`) e válida a de 100 °C ou menos. A leitura inválida MUST manter o valor informado pelo aparelho. O sistema MUST NOT enviar nenhum comando ao aparelho por causa dela.

#### Scenario: Valores observados no aparelho travado
- **WHEN** o hwmon informa `temp1_input = 224000` ou `temp1_input = 240000`
- **THEN** a temperatura do líquido é inválida, com o valor informado de 224 ou 240 °C

#### Scenario: No limite da faixa
- **WHEN** o hwmon informa `temp1_input = 100000`
- **THEN** a temperatura do líquido é válida, a 100,0 °C

#### Scenario: Acima do limite
- **WHEN** o hwmon informa `temp1_input = 101000`
- **THEN** a temperatura do líquido é inválida

#### Scenario: Leitura de referência
- **WHEN** o hwmon informa `temp1_input = 31000`
- **THEN** a temperatura do líquido é válida, a 31,0 °C

### Requirement: Aviso da temperatura inválida
Quando a temperatura do líquido for inválida, a saída padrão SHALL mostrar o valor informado pelo aparelho, e a saída de erro (stderr) SHALL mostrar uma nota. A nota diz que o aparelho já informou valores assim, que eles só voltaram ao normal depois de desligar o PC com corte de energia, e aponta para `docs/hardware.md`. Na leitura contínua, a nota MUST aparecer no máximo uma vez por execução.

#### Scenario: Leitura única
- **WHEN** o usuário pede a telemetria e o hwmon informa `temp1_input = 240000`
- **THEN** a linha do líquido diz que a leitura é inválida e mostra 240 °C como o valor informado
- **AND** o stderr tem a nota sobre o corte de energia e `docs/hardware.md`, e o stdout não tem

#### Scenario: Leitura contínua
- **WHEN** o usuário pede a leitura contínua e várias leituras seguidas têm a temperatura inválida
- **THEN** cada leitura marca o líquido como inválido
- **AND** a nota aparece só uma vez
