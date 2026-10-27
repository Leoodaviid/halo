# Spec Delta

## MODIFIED Requirements

### Requirement: Acesso exclusivo ao hidraw
Ao abrir o hidraw de um controlador, o sistema SHALL obter acesso exclusivo àquele aparelho entre os processos do halo e mantê-lo até fechar o hidraw. Enquanto um processo do halo tiver o aparelho aberto, nenhum outro processo do halo MUST enviar comandos a ele. O acesso exclusivo vale para todo comando que abre o hidraw: probe, LED, velocidade as escritas do daemon, o feed CPU e o comando de proteção; o feed SHALL fechar o nó ao concluir cada transação, sem manter o lock entre ticks.

#### Scenario: Probe durante uma escrita do daemon
- **WHEN** o daemon está gravando um preset no Waterforce e o usuário roda `halo probe waterforce`
- **THEN** o probe só envia o primeiro pedido depois que o daemon fecha o hidraw, e as respostas de cada um chegam só a quem fez o pedido

#### Scenario: Aparelhos diferentes
- **WHEN** um processo do halo tem o hidraw do Waterforce aberto e outro abre o do IT5702
- **THEN** o segundo abre o IT5702 sem esperar

## ADDED Requirements

### Requirement: Feed não monopoliza o nó
O feed CPU SHALL liberar o flock entre envios, usar a espera limitada de 5 s e nunca intercalar pacotes numa transação de CLI ou D-Bus. A proteção de ExecStopPost SHALL respeitar o mesmo lock e informar falha quando não obtiver acesso.

#### Scenario: CLI entre ticks
- **WHEN** a CLI abre o nó entre duas transações CPU
- **THEN** obtém o flock e conclui sua sequência inteira antes de qualquer novo E0

#### Scenario: Proteção com lock externo retido
- **WHEN** o principal morreu mas outra CLI mantém flock por mais de 5 s
- **THEN** o comando de proteção termina Busy sem enviar bytes e registra que não protegeu o aparelho

#### Scenario: Espera do feed
- **WHEN** outro processo libera flock dentro de 5 s
- **THEN** o feed prossegue com Tctl relido, sem reproduzir períodos antigos
