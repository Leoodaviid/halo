## MODIFIED Requirements

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
