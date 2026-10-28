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

### Requirement: Daemon sobe com a sessão gráfica
Com a unidade habilitada, o `halod` SHALL subir quando a sessão gráfica do usuário começa e SHALL ser considerado pronto só quando o nome `io.github.leoodaviid.Halo1` aparece no D-Bus da sessão. Subir o daemon SHALL permitir somente a exceção DE/E1/E0 de CPU supervisionada no X 240, sem reaplicar configuração de aparelhos.

#### Scenario: Login
- **WHEN** o usuário entra na sessão do GNOME com a unidade habilitada
- **THEN** `halod.service` fica ativo e o nome `io.github.leoodaviid.Halo1` tem dono, com o feed CPU supervisionado podendo abrir e fechar o X 240

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
Rodar o script de novo com o halo já instalado SHALL substituir os binários, a unidade e a ativação, e SHALL reiniciar o daemon, para que a versão nova passe a atender. Os perfis e os snapshots MUST NOT mudar. A unidade antiga SHALL ser parada antes da substituição, enquanto binário e helper ainda são compatíveis; binário e unidade novos SHALL ser instalados como um par compatível.

#### Scenario: Reinstalar com o daemon rodando
- **WHEN** o daemon está rodando e o usuário roda o script de novo depois de atualizar o código
- **THEN** o processo do daemon é outro, executa o binário novo, e os arquivos em `~/.config/halo` e `~/.local/state/halo` continuam iguais

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
halod.service SHALL executar halod --waterforce-cpu-safe em ExecStopPost, incluindo falha de startup, SIGKILL, OOM do principal, panic e parada por PartOf. SHALL usar TimeoutStopSec=20s e não mascarar falha do helper. MUST NOT exigir root, outra unidade, linger ou mudança das permissões udev.

#### Scenario: Principal morto
- **WHEN** o principal termina sem cleanup e o gerenciador consegue executar processos
- **THEN** ExecStopPost opera sem D-Bus, tem prazo de 18 s e registra sucesso, ausência ou falha de proteção

#### Scenario: Falha sistêmica
- **WHEN** o gerenciador morre, o kernel trava ou OOM global impede executar o helper
- **THEN** a documentação identifica ausência de garantia; a unidade não promete recuperar esses casos

### Requirement: Watchdog ligado ao progresso
A unidade SHALL manter Type=dbus e usar WatchdogSec=20s, WatchdogSignal=SIGKILL e NotifyAccess=main. O principal SHALL chamar sd_notify com WATCHDOG=1 somente por novo ciclo saudável de telemetria e envio CPU aceito, ou scan com aparelho ausente ou com feed desabilitado pelo modelo lido no DE; MUST NOT renovar por timer independente ou repetir um sucesso antigo.

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

#### Scenario: Ambiente watchdog incompatível
- **WHEN** o processo não dispõe de supervisão efetiva ou seu ambiente é inválido
- **THEN** não inicia E0 frio nem aceita SetFanCurve; leituras e operações legadas continuam disponíveis

### Requirement: Prova de recuperação observada
O aceite da implementação SHALL incluir log no aparelho com DA e journal em SIGKILL e travamento do principal, além de bloqueio do worker com tick ativo. SHALL distinguir E0 seguro do helper de envios do daemon reiniciado e não declarar proteção só com sucesso de write ou restart.

#### Scenario: Recuperação medida
- **WHEN** cada ensaio aprovado mata ou trava o alimentador após um patamar frio conhecido
- **THEN** os logs mostram a transição ao duty de E0 seguro após o E0 do helper e antes do primeiro E0 de qualquer temperatura do novo daemon, com tempos e curva registrados

#### Scenario: Helper sem prova isolada
- **WHEN** o primeiro DA de recuperação só chega depois do primeiro E0 do processo novo
- **THEN** o ensaio não prova ExecStopPost e não satisfaz o aceite, mesmo que o serviço tenha reiniciado e o duty esteja alto
