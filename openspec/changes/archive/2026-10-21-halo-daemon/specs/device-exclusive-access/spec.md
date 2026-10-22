# Spec Delta

## Purpose

Garantir que só um processo do halo por vez converse com o hidraw de um controlador, para que pedidos e respostas de clientes diferentes (CLI e daemon) não se misturem.

## ADDED Requirements

### Requirement: Acesso exclusivo ao hidraw
Ao abrir o hidraw de um controlador, o sistema SHALL obter acesso exclusivo àquele aparelho entre os processos do halo e mantê-lo até fechar o hidraw. Enquanto um processo do halo tiver o aparelho aberto, nenhum outro processo do halo MUST enviar comandos a ele. O acesso exclusivo vale para todo comando que abre o hidraw: probe, LED, velocidade e as escritas do daemon.

#### Scenario: Probe durante uma escrita do daemon
- **WHEN** o daemon está gravando um preset no Waterforce e o usuário roda `halo probe waterforce`
- **THEN** o probe só envia o primeiro pedido depois que o daemon fecha o hidraw, e as respostas de cada um chegam só a quem fez o pedido

#### Scenario: Aparelhos diferentes
- **WHEN** um processo do halo tem o hidraw do Waterforce aberto e outro abre o do IT5702
- **THEN** o segundo abre o IT5702 sem esperar

### Requirement: Espera limitada
Se o aparelho estiver em uso por outro processo do halo, o sistema SHALL esperar até 5 segundos pelo acesso. Se o acesso não vier nesse prazo, o sistema MUST NOT enviar nada ao aparelho e SHALL informar que ele está ocupado por outro processo do halo, com o caminho do hidraw.

#### Scenario: Liberado dentro do prazo
- **WHEN** o aparelho fica livre 1 segundo depois do pedido
- **THEN** o sistema prossegue normalmente

#### Scenario: Ocupado além do prazo
- **WHEN** o aparelho continua em uso 5 segundos depois do pedido
- **THEN** o sistema não envia nenhum comando e informa que o aparelho está ocupado, com o caminho do hidraw

### Requirement: Código de saída da CLI com aparelho ocupado
Quando a CLI não conseguir o acesso exclusivo no prazo, ela SHALL terminar com o código 5, o mesmo da falha de comunicação, e a mensagem MUST distinguir "ocupado" de falta de permissão e de aparelho ausente.

#### Scenario: CLI com o aparelho ocupado
- **WHEN** o usuário roda `halo led waterforce static` e o Waterforce continua em uso por outro processo do halo por mais de 5 segundos
- **THEN** a CLI informa que o aparelho está ocupado, não sugere a regra udev e termina com código 5

### Requirement: Leituras sem hidraw não esperam
Operações que não abrem o hidraw, como a telemetria pelo hwmon, a enumeração pelo sysfs e as simulações, SHALL funcionar sem pedir o acesso exclusivo e MUST NOT esperar por ele.

#### Scenario: Telemetria durante uma escrita
- **WHEN** o daemon está gravando no Waterforce e o usuário roda `halo telemetry waterforce`
- **THEN** a telemetria é mostrada sem esperar a escrita terminar
