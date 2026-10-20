# Spec Delta

## Purpose

Mostrar a telemetria do AORUS Waterforce (rotação e duty da fan e da bomba, temperatura do líquido) a partir do hwmon do kernel, sem enviar nenhum comando ao aparelho.

## ADDED Requirements

### Requirement: Fonte da telemetria
O sistema SHALL ler a telemetria do Waterforce no hwmon cujo `name` é `waterforce` e cujo dispositivo pai é o HID `1044:7a4d`. O sistema MUST NOT abrir o hidraw nem enviar comandos para obter a telemetria.

#### Scenario: hwmon presente
- **WHEN** o driver `gigabyte_waterforce` está carregado e o aparelho está conectado
- **THEN** o sistema encontra o hwmon do Waterforce em qualquer `hwmonN`, sem depender do número

#### Scenario: Leitura não abre o hidraw
- **WHEN** o usuário pede a telemetria
- **THEN** o sistema lê só arquivos do sysfs do hwmon e não abre nenhum `/dev/hidrawN`

### Requirement: Valores apresentados
O sistema SHALL mostrar a rotação da fan e da bomba em RPM, o duty de cada uma como valor bruto (0 a 255) e em porcentagem, e a temperatura do líquido em °C.

#### Scenario: Conversão de unidades
- **WHEN** o hwmon informa `temp1_input = 29000`, `fan1_input = 2204` e `pwm1 = 224`
- **THEN** a saída mostra líquido a 29,0 °C, fan a 2204 RPM e duty da fan 224 (88%)

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
