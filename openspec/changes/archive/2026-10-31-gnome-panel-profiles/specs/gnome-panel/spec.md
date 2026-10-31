## MODIFIED Requirements

### Requirement: Perfis no cabeçalho
O cabeçalho SHALL mostrar o perfil ativo num menu com os perfis salvos no daemon, lidos de novo a cada abertura do menu, com o ativo marcado. Escolher um perfil SHALL pedir ao daemon que o ative. Os controles SHALL refletir o resultado. Sem perfis salvos, o menu SHALL indicar que não há perfis, sem a opção de ativar. Com um pedido do LED em andamento ou esperando, ou com uma ativação em andamento, o menu MUST NOT pedir outra ativação.

#### Scenario: Ativar perfil
- **WHEN** o usuário escolhe o perfil `noite` no menu
- **THEN** o painel pede a ativação de `noite`, e o menu e os controles passam a mostrar o estado desse perfil quando a ativação termina

#### Scenario: Nenhum perfil
- **WHEN** o daemon não tem perfis salvos
- **THEN** o menu de perfis indica que não há perfis e não oferece nenhuma ativação

#### Scenario: Perfil salvo com o painel aberto
- **WHEN** um perfil `jogo` é salvo pela CLI depois da última abertura do menu, e o usuário abre o menu de novo
- **THEN** o menu lista `jogo`, com o efeito do LED dele ao lado

#### Scenario: Perfil ativo marcado
- **WHEN** o perfil ativo no daemon é `noite` e o usuário abre o menu
- **THEN** o cabeçalho mostra "Perfil: noite", e `noite` aparece marcado no menu

#### Scenario: Ativação em andamento
- **WHEN** o usuário escolhe `noite` e o daemon ainda não respondeu
- **THEN** o cabeçalho mostra "Ativando…", o menu não pede outra ativação, e um ajuste do LED feito nesse intervalo é pedido depois da resposta, mesmo que seja igual ao estado de antes do perfil

#### Scenario: Controles depois da ativação
- **WHEN** a ativação de `noite` termina, com o LED do perfil recebido antes ou depois da resposta do daemon, e o usuário não mexeu nos controles durante ela
- **THEN** os controles mostram o LED de `noite`, e mexer no brilho depois pede o efeito e a cor de `noite`

#### Scenario: Gesto durante a ativação
- **WHEN** o usuário começa a arrastar o brilho durante a ativação de `noite`, o LED de `noite` chega e a ativação termina antes de ele soltar o slider
- **THEN** o slider não salta, e ao soltar o painel pede o brilho escolhido pelo usuário

#### Scenario: Ativação que falha
- **WHEN** o usuário escolhe `jogo`, e o daemon responde com erro
- **THEN** o cabeçalho continua mostrando o perfil ativo de antes, e o painel mostra o erro como em "Erros de aplicação"

#### Scenario: Pedido do LED pendente
- **WHEN** um `Apply` está em andamento ou esperando na fila, e o usuário abre o menu de perfis
- **THEN** nenhum perfil pode ser escolhido até a fila esvaziar

#### Scenario: Daemon fora do ar
- **WHEN** o daemon está fora do ar
- **THEN** o cabeçalho mostra "—" no lugar do perfil, e o menu de perfis não abre

### Requirement: Erros de aplicação
Quando o daemon recusar um pedido do LED ou a ativação de um perfil, o painel SHALL mostrar uma mensagem curta, conforme o erro: aparelho não encontrado, ocupado por outro processo, sem permissão (com a regra udev), falha de comunicação ou, para outros erros, que o halod recusou o pedido, com o texto dele. Os controles SHALL voltar ao último estado aplicado. A mensagem SHALL ter uma ação para tentar de novo, que reenvia o pedido que falhou, e SHALL sumir no pedido seguinte.

#### Scenario: Aparelho ocupado
- **WHEN** o usuário aplica uma cor enquanto um `halo probe waterforce` mantém o aparelho aberto, e o daemon responde que ele está ocupado
- **THEN** o painel mostra que o aparelho está em uso por outro processo, e a cor volta à última aplicada

#### Scenario: Sem permissão
- **WHEN** o daemon responde que não tem permissão para abrir o aparelho
- **THEN** o painel mostra que falta a regra udev do halo

#### Scenario: Tentar de novo
- **WHEN** a cor roxa falhou com o aparelho ocupado e o usuário escolhe tentar de novo quando ele está livre
- **THEN** o painel envia um único pedido com a cor roxa, e a mensagem some quando ele dá certo

#### Scenario: Aparelho não encontrado
- **WHEN** o daemon responde que não encontrou o Waterforce
- **THEN** o painel mostra que o aparelho não foi encontrado e pede para conferir se ele está conectado

#### Scenario: Falha de comunicação
- **WHEN** o daemon responde com falha de comunicação com o aparelho
- **THEN** o painel mostra a falha de comunicação e avisa que o LED pode ter ficado no estado anterior

#### Scenario: Erro sem tipo conhecido
- **WHEN** o daemon responde com um erro que não é de aparelho, ocupado, permissão nem comunicação
- **THEN** o painel mostra que o halod recusou o pedido, com o texto do erro

#### Scenario: Ativação de perfil que falha
- **WHEN** a ativação de `noite` falhou com o aparelho ocupado e o usuário escolhe tentar de novo
- **THEN** o painel pede a ativação de `noite` uma única vez, e não um `Apply`

#### Scenario: Ativação parcial
- **WHEN** a ativação de `noite` aplicou a velocidade e falhou no LED com o aparelho ocupado
- **THEN** o painel mostra que o aparelho está em uso, que a velocidade já foi aplicada, que o LED falhou e que o perfil ativo não mudou

#### Scenario: Mensagem some no pedido seguinte
- **WHEN** a mensagem de erro está visível e o usuário aplica outro ajuste ou escolhe um perfil
- **THEN** a mensagem some quando o pedido novo sai

### Requirement: Linha de status
Abaixo dos controles, no aparelho com controle de LED, o painel SHALL mostrar uma linha de status: aplicando (com quantos pedidos esperam na fila), ativando um perfil, aplicado ao aparelho selecionado, LED desligado, ou estado desconhecido.

#### Scenario: Aplicando com fila
- **WHEN** um pedido está em andamento e outro espera
- **THEN** a linha de status mostra que está aplicando e que há um pedido na fila

#### Scenario: Aplicado
- **WHEN** o último pedido para o Waterforce terminou sem erro e o LED está ligado
- **THEN** a linha de status mostra que o estado foi aplicado ao Waterforce X 240

#### Scenario: Ativando perfil
- **WHEN** a ativação de um perfil está em andamento
- **THEN** a linha de status mostra que um perfil está sendo ativado

#### Scenario: LED desligado
- **WHEN** o estado aplicado é o LED desligado
- **THEN** a linha de status mostra que o LED está desligado

#### Scenario: Estado desconhecido
- **WHEN** o daemon ainda não aplicou nenhum LED e não guardou nenhum
- **THEN** a linha de status mostra que o estado do LED é desconhecido

#### Scenario: Aparelho sem LED
- **WHEN** o aparelho selecionado é a placa-mãe, sem controle de LED
- **THEN** a linha de status não aparece
