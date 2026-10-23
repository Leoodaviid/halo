# Spec Delta

## ADDED Requirements

### Requirement: Painel não inicia o daemon sozinho
O painel SHALL só observar se o nome `io.github.leoodaviid.Halo1` tem dono no D-Bus da sessão e MUST NOT provocar a ativação do daemon ao ler a temperatura, a lista de aparelhos ou qualquer outra informação. Iniciar o daemon SHALL acontecer só pela ação de iniciar o serviço.

#### Scenario: Daemon parado pelo usuário
- **WHEN** o usuário para o `halod.service` com o painel ativo na barra
- **THEN** o indicador fica esmaecido e o daemon continua parado, sem ser reiniciado pelo painel

#### Scenario: Iniciar depois de falhas seguidas
- **WHEN** o `halod.service` parou depois de falhar várias vezes seguidas e o usuário escolhe iniciar o halod
- **THEN** o painel limpa o estado de falha da unidade antes de pedir o início, e o daemon sobe
