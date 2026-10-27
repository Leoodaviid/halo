# Spec Delta

## MODIFIED Requirements

### Requirement: Nada é enviado ao hardware sem pedido
O daemon SHALL enviar comandos a um aparelho só como resposta a uma chamada D-Bus, exceto o fluxo de CPU e proteção do X 240 definido em daemon-waterforce. Na inicialização, ticks e resume, MUST NOT abrir outros aparelhos, reaplicar perfis ou enviar configuração espontânea.

#### Scenario: Inicialização
- **WHEN** o daemon é iniciado com um perfil ativo salvo
- **THEN** ele não reaplica o perfil e só pode abrir o X 240 para o fluxo de CPU supervisionado

#### Scenario: Daemon ocioso
- **WHEN** o daemon fica rodando por 10 minutos sem nenhuma chamada D-Bus
- **THEN** só DE/E1/E0 do fluxo de CPU podem ser enviados espontaneamente

### Requirement: Nome do Waterforce pelo modelo já lido
Até conhecer o modelo, o daemon SHALL chamar o Waterforce de `AORUS Waterforce X`. Quando uma operação pedida (LED, velocidade ou perfil) ou a inicialização do feed receber o modelo no `DE 00`, o nome SHALL passar a `AORUS` mais o nome do modelo (`Waterforce X 240`, `X 280` ou `X 360`). O daemon MUST NOT abrir o aparelho só para ler o nome; DE 00 do feed é permitido para validar a conexão. O nome vale até o daemon parar.

#### Scenario: Daemon recém-iniciado
- **WHEN** o daemon acabou de iniciar e nem o feed nem uma operação abriram o Waterforce
- **THEN** a lista mostra `AORUS Waterforce X`, e o daemon não enviou nenhum `DE 00`

#### Scenario: Leitura de velocidade revela o modelo
- **WHEN** um `Status` de velocidade recebe o modelo 2 no `DE 00`
- **THEN** a lista passa a mostrar `AORUS Waterforce X 360`

#### Scenario: Modelo desconhecido
- **WHEN** uma operação recebe no `DE 00` um modelo fora da tabela (por exemplo, 5)
- **THEN** o nome continua `AORUS Waterforce X`, e a operação falha como hoje

### Requirement: Tarefas periódicas só leem o sysfs
O daemon SHALL rodar tarefas periódicas a cada 2 segundos, o intervalo mínimo da telemetria do kernel. Essas leituras SHALL continuar no sysfs, incluindo a CPU; o envio periódico de CPU no X 240 é a única exceção permitida e não pode bloquear as leituras.

#### Scenario: Ciclo periódico
- **WHEN** passam 2 segundos desde a última verificação
- **THEN** o daemon lê a presença e a telemetria no sysfs e pode agendar uma amostra atual de CPU, sem abrir outros aparelhos

### Requirement: Resume
Quando o logind informar o fim de uma suspensão, o daemon SHALL atualizar na hora a presença dos aparelhos e a telemetria, sem esperar o próximo ciclo; no X 240 supervisionado SHALL reinicializar DE/E1 e E0 seguro por 8 s antes de voltar a Tctl atual. Se o D-Bus do sistema ou o logind não estiverem disponíveis, o daemon SHALL continuar funcionando e avisar no log que não vai acompanhar o resume.

#### Scenario: Volta da suspensão
- **WHEN** o logind emite `PrepareForSleep` com `false`
- **THEN** o daemon verifica presença e telemetria em seguida e só reinicializa o fluxo CPU do X 240, sem reaplicar configuração

#### Scenario: Sem logind
- **WHEN** o daemon não consegue assinar os sinais do logind
- **THEN** ele continua oferecendo o D-Bus da sessão e registra um aviso

### Requirement: Operações de um aparelho em série
O daemon SHALL atender as chamadas que abrem o hidraw de um mesmo aparelho uma de cada vez, na ordem em que chegaram entre chamadas de clientes. Um único feed vencido pode ocorrer entre chamadas; nenhuma transação em andamento pode ser intercalada. Enquanto uma chamada espera ou usa um aparelho, o daemon MUST continuar respondendo às chamadas que não usam aquele aparelho, como a lista de aparelhos, a telemetria e os perfis.

#### Scenario: Duas chamadas seguidas no Waterforce
- **WHEN** um cliente pede um preset e, antes de ele terminar, pede um LED
- **THEN** a sequência do preset termina antes de qualquer pacote do LED ser enviado; um E0 pode ocorrer entre as duas sequências

#### Scenario: Leitura durante uma escrita
- **WHEN** uma escrita no Waterforce está em andamento e um cliente lê a lista de perfis
- **THEN** a lista é devolvida sem esperar a escrita

## ADDED Requirements

### Requirement: Encerramento com proteção
Ao receber SIGTERM, o daemon SHALL cancelar amostras frias e novos jobs, tentar E0 seguro antes de sair e registrar o resultado. MUST NOT deixar envio frio pendente ocorrer depois da proteção. O caminho de recuperação SHALL ter prazo de 18 s, sem depender de término ilimitado do worker.

#### Scenario: SIGTERM com feed pendente
- **WHEN** há uma amostra fria aguardando e chega SIGTERM
- **THEN** ela é descartada e o último E0 do principal é de 80 °C, se o transporte permite, sem B6

#### Scenario: Worker travado
- **WHEN** o principal não consegue concluir a recuperação
- **THEN** não declara sucesso; o gerenciador limita a parada e executa a proteção independente

### Requirement: Amostra atual sem backlog
O fluxo periódico SHALL manter no máximo uma amostra pendente e reler Tctl antes do envio. MUST NOT gerar rajadas para compensar tempo perdido nem impedir leituras de telemetria ou ordem entre transações de clientes.

#### Scenario: CLI segurou o nó
- **WHEN** o feed perde dois períodos aguardando o flock
- **THEN** descarta os períodos vencidos e envia no máximo uma amostra atual ao recuperar acesso

### Requirement: Suspensão sem promessa de entrega
Ao receber PrepareForSleep(true), o daemon SHALL descartar feeds frios e tentar E0 seguro. No resume ou salto detectado no relógio de boot, SHALL reinicializar o feed imediatamente. MUST NOT alegar que o sinal sem inibidor garante envio antes da suspensão.

#### Scenario: Resume com fila antiga
- **WHEN** a sessão retoma com amostra fria anterior ao sono
- **THEN** descarta a amostra, reinicializa E1/E0 alto e só depois envia Tctl recém-lido
