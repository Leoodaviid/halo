# Spec Delta

## MODIFIED Requirements

### Requirement: Aplicação só ao concluir o gesto
Cada escrita no LED grava no aparelho. Por isso, o painel SHALL pedir uma aplicação só ao fim de cada gesto: ao soltar um slider, ao escolher um efeito ou cor, ou ao confirmar o hex. Durante o arraste, a mudança SHALL aparecer só na interface. O painel MUST NOT ter mais de um pedido de aplicação em andamento nem repetir pedidos em laço.

#### Scenario: Arrastar o brilho
- **WHEN** o usuário arrasta o brilho de 3 até 9 e solta
- **THEN** o valor mostrado acompanha o arraste, e o painel envia um único pedido de aplicação, com brilho 9, depois de soltar

#### Scenario: Mudança durante um pedido em andamento
- **WHEN** o usuário muda a cor duas vezes enquanto um pedido de aplicação ainda não terminou
- **THEN** o painel espera o pedido em andamento terminar e envia um único pedido, com a última cor escolhida

#### Scenario: Valor igual ao aplicado
- **WHEN** o usuário solta o slider no mesmo valor que já está aplicado
- **THEN** nenhum pedido é enviado

#### Scenario: Pedido em andamento falha com uma mudança esperando
- **WHEN** o usuário muda a cor enquanto um pedido de aplicação está em andamento, e esse pedido falha
- **THEN** o painel descarta a mudança que esperava, não envia mais nenhum pedido, e os controles voltam ao último estado aplicado

### Requirement: Liga e desliga
O liga/desliga do cabeçalho SHALL desligar o LED do aparelho selecionado com o efeito desligado. Religar SHALL reaplicar o último estado ligado que o daemon conhece. Sem esse dado, SHALL reaplicar o LED do perfil ativo e, sem perfil com LED, o efeito ciclo, o mais próximo do modo de fábrica.

#### Scenario: Desligar
- **WHEN** o usuário desliga com o pulso laranja aplicado
- **THEN** o painel pede o efeito desligado, e o switch fica desligado

#### Scenario: Religar com estado conhecido
- **WHEN** o usuário religa depois de desligar o pulso laranja na mesma sessão do daemon
- **THEN** o painel pede de novo o pulso laranja com o mesmo brilho e a mesma velocidade

#### Scenario: Religar sem estado conhecido
- **WHEN** o usuário religa depois de o daemon reiniciar, sem perfil ativo com LED
- **THEN** o painel pede o efeito ciclo

#### Scenario: Religar com o perfil ativo desligado
- **WHEN** o usuário religa sem estado ligado conhecido, e o LED do perfil ativo é o efeito desligado
- **THEN** o painel pede o efeito ciclo, e não o desligado do perfil
