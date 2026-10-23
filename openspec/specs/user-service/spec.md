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
Com a unidade habilitada, o `halod` SHALL subir quando a sessão gráfica do usuário começa e SHALL ser considerado pronto só quando o nome `io.github.leoodaviid.Halo1` aparece no D-Bus da sessão. Subir o daemon MUST NOT enviar nenhum comando a nenhum aparelho.

#### Scenario: Login
- **WHEN** o usuário entra na sessão do GNOME com a unidade habilitada
- **THEN** `halod.service` fica ativo e o nome `io.github.leoodaviid.Halo1` tem dono, sem nenhum hidraw aberto pelo daemon

#### Scenario: Registro do daemon
- **WHEN** o daemon está rodando pela unidade
- **THEN** as mensagens dele aparecem em `journalctl --user -u halod.service`

### Requirement: Ativação pelo D-Bus
Uma chamada D-Bus a `io.github.leoodaviid.Halo1` com o daemon parado SHALL subir o daemon pela unidade `halod.service` e ser atendida, mesmo com a unidade desabilitada.

#### Scenario: Chamada com o daemon parado
- **WHEN** o daemon está parado e um cliente chama `Manager.Devices`
- **THEN** o systemd do usuário inicia `halod.service` e o cliente recebe a lista de aparelhos

### Requirement: Reinício depois de falha
Se o `halod` terminar com erro, o systemd do usuário SHALL reiniciá-lo depois de uma pausa e SHALL desistir depois de falhas seguidas, para não reiniciar em laço.

#### Scenario: Outra instância com o nome
- **WHEN** um `halod` iniciado à mão já é dono do nome e a unidade é iniciada
- **THEN** a unidade falha algumas vezes, para de tentar, e o `halod` iniciado à mão continua dono do nome

### Requirement: Atualização
Rodar o script de novo com o halo já instalado SHALL substituir os binários, a unidade e a ativação, e SHALL reiniciar o daemon, para que a versão nova passe a atender. Os perfis e os snapshots MUST NOT mudar.

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
