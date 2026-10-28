## MODIFIED Requirements

### Requirement: Fonte da temperatura configurável
As preferências SHALL oferecer duas fontes de temperatura para a barra: o líquido do Waterforce, que é o padrão, e a CPU. A opção CPU SHALL ficar desabilitada enquanto o daemon não publicar a temperatura da CPU, a menos que ela já seja a fonte escolhida. A escolha SHALL ser guardada entre sessões, e a troca de fonte SHALL valer sem reabrir o painel nem reiniciar o Shell.

#### Scenario: Padrão
- **WHEN** o usuário nunca alterou as preferências
- **THEN** a barra mostra a temperatura do líquido

#### Scenario: CPU indisponível no daemon
- **WHEN** o usuário abre as preferências e o daemon não publica a temperatura da CPU
- **THEN** a opção CPU aparece desabilitada, com a explicação de que o daemon ainda não fornece esse valor

#### Scenario: CPU disponível no daemon
- **WHEN** o usuário abre as preferências e o daemon publica a temperatura da CPU
- **THEN** a opção CPU fica habilitada, com a explicação de que o valor vem do halod

#### Scenario: Daemon passa a publicar com as preferências abertas
- **WHEN** as preferências estão abertas com a opção CPU desabilitada e o daemon começa a publicar a temperatura da CPU
- **THEN** a opção CPU fica habilitada sem fechar e reabrir as preferências

#### Scenario: CPU escolhida com a CPU indisponível
- **WHEN** a fonte escolhida é a CPU e o usuário abre as preferências com o daemon sem publicá-la
- **THEN** a opção CPU continua marcada e selecionável, com a explicação de indisponível, e o líquido pode ser escolhido

#### Scenario: Troca para a CPU
- **WHEN** o daemon publica a CPU válida em 61,2 °C e o líquido em 34 °C, e o usuário escolhe a CPU com a barra mostrando `34°`
- **THEN** a barra passa a mostrar `61°` sem reabrir o painel nem reiniciar o Shell

#### Scenario: Escolha guardada
- **WHEN** a fonte escolhida é a CPU e o usuário sai e entra na sessão
- **THEN** a barra continua com a temperatura da CPU

#### Scenario: CPU sem Waterforce
- **WHEN** a fonte escolhida é a CPU, o daemon publica a CPU válida e o Waterforce não está presente
- **THEN** a barra mostra a temperatura da CPU

#### Scenario: CPU escolhida e depois indisponível
- **WHEN** a fonte escolhida é a CPU e o daemon deixa de publicá-la
- **THEN** a barra mostra só o ícone, sem trocar a fonte sozinha para o líquido

### Requirement: Temperatura inválida ou ausente
A barra MUST NOT mostrar uma temperatura que o daemon marcou como inválida, do líquido ou da CPU. Um valor inválido SHALL aparecer como `--`. Quando o daemon não tiver leitura da fonte escolhida, a barra SHALL mostrar só o ícone.

#### Scenario: Líquido inválido
- **WHEN** o daemon publica o líquido em 224 °C com a validade falsa
- **THEN** a barra mostra o ícone e `--`, e nunca `224°`

#### Scenario: CPU inválida com valor antigo
- **WHEN** a fonte escolhida é a CPU e o daemon publica a CPU em 45 °C com a validade falsa, o último valor antes da falha
- **THEN** a barra mostra o ícone e `--`, e nunca `45°`

#### Scenario: Telemetria indisponível
- **WHEN** o daemon informa que a telemetria do Waterforce não está disponível
- **THEN** a barra mostra só o ícone

### Requirement: Itens de menu do painel
O fim da página principal SHALL ter o item de menu que abre as preferências da extensão. O item que abre os detalhes do aparelho selecionado SHALL ficar acima dele quando os detalhes existirem.

#### Scenario: Preferências
- **WHEN** o usuário escolhe as preferências do halo no painel
- **THEN** o painel fecha e a janela de preferências da extensão abre, com a fonte da temperatura da barra

#### Scenario: Preferências com o daemon fora do ar
- **WHEN** o daemon está fora do ar e o usuário abre o painel
- **THEN** o item de preferências continua no fim da página principal e abre a janela
