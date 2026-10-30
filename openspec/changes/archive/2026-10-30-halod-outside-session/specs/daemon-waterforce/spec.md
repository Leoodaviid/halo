## ADDED Requirements

### Requirement: Proteção ao perder o acesso da sessão
Quando o acesso do daemon ao Castor3 depender só da ACL da sessão, `Session.End()` SHALL responder só depois de o E0 de 80 °C ser aceito e o feed pausado, e MUST NOT haver E0 com Tctl depois disso. `Session.Begin()` SHALL retomar com E1 e a janela de 8 s só se o feed estiver pausado por sessão; fora dela, MUST NOT reiniciar nada. Com acesso pelo grupo, `End()` SHALL responder sem pausar. Caso não comprovado conta como só ACL.

#### Scenario: Linger com a regra antiga
- **WHEN** o daemon roda com linger, o nó do Castor3 só tem a ACL da sessão, e o usuário sai da sessão
- **THEN** o journal registra o E0 de 80 °C e a pausa antes da resposta do `End()`, a resposta vem antes de o logind encerrar a sessão, e nenhum E0 com Tctl é enviado depois

#### Scenario: Volta à sessão
- **WHEN** o feed está pausado por sessão e o usuário entra de novo
- **THEN** o daemon envia E1 e E0 de 80 °C, mantém a proteção por 8 s e volta ao Tctl atual

#### Scenario: Sem acesso no boot
- **WHEN** o daemon sobe pelo linger antes de qualquer login, a guarda da sessão não está ativa, e o nó só tem a ACL da sessão
- **THEN** ele começa pausado, registra que não alcança o Castor3 sem sessão e não envia nada até o `Begin()`

#### Scenario: Acesso pelo grupo
- **WHEN** o grupo dono do nó do Castor3 tem leitura e escrita efetivas, inclusive sob a máscara da ACL, o daemon é desse grupo, e o usuário sai da sessão
- **THEN** o `End()` responde sem pausar, e o feed continua enviando o Tctl a cada 2 s, sem E0 de 80 °C

#### Scenario: Login com acesso pelo grupo
- **WHEN** o feed está enviando o Tctl sem pausa, e o usuário sai e entra na sessão (`End()` seguido de `Begin()`)
- **THEN** nenhum E1 nem E0 de 80 °C é enviado, e o Tctl continua a cada 2 s sem reiniciar a janela de 8 s

#### Scenario: Máscara da ACL não é o grupo
- **WHEN** o nó é `root:plugdev` com ACL `group::r--`, `user:<usuário>:rw-` e `mask::rw-`, e o daemon é do grupo `plugdev`
- **THEN** o acesso conta como só pela ACL, e o `End()` protege e pausa

#### Scenario: Tela bloqueada
- **WHEN** o usuário bloqueia a tela, sem sair
- **THEN** a sessão gráfica continua, nenhum `End()` é chamado, e o feed segue sem pausa
