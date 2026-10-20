# Spec Delta

## Purpose

Liberar ao usuário da sessão local o acesso aos controladores que o halo usa, com uma regra udev do próprio projeto, sem depender de regras instaladas por outros programas.

## ADDED Requirements

### Requirement: Dispositivos cobertos
A regra udev do halo SHALL dar `TAG+="uaccess"` aos nós hidraw do IT5702 (`048d:5702`) e do Castor3 (`1044:7a4d`). A regra MUST NOT incluir dispositivos que o halo não usa, barramentos I2C/SMBus nem o teclado Redragon (`258a:013b`).

#### Scenario: Acesso sem a regra do OpenRGB
- **WHEN** a regra do halo está instalada e o `60-openrgb.rules` não está
- **THEN** o usuário da sessão consegue abrir os hidraw do IT5702 e do Castor3 sem root

#### Scenario: Outros dispositivos intactos
- **WHEN** a regra do halo está instalada
- **THEN** as permissões dos hidraw do teclado e dos `/dev/i2c-*` não mudam por causa dela

### Requirement: Ordem de aplicação
O arquivo de regra SHALL ter prioridade menor que 73, para que a tag `uaccess` seja aplicada pela `73-seat-late.rules` do systemd.

#### Scenario: Nome do arquivo
- **WHEN** a regra é instalada em `/etc/udev/rules.d/`
- **THEN** o nome do arquivo começa com um número menor que 73

### Requirement: Instalação documentada
O projeto SHALL documentar como instalar, recarregar e remover a regra, e como verificar o resultado.

#### Scenario: Verificação depois de instalar
- **WHEN** o usuário segue as instruções de instalação
- **THEN** `getfacl` no hidraw do Castor3 mostra `rw` para o usuário
