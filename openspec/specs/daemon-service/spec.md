# daemon-service Specification

## Purpose

Definir o processo `halod`: como ele aparece no D-Bus da sessão, quais aparelhos expõe, como informa erros, o que faz sozinho em segundo plano e a regra de nunca enviar comandos ao hardware sem um pedido.

## Requirements

### Requirement: Nome no D-Bus da sessão
O daemon SHALL se registrar no D-Bus da sessão do usuário com o nome `io.github.leoodaviid.Halo1` e publicar seus objetos sob `/io/github/leoodaviid/Halo1`. Ele MUST rodar sem root e MUST NOT usar o D-Bus do sistema para oferecer serviços.

#### Scenario: Daemon iniciado
- **WHEN** o usuário inicia o `halod` numa sessão gráfica
- **THEN** o nome `io.github.leoodaviid.Halo1` aparece no D-Bus da sessão, pertencendo ao processo do usuário

### Requirement: Instância única
Se o nome `io.github.leoodaviid.Halo1` já tiver dono, o daemon SHALL informar que já existe uma instância e terminar com código diferente de zero, sem entrar em fila pelo nome e sem abrir nenhum aparelho.

#### Scenario: Segunda instância
- **WHEN** um `halod` já está rodando e o usuário inicia outro
- **THEN** o segundo informa que o halo já está rodando e termina com código diferente de zero, e o primeiro continua dono do nome

### Requirement: Nada é enviado ao hardware sem pedido
O daemon SHALL enviar comandos a um aparelho só como resposta a uma chamada D-Bus, exceto o fluxo de CPU e proteção do X 240 definido em daemon-waterforce. Na inicialização, ticks e resume, MUST NOT abrir outros aparelhos, reaplicar perfis ou enviar configuração espontânea.

#### Scenario: Inicialização
- **WHEN** o daemon é iniciado com um perfil ativo salvo
- **THEN** ele não reaplica o perfil e só pode abrir o X 240 para o fluxo de CPU supervisionado

#### Scenario: Daemon ocioso
- **WHEN** o daemon fica rodando por 10 minutos sem nenhuma chamada D-Bus
- **THEN** só DE/E1/E0 do fluxo de CPU podem ser enviados espontaneamente

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

### Requirement: Erros D-Bus
Uma chamada que falhar SHALL devolver um erro D-Bus com nome sob `io.github.leoodaviid.Halo1.Error` e uma mensagem em português. Os nomes MUST distinguir: argumento inválido (`InvalidArgument`), aparelho ausente (`NotFound`), falta de permissão (`PermissionDenied`, com a indicação da regra udev), aparelho ocupado (`Busy`), falha de comunicação ou modelo desconhecido (`Device`), perfil inexistente (`ProfileNotFound`) e falha ao ler ou gravar arquivos (`Storage`).

#### Scenario: Waterforce sem permissão
- **WHEN** um cliente pede para aplicar um LED e o hidraw do Waterforce não abre por falta de permissão
- **THEN** a chamada devolve `io.github.leoodaviid.Halo1.Error.PermissionDenied` com o caminho do hidraw e a menção à regra udev do halo

#### Scenario: Argumento inválido
- **WHEN** um cliente pede brilho 11
- **THEN** a chamada devolve `io.github.leoodaviid.Halo1.Error.InvalidArgument` e nada é enviado ao aparelho

### Requirement: Operações de um aparelho em série
O daemon SHALL atender as chamadas que abrem o hidraw de um mesmo aparelho uma de cada vez, na ordem em que chegaram entre chamadas de clientes. Um único feed vencido pode ocorrer entre chamadas; nenhuma transação em andamento pode ser intercalada. Enquanto uma chamada espera ou usa um aparelho, o daemon MUST continuar respondendo às chamadas que não usam aquele aparelho, como a lista de aparelhos, a telemetria e os perfis.

#### Scenario: Duas chamadas seguidas no Waterforce
- **WHEN** um cliente pede um preset e, antes de ele terminar, pede um LED
- **THEN** a sequência do preset termina antes de qualquer pacote do LED ser enviado; um E0 pode ocorrer entre as duas sequências

#### Scenario: Leitura durante uma escrita
- **WHEN** uma escrita no Waterforce está em andamento e um cliente lê a lista de perfis
- **THEN** a lista é devolvida sem esperar a escrita

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
