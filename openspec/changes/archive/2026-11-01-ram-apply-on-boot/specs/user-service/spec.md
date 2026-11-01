# Spec Delta

## MODIFIED Requirements

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
- **THEN** o script não mostra nenhum comando com sudo para a regra udev

## ADDED Requirements

### Requirement: Conferência das peças de sistema da RAM
O script de instalação SHALL conferir se `/usr/local/libexec/halo/halo` é igual ao `halo` compilado e se `halo-ram-restore.service` e `halo-ram-sleep.service` em `/etc/systemd/system/` são iguais às do repositório. Se alguma faltar ou diferir, SHALL mostrar, sem executar, os comandos com `sudo` para instalar as três, o `daemon-reload` e o `enable` das unidades. A instalação do usuário MUST continuar, sem executar `sudo`.

#### Scenario: Peças ausentes
- **WHEN** nenhuma das três peças está instalada
- **THEN** o script avisa que a RAM só é reaplicada no boot e ao acordar com elas, mostra os comandos `sudo install`, `sudo systemctl daemon-reload` e `sudo systemctl enable` e termina a instalação do usuário com sucesso

#### Scenario: Binário do root desatualizado
- **WHEN** as unidades estão em dia, mas `/usr/local/libexec/halo/halo` difere do `halo` compilado
- **THEN** o script avisa que a cópia do root está desatualizada e mostra o comando para substituí-la

#### Scenario: Peças em dia
- **WHEN** as três peças são iguais às do repositório e do build
- **THEN** o script não mostra nenhum comando com `sudo` para a RAM

### Requirement: Remoção das peças de sistema da RAM
Na desinstalação, quando alguma das três peças de sistema da RAM existir, o script SHALL mostrar, sem executá-los, os comandos com `sudo` para desabilitar as duas unidades e remover as três peças. Ele MUST NOT apagar `/var/lib/halo` e SHALL dizer que o estado salvo continua lá.

#### Scenario: Desinstalar com as peças de sistema
- **WHEN** as unidades e a cópia do root estão instaladas e o usuário desinstala
- **THEN** o script mostra `sudo systemctl disable` e os `sudo rm` das três peças, não executa nenhum, e diz que `/var/lib/halo` continua lá
