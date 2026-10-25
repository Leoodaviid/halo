# Spec Delta

## MODIFIED Requirements

### Requirement: Estado mostrado vem do daemon
Ao abrir, o painel SHALL mostrar o último estado de LED que o daemon informa como aplicado. Sem esse dado, o painel SHALL mostrar os controles sem seleção de efeito e indicar que o estado atual do aparelho não é conhecido. Depois de um erro, os controles SHALL voltar ao último estado aplicado.

#### Scenario: Daemon com estado aplicado
- **WHEN** o daemon informa como último estado o pulso laranja, com brilho 8 e velocidade 2
- **THEN** o painel abre com pulso, `#FF6600`, brilho 8 de 10 e velocidade 2 de 5

#### Scenario: Daemon sem estado aplicado
- **WHEN** o daemon não tem nenhum LED aplicado registrado
- **THEN** o painel indica que o estado atual do LED não é conhecido e não envia nada até o usuário escolher um efeito

#### Scenario: Depois de sair e entrar na sessão
- **WHEN** o usuário aplicou o estático vermelho, saiu da sessão e entrou de novo
- **THEN** o painel abre com estático, `#FF0000` e o switch ligado, sem enviar nada ao aparelho

### Requirement: Liga e desliga
O liga/desliga do cabeçalho SHALL desligar o LED do aparelho selecionado com o efeito desligado. Religar SHALL reaplicar o último estado ligado que o daemon conhece. Sem esse dado, SHALL reaplicar o LED do perfil ativo e, sem perfil com LED, o efeito ciclo, o mais próximo do modo de fábrica.

#### Scenario: Desligar
- **WHEN** o usuário desliga com o pulso laranja aplicado
- **THEN** o painel pede o efeito desligado, e o switch fica desligado

#### Scenario: Religar com estado conhecido
- **WHEN** o usuário religa depois de desligar o pulso laranja na mesma sessão do daemon
- **THEN** o painel pede de novo o pulso laranja com o mesmo brilho e a mesma velocidade

#### Scenario: Religar depois de entrar na sessão
- **WHEN** o usuário desliga o LED pelo switch depois de entrar na sessão, com o estático vermelho registrado pelo daemon, e religa
- **THEN** o painel pede de novo o estático vermelho

#### Scenario: Religar sem estado conhecido
- **WHEN** o usuário religa sem estado ligado conhecido (o daemon não tem LED registrado, ou o último registrado é o desligado), sem perfil ativo com LED
- **THEN** o painel pede o efeito ciclo

#### Scenario: Religar com o perfil ativo desligado
- **WHEN** o usuário religa sem estado ligado conhecido, e o LED do perfil ativo é o efeito desligado
- **THEN** o painel pede o efeito ciclo, e não o desligado do perfil

#### Scenario: Reboot com corte de energia
- **WHEN** o usuário aplicou o estático branco pelo painel, desligou o PC com corte de energia e ligou de novo
- **THEN** o painel abre com estático, `#FFFFFF` e o switch ligado, sem Apply no login nem ao abrir o painel; desligar e religar pedem off e o mesmo estático branco
