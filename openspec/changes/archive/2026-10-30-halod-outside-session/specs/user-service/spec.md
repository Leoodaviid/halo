## ADDED Requirements

### Requirement: Daemon independente da sessão gráfica
Com a unidade habilitada, `halod.service` SHALL subir com o gerenciador do systemd do usuário, e não com a sessão gráfica, e SHALL ser considerado pronto só quando o nome `io.github.leoodaviid.Halo1` aparece no D-Bus da sessão. Com o linger ligado, a unidade SHALL subir no boot e MUST NOT parar no logout. Subir o daemon SHALL permitir somente a exceção DE/E1/E0 de CPU supervisionada no X 240, sem reaplicar configuração de aparelhos.

#### Scenario: Login
- **WHEN** o usuário entra na sessão do GNOME com a unidade habilitada
- **THEN** `halod.service` fica ativo e o nome `io.github.leoodaviid.Halo1` tem dono, com o feed CPU supervisionado podendo abrir e fechar o X 240

#### Scenario: Logout com linger
- **WHEN** o linger está ligado, a regra udev dá o grupo `plugdev` ao Castor3, e o usuário sai da sessão
- **THEN** `halod.service` continua ativo, o journal não registra parada nem `E0` de 80 °C, e o aparelho continua recebendo o Tctl a cada 2 s na tela de login

#### Scenario: Boot com linger
- **WHEN** o linger está ligado e o PC liga até a tela de login, sem ninguém entrar
- **THEN** `halod.service` sobe sem sessão gráfica e passa a enviar o Tctl depois da janela inicial de 8 s

#### Scenario: Logout sem linger
- **WHEN** o linger está desligado e o usuário sai da última sessão
- **THEN** o gerenciador do usuário termina, a unidade para com a proteção de 80 °C, e o comportamento é o de antes desta change

#### Scenario: Registro do daemon
- **WHEN** o daemon está rodando pela unidade
- **THEN** as mensagens dele aparecem em `journalctl --user -u halod.service`

### Requirement: Conferência do linger e do grupo
O script de instalação SHALL conferir se o linger do usuário está ligado e se o usuário está no grupo `plugdev`. Quando algum faltar, o script SHALL avisar que o `halod` vai parar no logout, ou não vai alcançar o Castor3 fora da sessão, e mostrar o comando que resolve, sem executá-lo. A instalação do usuário MUST continuar.

#### Scenario: Linger desligado
- **WHEN** `loginctl show-user` informa `Linger=no`
- **THEN** o script avisa que, sem o linger, o daemon para no logout, mostra `loginctl enable-linger <usuário>` e termina a instalação com sucesso

#### Scenario: Fora do grupo plugdev
- **WHEN** o usuário não está no grupo `plugdev`
- **THEN** o script avisa que o Castor3 só abre com a sessão ativa, mostra o comando com sudo para entrar no grupo e termina a instalação com sucesso

#### Scenario: Tudo em dia
- **WHEN** o linger está ligado e o usuário está no grupo `plugdev`
- **THEN** o script não mostra nenhum desses dois comandos

### Requirement: Guarda da sessão gráfica
A instalação SHALL incluir `halod-session.service`, presa ao `graphical-session.target` e parada antes dele, que chama `Session.Begin()` ao subir e `Session.End()` ao parar, esperando a resposta. Sem resposta em 10 s, ou sem o daemon no barramento, SHALL rodar `halod --waterforce-cpu-safe` e terminar com falha, com prazo de parada que comporte os dois em sequência. A sessão gráfica SHALL esperar o fim dela, e a instalação SHALL iniciá-la numa sessão já ativa.

#### Scenario: Logout espera a proteção
- **WHEN** o usuário sai da sessão, e o `End()` demora 3 s para responder
- **THEN** o `graphical-session.target` só para depois da resposta, e o journal mostra a guarda parada antes do alvo

#### Scenario: Daemon fora do ar no logout
- **WHEN** o usuário sai da sessão e o nome `io.github.leoodaviid.Halo1` não tem dono
- **THEN** a guarda roda o `halod --waterforce-cpu-safe`, registra a falha da chamada e termina com código de falha

#### Scenario: Login
- **WHEN** o usuário entra na sessão
- **THEN** a guarda fica ativa depois do `graphical-session.target` e chama o `Begin()`

#### Scenario: Daemon travado no logout
- **WHEN** o `End()` não responde e o nó continua ocupado pelo `flock` de outro processo por alguns segundos
- **THEN** a chamada desiste em 10 s, o helper roda com o orçamento inteiro de 18 s, e o systemd não o mata antes do fim

#### Scenario: Instalação numa sessão aberta
- **WHEN** o usuário instala pela primeira vez esta versão com a sessão gráfica ativa
- **THEN** ao fim do script a guarda está ativa, o `Begin()` foi chamado, e o feed não começa pausado

### Requirement: D-Bus da sessão que reinicia
Quando a conexão com o D-Bus da sessão cair, o daemon SHALL continuar enviando a CPU e renovando o watchdog, e SHALL reconectar no mesmo processo, tentando uma vez por segundo, com os mesmos objetos, o estado em memória deles e o mesmo nome. Sem o nome em 10 s, o daemon SHALL sair com falha. Reconectado, SHALL reler o estado da guarda. MUST NOT alterar unidades do GNOME.

#### Scenario: Logout no GNOME com acesso pelo grupo
- **WHEN** o linger está ligado, o Castor3 tem o grupo `plugdev`, o usuário sai, e o `gnome-session-restart-dbus.service` para o `dbus.service`
- **THEN** o `MainPID` e o `NRestarts` de `halod.service` continuam os mesmos, o journal registra a queda e a reconexão uma vez cada, sem `Watchdog timeout`, o nome volta a ter dono, e nenhum `E0` de 80 °C é enviado

#### Scenario: Envio durante a queda
- **WHEN** a conexão com o D-Bus está morta e o Waterforce está presente
- **THEN** cada tick envia o Tctl e renova o watchdog, e as emissões de sinal que falham não interrompem o tick

#### Scenario: Estado preservado
- **WHEN** o daemon reconecta
- **THEN** a lista de aparelhos, a telemetria, o `LastApplied` do LED e o perfil ativo lidos pelo D-Bus são os de antes da queda

#### Scenario: Barramento que não volta
- **WHEN** o daemon não adquire o nome em 10 s depois da queda, porque o barramento não sobe ou o nome está com outro processo
- **THEN** o daemon registra o motivo e sai com falha, o `ExecStopPost` envia o `E0` de 80 °C, e a unidade é reiniciada

#### Scenario: Begin perdido durante a reconexão
- **WHEN** o feed está pausado por sessão e a guarda fica ativa enquanto o daemon está sem conexão
- **THEN** depois de reconectar, o daemon lê a guarda ativa e retoma o feed com E1 e 8 s em 80 °C, como o `Begin()`

## MODIFIED Requirements

### Requirement: Atualização
Rodar o script de novo com o halo já instalado SHALL substituir os binários, a unidade e a ativação, e SHALL reiniciar o daemon, para que a versão nova passe a atender. Os perfis e os snapshots MUST NOT mudar. A unidade antiga SHALL ser parada antes da substituição, enquanto binário e helper ainda são compatíveis; binário e unidade novos SHALL ser instalados como um par compatível. Uma habilitação antiga pelo alvo da sessão gráfica SHALL ser trocada pela do alvo novo, sem sobrar as duas.

#### Scenario: Reinstalar com o daemon rodando
- **WHEN** o daemon está rodando e o usuário roda o script de novo depois de atualizar o código
- **THEN** o processo do daemon é outro, executa o binário novo, e os arquivos em `~/.config/halo` e `~/.local/state/halo` continuam iguais

#### Scenario: Migração da habilitação
- **WHEN** a unidade instalada antes estava habilitada em `graphical-session.target.wants`
- **THEN** depois do script o `halod.service` está habilitado só em `default.target.wants`, a `halod-session.service` em `graphical-session.target.wants`, e `systemctl --user is-enabled` responde `enabled` para as duas

### Requirement: Proteção na unidade de usuário
halod.service SHALL executar halod --waterforce-cpu-safe em ExecStopPost, incluindo falha de startup, SIGKILL, OOM do principal, panic, parada pelo usuário e o fim do gerenciador do usuário (desligamento, ou logout sem linger). SHALL usar TimeoutStopSec=20s e não mascarar falha do helper. A proteção MUST NOT exigir root, outra unidade, linger ou mudança das permissões udev; o linger e o grupo `plugdev` só servem para o daemon continuar fora da sessão.

#### Scenario: Principal morto
- **WHEN** o principal termina sem cleanup e o gerenciador consegue executar processos
- **THEN** ExecStopPost opera sem D-Bus, tem prazo de 18 s e registra sucesso, ausência ou falha de proteção

#### Scenario: Desligamento
- **WHEN** o PC desliga com o daemon rodando
- **THEN** o gerenciador do usuário para a unidade, e o último `E0` enviado é o de 80 °C

#### Scenario: Falha sistêmica
- **WHEN** o gerenciador morre, o kernel trava ou OOM global impede executar o helper
- **THEN** a documentação identifica ausência de garantia; a unidade não promete recuperar esses casos

### Requirement: Watchdog ligado ao progresso
A unidade SHALL usar Type=notify, WatchdogSec=20s, WatchdogSignal=SIGKILL e NotifyAccess=main. O principal SHALL chamar sd_notify com READY=1 só depois de adquirir o nome `io.github.leoodaviid.Halo1`, e com WATCHDOG=1 somente por novo ciclo saudável de telemetria e envio CPU aceito, ou scan com aparelho ausente, com feed desabilitado pelo modelo lido no DE, ou com o feed pausado por sessão sem um Tctl como último `E0` aceito; MUST NOT renovar por timer independente ou repetir um sucesso antigo.

#### Scenario: Worker bloqueado com tick vivo
- **WHEN** ticks continuam, mas o feed presente não conclui novos envios
- **THEN** cessam pings; o watchdog mata o principal e ExecStopPost tenta E0 alto antes de Restart=on-failure

#### Scenario: Processo suspenso por SIGSTOP
- **WHEN** o principal inteiro recebe STOP e o gerenciador permanece ativo
- **THEN** o watchdog usa SIGKILL após expirar, sem depender de SIGTERM ou core dump, e executa o helper

#### Scenario: Modelo sem feed
- **WHEN** o DE da sessão lógica informa modelo 1, 2 ou fora da tabela
- **THEN** o scan concluído renova o watchdog sem E0, como na ausência, e as operações legadas continuam disponíveis

#### Scenario: Cooler ausente
- **WHEN** o scan avança e confirma ausência do Waterforce
- **THEN** o watchdog recebe progresso legítimo e não reinicia o daemon apenas por falta de aparelho

#### Scenario: Pausa antes da proteção aceita
- **WHEN** o `End()` pausou o feed, o último `E0` aceito foi um Tctl, e a proteção de 80 °C ainda não foi aceita (worker preso) ou falhou
- **THEN** o scan não renova o watchdog, e o daemon é morto pelo prazo, passando pelo `ExecStopPost`

#### Scenario: Feed pausado por sessão
- **WHEN** o feed está pausado depois do `End()`, com o Waterforce presente, e a tela de login fica aberta por mais de 20 s
- **THEN** cada scan concluído renova o watchdog sem E0, e o daemon não é morto nem reiniciado; depois do `Begin()`, o progresso volta a exigir envios aceitos

#### Scenario: Pronto só com o nome
- **WHEN** a unidade inicia o daemon
- **THEN** o systemd só considera a unidade ativa depois de o nome `io.github.leoodaviid.Halo1` ter dono, e o READY=1 nunca vem antes disso

#### Scenario: Ambiente watchdog incompatível
- **WHEN** o processo não dispõe de supervisão efetiva ou seu ambiente é inválido
- **THEN** não inicia E0 frio nem aceita SetFanCurve; leituras e operações legadas continuam disponíveis

## REMOVED Requirements

### Requirement: Daemon sobe com a sessão gráfica
**Reason**: A unidade deixa de seguir a sessão gráfica, para continuar alimentando a CPU na tela de login e antes do primeiro login. Ver "Daemon independente da sessão gráfica".
**Migration**: O `packaging/install.sh` troca a habilitação para `default.target`. O linger (`loginctl enable-linger`) é o que mantém o daemon fora da sessão; sem ele, o comportamento continua o de antes.
