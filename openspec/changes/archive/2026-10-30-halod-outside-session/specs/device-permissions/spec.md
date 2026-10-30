## MODIFIED Requirements

### Requirement: Dispositivos cobertos
A regra udev do halo SHALL dar `TAG+="uaccess"` aos nós hidraw do IT5702 (`048d:5702`) e do Castor3 (`1044:7a4d`). O nó do Castor3 SHALL ter também o grupo `plugdev` com modo `0660`, para o `halod` continuar alcançando o aparelho sem sessão ativa. A regra MUST NOT incluir dispositivos que o halo não usa, barramentos I2C/SMBus nem o teclado Redragon (`258a:013b`).

#### Scenario: Acesso sem a regra do OpenRGB
- **WHEN** a regra do halo está instalada e o `60-openrgb.rules` não está
- **THEN** o usuário da sessão consegue abrir os hidraw do IT5702 e do Castor3 sem root

#### Scenario: Castor3 sem sessão ativa
- **WHEN** a regra do halo está instalada, o usuário está no grupo `plugdev` e a tela de login é a sessão ativa do assento
- **THEN** um processo desse usuário consegue abrir o hidraw do Castor3 para leitura e escrita, e o do IT5702 continua só com a ACL da sessão

#### Scenario: Outros dispositivos intactos
- **WHEN** a regra do halo está instalada
- **THEN** as permissões dos hidraw do teclado e dos `/dev/i2c-*` não mudam por causa dela
