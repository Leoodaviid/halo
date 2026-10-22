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
O daemon SHALL enviar comandos a um aparelho só como resposta a uma chamada D-Bus de um cliente. Na inicialização, nas tarefas periódicas e no resume, ele MUST NOT abrir nenhum hidraw nem enviar nenhum comando.

#### Scenario: Inicialização
- **WHEN** o daemon é iniciado com um perfil ativo salvo
- **THEN** ele não abre o hidraw de nenhum aparelho e não reaplica o perfil

#### Scenario: Daemon ocioso
- **WHEN** o daemon fica rodando por 10 minutos sem nenhuma chamada D-Bus
- **THEN** nenhum comando é enviado a nenhum aparelho

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

### Requirement: Tarefas periódicas só leem o sysfs
O daemon SHALL rodar tarefas periódicas a cada 2 segundos, o intervalo mínimo da telemetria do kernel. Nesta versão, essas tarefas MUST só ler o sysfs: a presença dos aparelhos e a telemetria do hwmon.

#### Scenario: Ciclo periódico
- **WHEN** passam 2 segundos desde a última verificação
- **THEN** o daemon lê a presença e a telemetria no sysfs e não abre nenhum hidraw

### Requirement: Resume
Quando o logind informar o fim de uma suspensão, o daemon SHALL atualizar na hora a presença dos aparelhos e a telemetria, sem esperar o próximo ciclo e sem enviar nada ao hardware. Se o D-Bus do sistema ou o logind não estiverem disponíveis, o daemon SHALL continuar funcionando e avisar no log que não vai acompanhar o resume.

#### Scenario: Volta da suspensão
- **WHEN** o logind emite `PrepareForSleep` com `false`
- **THEN** o daemon verifica presença e telemetria em seguida e não envia nenhum comando a nenhum aparelho

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
O daemon SHALL atender as chamadas que abrem o hidraw de um mesmo aparelho uma de cada vez, na ordem em que chegaram. Enquanto uma chamada espera ou usa um aparelho, o daemon MUST continuar respondendo às chamadas que não usam aquele aparelho, como a lista de aparelhos, a telemetria e os perfis.

#### Scenario: Duas chamadas seguidas no Waterforce
- **WHEN** um cliente pede um preset e, antes de ele terminar, pede um LED
- **THEN** a sequência do preset termina antes de qualquer pacote do LED ser enviado

#### Scenario: Leitura durante uma escrita
- **WHEN** uma escrita no Waterforce está em andamento e um cliente lê a lista de perfis
- **THEN** a lista é devolvida sem esperar a escrita
