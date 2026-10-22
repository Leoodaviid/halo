# Spec Delta

## MODIFIED Requirements

### Requirement: Aparelhos vindos do daemon
O carrossel SHALL mostrar só os aparelhos que o daemon lista como presentes, na ordem da lista, com o nome que o daemon informa e o status "Disponível" quando o aparelho tem a capacidade `led`, ou "Detectado" quando não tem. O painel MUST NOT ter um catálogo próprio de hardware. Com um único aparelho, a navegação do carrossel (setas e indicador de página) SHALL ficar oculta.

#### Scenario: Lista do daemon
- **WHEN** o daemon lista o Waterforce presente com a capacidade `led`, a placa-mãe presente sem capacidades e o G502 ausente
- **THEN** o carrossel tem duas páginas, o Waterforce como "Disponível" e a placa-mãe como "Detectado", nessa ordem, sem o G502

#### Scenario: Um aparelho só
- **WHEN** o daemon lista só o Waterforce como presente
- **THEN** o carrossel mostra o Waterforce sem setas e sem indicador de página

#### Scenario: Aparelho desconectado
- **WHEN** o daemon anuncia que a lista mudou e o aparelho da página atual deixou de estar presente
- **THEN** o carrossel vai para o primeiro aparelho presente da nova lista, ou mostra que nenhum aparelho foi encontrado se nenhum estiver presente

#### Scenario: Nome muda
- **WHEN** o daemon anuncia o Waterforce com o nome `AORUS Waterforce X 240` no lugar de `AORUS Waterforce X`
- **THEN** o carrossel continua na mesma página e mostra o nome novo

### Requirement: Aparelho sem controle
Um aparelho presente sem a capacidade `led` SHALL aparecer com o aviso de controle em desenvolvimento no lugar dos controles, com o status "Detectado". O liga/desliga SHALL ficar desabilitado nessa página.

#### Scenario: Placa-mãe detectada
- **WHEN** o usuário navega até a placa-mãe, listada sem capacidades
- **THEN** o painel mostra o aviso de controle em desenvolvimento, sem cor, efeito, brilho ou velocidade, e com o liga/desliga desabilitado
