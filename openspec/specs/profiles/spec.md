# profiles Specification

## Purpose

Guardar configurações nomeadas dos aparelhos (perfis), editá-las pelo D-Bus e aplicá-las de uma vez, sem gravar na flash o que já está no aparelho.

## Requirements

### Requirement: Local dos perfis
Cada perfil SHALL ser um arquivo TOML em `$XDG_CONFIG_HOME/halo/profiles/<nome>.toml` (ou `~/.config/halo/profiles/` sem a variável). O nome do perfil ativo SHALL ficar em `$XDG_STATE_HOME/halo/active-profile` (ou `~/.local/state/halo/` sem a variável).

#### Scenario: Perfil salvo
- **WHEN** um cliente salva o perfil `noite`, sem `XDG_CONFIG_HOME` definido
- **THEN** o arquivo `~/.config/halo/profiles/noite.toml` passa a existir com o conteúdo do perfil

### Requirement: Nome do perfil
O nome de um perfil SHALL ter de 1 a 32 caracteres, só letras minúsculas ASCII, dígitos e hífen, começando por letra ou dígito. Um nome fora dessa regra MUST ser recusado com `InvalidArgument` sem ler nem gravar arquivos.

#### Scenario: Nome com barra
- **WHEN** um cliente salva um perfil com o nome `../x`
- **THEN** a chamada devolve `InvalidArgument` e nenhum arquivo é criado

### Requirement: Conteúdo do perfil
Um perfil SHALL ter seções opcionais por aparelho. Nesta versão: `waterforce.led`, com efeito, cores, brilho e velocidade como na interface `Led`, e `waterforce.speed`, com `fan` e `pump`, cada um opcional e igual a um preset aceito ou a um RPM de curva plana. Um perfil sem nenhuma seção é válido e não faz nada ao ser ativado.

#### Scenario: Perfil com as duas seções
- **WHEN** o perfil tem `waterforce.led` com `static` e `ff6600` e `waterforce.speed` com `fan = "quiet"` e `pump = "turbo"`
- **THEN** o perfil é aceito

#### Scenario: Velocidade com RPM
- **WHEN** o perfil tem `waterforce.speed` com `fan = 1500` e `pump = "balanced"`
- **THEN** o perfil é aceito, com a fan numa curva plana de 1500 RPM

### Requirement: Validação ao salvar e ao ler
O daemon SHALL validar o perfil inteiro ao salvar, com as mesmas regras da CLI para efeito, cores, brilho, velocidade, presets e faixas de RPM. Um perfil inválido MUST ser recusado com `InvalidArgument` sem alterar o arquivo existente. Um arquivo inválido encontrado no disco SHALL ser informado ao ler ou ativar, sem enviar nada ao hardware.

#### Scenario: RPM fora da faixa
- **WHEN** um cliente salva um perfil com `pump = 2000`
- **THEN** a chamada devolve `InvalidArgument` e o arquivo anterior desse perfil continua igual

#### Scenario: Arquivo editado à mão com erro
- **WHEN** o arquivo `noite.toml` tem brilho 15 e um cliente ativa `noite`
- **THEN** a chamada devolve `InvalidArgument` com o nome do arquivo e nada é enviado ao hardware

### Requirement: Operações dos perfis
A interface `io.github.leoodaviid.Halo1.Profiles`, no objeto `/io/github/leoodaviid/Halo1`, SHALL oferecer listar os nomes, ler, salvar (criando ou substituindo), apagar e ativar um perfil, e uma propriedade com o nome do perfil ativo, vazia quando não houver. Ler, apagar ou ativar um perfil inexistente MUST devolver `ProfileNotFound`.

#### Scenario: Apagar o perfil ativo
- **WHEN** um cliente apaga o perfil que está ativo
- **THEN** o arquivo é removido, a propriedade do perfil ativo fica vazia e um sinal de mudança é emitido

#### Scenario: Ler perfil inexistente
- **WHEN** um cliente lê o perfil `nada` e ele não existe
- **THEN** a chamada devolve `ProfileNotFound`

### Requirement: Gravação atômica
Salvar um perfil e o nome do perfil ativo SHALL substituir o arquivo de forma atômica, para que uma falha no meio nunca deixe um arquivo pela metade.

#### Scenario: Falha ao gravar
- **WHEN** a gravação do novo conteúdo de `noite.toml` falha
- **THEN** o `noite.toml` anterior continua inteiro e a chamada devolve `Storage`

### Requirement: Ativação da velocidade só quando difere
Ao ativar um perfil com `waterforce.speed`, o daemon SHALL ler o estado com `DE 00`, `DD`, `D9 01` e `D9 02` e escrever só se ele diferir do perfil. Um lado em preset é igual quando o modo é o mesmo. Um lado em RPM é igual quando o modo é Customized e a curva do `D9` é a curva plana daquele RPM. Um lado ausente no perfil mantém o modo lido. Se nada diferir, o daemon MUST NOT enviar `E5`, `E6` nem `B6`.

#### Scenario: Velocidade já igual
- **WHEN** o perfil pede a fan em `quiet` e a bomba em `turbo`, e o `DD` lido mostra a fan em Quiet (6) e a bomba em Turbo (4)
- **THEN** o daemon só envia `99 DE 00`, `99 DD`, `99 D9 01` e `99 D9 02` para a velocidade, e não grava snapshot

### Requirement: Sequência da ativação da velocidade
Quando a velocidade diferir, o daemon SHALL gravar o snapshot anterior e enviar, com o estado desejado, a mesma sequência de escrita da capability `waterforce-speed`: `E5 01 <modo da fan>`, `E5 02 <modo da bomba>`, um `E6` para cada lado em curva plana (fan antes da bomba) e um único `B6`. Se o lado ausente estiver num modo que o halo não repete, MUST NOT escrever nada.

#### Scenario: Fan em curva e bomba em preset
- **WHEN** o perfil pede `fan = 1500` e `pump = "balanced"`, e o aparelho está com a fan em Performance
- **THEN** o daemon grava o snapshot e envia `99 E5 01 01 00`, `99 E5 02 00 00`, `99 E6 00 01 00 05 DC 1E 05 DC 32 05 DC 50 05 DC` e `99 B6`

#### Scenario: Lado ausente em FixedRPM
- **WHEN** o perfil só define a fan e o `DD` mostra a bomba no modo 3
- **THEN** o daemon não envia `E5`, `E6` nem `B6` e informa o motivo com `InvalidArgument`

### Requirement: Ativação do LED
Ao ativar um perfil com `waterforce.led`, o daemon SHALL aplicar a seção com a mesma sequência da interface `Led`, porque o protocolo não permite saber se o LED já está igual. A seção de velocidade SHALL ser aplicada antes da de LED.

#### Scenario: Perfil com LED
- **WHEN** um cliente ativa um perfil com `waterforce.led` em `cycle`
- **THEN** o daemon envia a sequência do `cycle` com um único `B6`

### Requirement: Resultado da ativação
O daemon SHALL tentar todas as seções do perfil, mesmo que uma falhe, e só gravar o perfil como ativo se todas tiverem dado certo. Se alguma falhar, a chamada SHALL devolver o erro da primeira falha, com a lista das seções aplicadas e das que falharam.

#### Scenario: LED falha e velocidade dá certo
- **WHEN** a velocidade é aplicada e o LED falha porque o modelo não foi reconhecido
- **THEN** a chamada devolve `Device` dizendo que `waterforce.speed` foi aplicada e `waterforce.led` falhou, e o perfil ativo não muda

#### Scenario: Tudo certo
- **WHEN** todas as seções são aplicadas
- **THEN** o arquivo `active-profile` passa a ter o nome do perfil e a propriedade do perfil ativo muda, com sinal
