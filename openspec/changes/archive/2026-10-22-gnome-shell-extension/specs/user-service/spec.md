# Spec Delta

## ADDED Requirements

### Requirement: Instalação da extensão do GNOME
O script de instalação SHALL instalar a extensão do halo no diretório de extensões do usuário e habilitá-la nas configurações do GNOME, mesmo com o Shell em execução sem conhecer a extensão. Quando a extensão instalada mudar, o script SHALL avisar que é preciso sair e entrar na sessão para carregá-la; quando não mudar, MUST NOT pedir isso.

#### Scenario: Primeira instalação da extensão
- **WHEN** o usuário roda o script e a extensão ainda não está instalada
- **THEN** a extensão fica em `~/.local/share/gnome-shell/extensions/halo@leoodaviid.github.io`, o UUID entra na lista de extensões habilitadas do GNOME, e o script avisa que é preciso sair e entrar na sessão

#### Scenario: Reinstalar sem mudar a extensão
- **WHEN** o usuário roda o script de novo e a extensão do repositório é igual à instalada
- **THEN** o script não pede para sair e entrar na sessão

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
