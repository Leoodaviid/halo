# Spec Delta

## MODIFIED Requirements

### Requirement: Último LED aplicado
Como o protocolo não tem leitura do LED, a interface `Led` SHALL expor como propriedade o último efeito, cores, brilho e velocidade que o próprio daemon aplicou com sucesso, e emitir a mudança dessa propriedade. O daemon SHALL guardar esse valor no diretório de estado do usuário a cada aplicação bem-sucedida e SHALL voltar a publicá-lo ao iniciar, sem enviar nada ao aparelho. A propriedade MUST ficar vazia só quando não houver valor guardado válido, e não pode ser apresentada como o estado do aparelho.

#### Scenario: Depois de aplicar
- **WHEN** a aplicação de `cycle` termina com sucesso
- **THEN** a propriedade passa a informar `cycle` com o brilho e a velocidade usados, e um sinal de mudança é emitido

#### Scenario: Aplicação que falhou
- **WHEN** a aplicação falha porque o modelo não foi reconhecido
- **THEN** a propriedade continua com o valor anterior, e o valor guardado também

#### Scenario: Depois de reiniciar
- **WHEN** o daemon aplicou o estático vermelho com brilho 10, foi encerrado e iniciou de novo
- **THEN** a propriedade informa o estático vermelho com brilho 10 desde o início, e nenhum comando é enviado ao aparelho por causa disso

#### Scenario: Sem valor guardado
- **WHEN** o daemon inicia sem nenhum valor guardado, porque o halo nunca aplicou um LED pelo daemon
- **THEN** a propriedade fica vazia

#### Scenario: Valor guardado inválido
- **WHEN** o valor guardado não pode ser lido ou tem um efeito, uma cor, um brilho ou uma velocidade inválidos
- **THEN** o daemon inicia normalmente com a propriedade vazia, registra um aviso no log e não envia nada ao aparelho

#### Scenario: Falha ao guardar
- **WHEN** uma aplicação dá certo, mas o valor não pode ser gravado no diretório de estado
- **THEN** a aplicação continua sendo respondida com sucesso, a propriedade muda, e o daemon registra um aviso no log
