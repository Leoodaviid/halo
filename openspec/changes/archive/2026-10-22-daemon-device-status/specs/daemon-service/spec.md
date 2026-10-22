# Spec Delta

## MODIFIED Requirements

### Requirement: Lista de aparelhos
O daemon SHALL listar os aparelhos que conhece (IT5702, Waterforce e G502), cada um como um dicionário com o identificador estável (`it5702`, `waterforce`, `g502`), o nome, se está presente, o caminho hidraw atual e as capacidades. A presença SHALL vir só do sysfs, por VID:PID e usage page. Um aparelho ausente SHALL continuar na lista, marcado como ausente.

#### Scenario: Waterforce conectado
- **WHEN** o Castor3 `1044:7a4d` com usage page `0x0000` está em `/dev/hidraw3`
- **THEN** a lista mostra `waterforce` presente em `/dev/hidraw3`, com as capacidades `led`, `speed` e `telemetry`

#### Scenario: Aparelho desconectado
- **WHEN** o IT5702 não aparece no sysfs
- **THEN** a lista mostra `it5702` ausente e sem caminho

#### Scenario: G502 detectado sem ser aberto
- **WHEN** o Logitech `046d:c08b` tem uma interface com usage page `0xFF00` em `/dev/hidraw1`
- **THEN** a lista mostra `g502` presente em `/dev/hidraw1`, com o nome `Logitech G502 SE HERO` e sem capacidades, e o daemon não abre nenhum hidraw do mouse

#### Scenario: Placa-mãe pelo nome DMI
- **WHEN** o IT5702 está presente e `/sys/class/dmi/id/board_name` contém `B550M AORUS ELITE`
- **THEN** a lista mostra `it5702` com o nome `B550M AORUS ELITE` e sem capacidades

#### Scenario: Campo desconhecido
- **WHEN** um cliente lê um aparelho que tem uma chave que ele não conhece
- **THEN** as outras chaves continuam com o mesmo nome e o mesmo tipo, e o cliente pode ignorar a desconhecida

### Requirement: Mudança de presença sinalizada
Quando a presença, o caminho hidraw ou o nome de um aparelho mudar, o daemon SHALL emitir um sinal D-Bus com a lista nova, no formato da lista de aparelhos, no máximo 2 segundos depois da próxima verificação ou logo depois da operação que revelou o nome. Ele MUST NOT emitir o sinal quando nada mudou.

#### Scenario: Aparelho volta com outro hidraw
- **WHEN** o Waterforce sai de `/dev/hidraw3` e volta em `/dev/hidraw5`
- **THEN** o daemon emite o sinal com `waterforce` presente em `/dev/hidraw5`

#### Scenario: Nome muda depois da primeira operação
- **WHEN** o daemon ainda mostra `AORUS Waterforce X` e uma aplicação de LED recebe o modelo 0 no `DE 00`
- **THEN** o daemon emite o sinal com o nome `AORUS Waterforce X 240`

#### Scenario: Nada mudou
- **WHEN** duas verificações seguidas encontram os mesmos aparelhos, caminhos e nomes
- **THEN** o daemon não emite o sinal

## ADDED Requirements

### Requirement: Nome do Waterforce pelo modelo já lido
Até conhecer o modelo, o daemon SHALL chamar o Waterforce de `AORUS Waterforce X`. Quando uma operação pedida (LED, velocidade ou perfil) receber o modelo no `DE 00`, o nome SHALL passar a `AORUS` mais o nome do modelo (`Waterforce X 240`, `X 280` ou `X 360`). O daemon MUST NOT abrir o aparelho só para ler o modelo. O nome vale até o daemon parar.

#### Scenario: Daemon recém-iniciado
- **WHEN** o daemon acabou de iniciar e nenhuma operação abriu o Waterforce
- **THEN** a lista mostra `AORUS Waterforce X`, e o daemon não enviou nenhum `DE 00`

#### Scenario: Leitura de velocidade revela o modelo
- **WHEN** um `Status` de velocidade recebe o modelo 2 no `DE 00`
- **THEN** a lista passa a mostrar `AORUS Waterforce X 360`

#### Scenario: Modelo desconhecido
- **WHEN** uma operação recebe no `DE 00` um modelo fora da tabela (por exemplo, 5)
- **THEN** o nome continua `AORUS Waterforce X`, e a operação falha como hoje
