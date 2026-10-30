# user-service Specification

## Purpose

Instalar, atualizar e remover o halo no usuário, sem root, e fazer o daemon subir sozinho com a sessão gráfica ou quando um cliente o chama pelo D-Bus.

## Requirements

### Requirement: Instalação no usuário sem root
O projeto SHALL ter um script de instalação que compila o `halo` e o `halod` e instala, só no diretório do usuário, os dois binários, a unidade `halod.service` do systemd do usuário e o arquivo de ativação D-Bus de `io.github.leoodaviid.Halo1`. O script MUST NOT pedir sudo nem escrever fora do `$HOME`.

#### Scenario: Primeira instalação
- **WHEN** o usuário roda o script sem argumentos numa sessão gráfica
- **THEN** `halo` e `halod` ficam em `~/.local/bin`, a unidade e a ativação D-Bus ficam nos diretórios do usuário, e `systemctl --user is-enabled halod.service` responde `enabled`

#### Scenario: Nada fora do usuário
- **WHEN** o script termina
- **THEN** nenhum arquivo fora do `$HOME` e da pasta de compilação do repositório foi criado ou alterado, e nenhum comando foi executado com sudo

#### Scenario: Compilação falha
- **WHEN** a compilação dos binários falha
- **THEN** o script termina com código diferente de zero sem ter trocado os binários, a unidade ou a ativação já instalados

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

### Requirement: Ativação pelo D-Bus
Uma chamada D-Bus a `io.github.leoodaviid.Halo1` com o daemon parado SHALL subir o daemon pela unidade `halod.service` e ser atendida, mesmo com a unidade desabilitada.

#### Scenario: Chamada com o daemon parado
- **WHEN** o daemon está parado e um cliente chama `Manager.Devices`
- **THEN** o systemd do usuário inicia `halod.service` e o cliente recebe a lista de aparelhos

### Requirement: Reinício depois de falha
Se o `halod` terminar com erro, sofrer SIGKILL/OOM/panic ou falhar no watchdog, o systemd do usuário SHALL reiniciá-lo depois de executar ExecStopPost e de uma pausa de 2 s e SHALL desistir depois de falhas seguidas, para não reiniciar em laço. A unidade SHALL declarar StartLimitIntervalSec=180s e StartLimitBurst=5, porque o helper torna cada ciclo de falha de startup mais longo que o intervalo padrão de 10 s.

#### Scenario: Outra instância com o nome
- **WHEN** um `halod` iniciado à mão já é dono do nome e a unidade é iniciada
- **THEN** a unidade falha algumas vezes, para de tentar, e o `halod` iniciado à mão continua dono do nome, mesmo com o X 240 presente e o helper esperando 8 s em cada falha

#### Scenario: Falha com último E0 frio
- **WHEN** o principal morre e o gerenciador do usuário continua funcionando
- **THEN** o helper tenta E0 de 80 °C antes do restart; se o limite de reinícios for atingido, a última tentativa de proteção ainda ocorre

#### Scenario: Parada explícita ou logout
- **WHEN** o usuário para a unidade ou PartOf propaga parada da sessão gráfica
- **THEN** ExecStopPost é tentado sem provocar restart automático

### Requirement: Atualização
Rodar o script de novo com o halo já instalado SHALL substituir os binários, a unidade e a ativação, e SHALL reiniciar o daemon, para que a versão nova passe a atender. Os perfis e os snapshots MUST NOT mudar. A unidade antiga SHALL ser parada antes da substituição, enquanto binário e helper ainda são compatíveis; binário e unidade novos SHALL ser instalados como um par compatível. Uma habilitação antiga pelo alvo da sessão gráfica SHALL ser trocada pela do alvo novo, sem sobrar as duas.

#### Scenario: Reinstalar com o daemon rodando
- **WHEN** o daemon está rodando e o usuário roda o script de novo depois de atualizar o código
- **THEN** o processo do daemon é outro, executa o binário novo, e os arquivos em `~/.config/halo` e `~/.local/state/halo` continuam iguais

#### Scenario: Migração da habilitação
- **WHEN** a unidade instalada antes estava habilitada em `graphical-session.target.wants`
- **THEN** depois do script o `halod.service` está habilitado só em `default.target.wants`, a `halod-session.service` em `graphical-session.target.wants`, e `systemctl --user is-enabled` responde `enabled` para as duas

### Requirement: Desinstalação preserva perfis e snapshots
O script SHALL ter uma opção de desinstalação que para e desabilita a unidade e remove os binários, a unidade e a ativação que ele instalou. Ela MUST NOT apagar os perfis, o perfil ativo nem os snapshots de velocidade, e SHALL informar onde eles ficaram.

#### Scenario: Desinstalar
- **WHEN** o usuário roda o script com a opção de desinstalação
- **THEN** `halod.service` não existe mais para o systemd do usuário, os binários e a ativação D-Bus foram removidos, e `~/.config/halo` e `~/.local/state/halo` continuam como estavam

#### Scenario: Snapshot original preservado
- **WHEN** existe `~/.local/state/halo/waterforce-speed.original` e o usuário desinstala
- **THEN** o arquivo continua lá, com o mesmo conteúdo

#### Scenario: Desinstalar sem nada instalado
- **WHEN** o halo não está instalado e o usuário roda a desinstalação
- **THEN** o script termina com sucesso e informa que não havia nada a remover

### Requirement: Conferência da regra udev
O script SHALL conferir se a regra udev do halo está instalada em `/etc/udev/rules.d/` com o mesmo conteúdo da do repositório. Quando ela faltar ou for diferente, o script SHALL mostrar os comandos de instalação com sudo, sem executá-los, e a instalação do usuário MUST continuar.

#### Scenario: Regra ausente
- **WHEN** `/etc/udev/rules.d/70-halo.rules` não existe
- **THEN** o script avisa que o LED e a velocidade do Waterforce precisam da regra, mostra os comandos de instalação e termina a instalação do usuário com sucesso

#### Scenario: Regra desatualizada
- **WHEN** a regra instalada difere da do repositório
- **THEN** o script avisa que ela está desatualizada e mostra os comandos para substituí-la

#### Scenario: Regra em dia
- **WHEN** a regra instalada é igual à do repositório
- **THEN** o script não mostra nenhum comando com sudo

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

### Requirement: Instalação da extensão do GNOME
O script de instalação SHALL instalar a extensão do halo no diretório de extensões do usuário, com o schema das preferências compilado, e habilitá-la nas configurações do GNOME, mesmo com o Shell em execução sem conhecer a extensão. Quando a extensão instalada mudar, o script SHALL avisar que é preciso sair e entrar na sessão para carregá-la; quando não mudar, MUST NOT pedir isso.

#### Scenario: Primeira instalação da extensão
- **WHEN** o usuário roda o script e a extensão ainda não está instalada
- **THEN** a extensão fica em `~/.local/share/gnome-shell/extensions/halo@leoodaviid.github.io`, com o schema compilado na pasta `schemas/` dela, o UUID entra na lista de extensões habilitadas do GNOME, e o script avisa que é preciso sair e entrar na sessão

#### Scenario: Reinstalar sem mudar a extensão
- **WHEN** o usuário roda o script de novo e a extensão do repositório é igual à instalada
- **THEN** o script não pede para sair e entrar na sessão

#### Scenario: Sem o compilador de schemas
- **WHEN** o usuário roda o script e o `glib-compile-schemas` não está instalado
- **THEN** o script falha antes de trocar a extensão instalada, com uma mensagem que diz o que instalar, e a extensão instalada antes continua como estava

#### Scenario: Habilitar sem sessão gráfica
- **WHEN** o script não consegue gravar a lista de extensões habilitadas
- **THEN** ele avisa como habilitar a extensão depois e termina a instalação com sucesso

#### Scenario: Habilitar sem duplicar
- **WHEN** a extensão já está na lista de extensões habilitadas e o usuário reinstala
- **THEN** a lista continua com o UUID uma vez só, e as outras extensões do usuário continuam habilitadas

### Requirement: Remoção da extensão do GNOME
A desinstalação SHALL tirar a extensão da lista de extensões habilitadas, sem mexer nas outras, e remover a pasta dela.

#### Scenario: Desinstalar com a extensão
- **WHEN** a extensão está instalada e habilitada e o usuário desinstala
- **THEN** a pasta da extensão foi removida, o UUID saiu da lista de habilitadas, e as outras extensões continuam na lista

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

### Requirement: Prova de recuperação observada
O aceite da implementação SHALL incluir log no aparelho com DA e journal em SIGKILL e travamento do principal, além de bloqueio do worker com tick ativo. SHALL distinguir E0 seguro do helper de envios do daemon reiniciado e não declarar proteção só com sucesso de write ou restart.

#### Scenario: Recuperação medida
- **WHEN** cada ensaio aprovado mata ou trava o alimentador após um patamar frio conhecido
- **THEN** os logs mostram a transição ao duty de E0 seguro após o E0 do helper e antes do primeiro E0 de qualquer temperatura do novo daemon, com tempos e curva registrados

#### Scenario: Helper sem prova isolada
- **WHEN** o primeiro DA de recuperação só chega depois do primeiro E0 do processo novo
- **THEN** o ensaio não prova ExecStopPost e não satisfaz o aceite, mesmo que o serviço tenha reiniciado e o duty esteja alto
