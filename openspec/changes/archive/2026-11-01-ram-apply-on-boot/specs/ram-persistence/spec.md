# Spec Delta

## Purpose

Guardar o último estado aplicado à RAM e reaplicá-lo no boot e ao acordar, e apagar os pentes ao suspender, como o FURY CTRL faz, porque o pente não guarda o que recebe depois de um corte de energia.

## ADDED Requirements

### Requirement: Gravação do estado
Depois de aplicar com sucesso `halo led ram off`, `on`, `default` ou `static`, o sistema SHALL gravar o estado resultante (requisito "Estado depois de cada comando") no arquivo de estado (`/var/lib/halo/ram-led`), de forma atômica (arquivo temporário no mesmo diretório e `rename`), com modo `0644`, criando o diretório com modo `0755` se ele faltar. O `--dry-run`, o `restore` e o `suspend` MUST NOT gravar.

#### Scenario: Estático gravado
- **WHEN** `halo led ram static --color ffffff` aplica com sucesso nos pentes `0-0052` e `0-0053`
- **THEN** o arquivo de estado tem `version=1`, `effect=static`, `color=ffffff`, `brightness=80`, `power=on` e `spd=0-0052,0-0053`

#### Scenario: Simulação não grava
- **WHEN** o usuário executa `halo led ram static --color ffffff --dry-run`
- **THEN** o arquivo de estado não é criado nem alterado

#### Scenario: Falha nas escritas
- **WHEN** uma escrita da moldura falha nas 30 tentativas
- **THEN** o arquivo de estado continua como estava

### Requirement: Falha ao gravar o estado
Se as escritas derem certo e a gravação do estado falhar antes de trocar o arquivo, o sistema SHALL dizer que os pentes foram alterados mas o estado não foi salvo, com a causa, e o arquivo antigo MUST continuar inteiro. Se só o `fsync` do diretório falhar, depois da troca, SHALL dizer que o estado novo foi gravado mas pode não sobreviver a um corte de energia. Nos dois casos, o código de saída SHALL ser 5.

#### Scenario: Diretório sem permissão
- **WHEN** as escritas dão certo e o diretório do estado não aceita gravação
- **THEN** a mensagem diz que os pentes mudaram e que o estado não foi salvo, o estado antigo continua o mesmo e o código de saída é 5

### Requirement: Gravação serializada
O sistema SHALL manter o lock exclusivo do `/dev/i2c-N` (requisito "Um processo do halo por vez no barramento" da `ram-led`) desde as escritas até o fim da gravação do estado, para que dois `halo led ram` simultâneos gravem na mesma ordem em que escreveram nos pentes.

#### Scenario: Dois comandos ao mesmo tempo
- **WHEN** um `static --color ff0000` e um `static --color 0000ff` rodam ao mesmo tempo, e o azul toma o lock por último
- **THEN** os pentes ficam azuis e o estado gravado é `color=0000ff`

### Requirement: Formato do estado
O arquivo de estado SHALL ser texto, uma `chave=valor` por linha, com linhas em branco e comentários (`#`) permitidos. As chaves SHALL ser `version` (`1`), `effect` (`none`, `default` ou `static`), `power` (`on` ou `off`), `spd` (instâncias `B-00AA` separadas por vírgula, sem repetição) e, só com `effect=static`, `color` (`RRGGBB`) e `brightness` (0 a 100). Chave repetida, desconhecida ou faltando, ou valor fora disso, MUST tornar o arquivo inválido.

#### Scenario: Chave desconhecida
- **WHEN** o arquivo de estado tem uma linha `command=static`
- **THEN** o arquivo é inválido

#### Scenario: Cor sem efeito estático
- **WHEN** o arquivo de estado tem `effect=default` e `color=ffffff`
- **THEN** o arquivo é inválido

#### Scenario: Brilho fora da faixa
- **WHEN** o arquivo de estado tem `effect=static`, `color=ffffff` e `brightness=150`
- **THEN** o arquivo é inválido

### Requirement: Estado depois de cada comando
O estado gravado SHALL separar o efeito de ligado/desligado. `static` grava `effect=static` com a cor e o brilho, e `power=on`; `default` grava `effect=default` e `power=on`. `off` e `on` SHALL manter o efeito do estado anterior e gravar `power=off` ou `power=on`; o `on` sobre `effect=static` SHALL gravar `brightness=80`, o brilho que ele escreve. Sem estado anterior válido, `off` e `on` gravam `effect=none`.

#### Scenario: Estático, apagar e reacender
- **WHEN** o usuário aplica `static --color ff0000 --brightness 20`, depois `off` e depois `on`
- **THEN** o estado gravado é `effect=static`, `color=ff0000`, `brightness=80` e `power=on`

#### Scenario: Apagar sem estado anterior
- **WHEN** não há arquivo de estado e o usuário aplica `off`
- **THEN** o estado gravado é `effect=none` e `power=off`

### Requirement: Reaplicar o estado
O `halo led ram restore` SHALL aplicar, nos pentes salvos, primeiro o efeito salvo, com exatamente as transações do `static` ou do `default` correspondente (nada com `effect=none`), e depois, com `power=off`, as transações do `off`, ou, com `power=on` e `effect=none`, as do `on`. Ele MUST NOT gravar o estado.

#### Scenario: Estático salvo
- **WHEN** o estado salvo é `effect=static`, `ffffff`, brilho 80, `power=on`, nos pentes `0-0052` e `0-0053`
- **THEN** o `restore` envia as mesmas 30 escritas do `halo led ram static --color ffffff`, termina com código 0 e o arquivo de estado não é alterado

#### Scenario: Cor volta depois de apagar e reacender
- **WHEN** o usuário aplicou `static --color ff0000`, `off` e `on`, e o PC liga depois de um corte de energia
- **THEN** o `restore` envia as escritas do `static --color ff0000` com brilho 80, e os pentes ficam vermelhos

#### Scenario: Desligado com efeito salvo
- **WHEN** o estado salvo é `effect=static`, `ff0000`, `power=off`
- **THEN** o `restore` envia as escritas do `static --color ff0000` e depois as do `off`, e um `halo led ram on` seguinte mostra o vermelho

### Requirement: Sem estado salvo
Sem o arquivo de estado, o `halo led ram restore` e o `halo led ram suspend` SHALL dizer que não há estado salvo e terminar com código 0, sem ler o sysfs e sem abrir nenhum `/dev/i2c-*`.

#### Scenario: Primeiro boot com o halo
- **WHEN** o `halo led ram restore` roda e `/var/lib/halo/ram-led` não existe
- **THEN** a saída diz que não há estado salvo, nenhum `/dev/i2c-*` é aberto e o código de saída é 0

### Requirement: Estado recusado
O `restore` e o `suspend` SHALL recusar o arquivo de estado, sem ler o sysfs e sem abrir nenhum `/dev/i2c-*`, quando ele não for um arquivo comum do usuário efetivo, quando o grupo ou outros puderem gravar nele ou no diretório dele, ou quando for inválido pelo requisito "Formato do estado". Ao recusar, SHALL dizer o motivo e terminar com código 5.

#### Scenario: Arquivo gravável por outros
- **WHEN** o arquivo de estado tem modo `0666`
- **THEN** o `restore` não abre nenhum `/dev/i2c-*`, diz que o arquivo pode ser alterado por outros usuários e termina com código 5

#### Scenario: Arquivo de outro dono
- **WHEN** o arquivo de estado não pertence ao usuário que roda o `restore`
- **THEN** o `restore` não abre nenhum `/dev/i2c-*` e termina com código 5

#### Scenario: Arquivo inválido
- **WHEN** o arquivo de estado tem `version=2`
- **THEN** o `restore` não abre nenhum `/dev/i2c-*` e termina com código 5

### Requirement: Espera pelos pentes salvos
Antes de escrever, o `restore` SHALL esperar até 30 segundos que o conjunto de pentes controláveis encontrado no sysfs seja igual ao `spd` salvo, consultando de novo a cada 500 ms. Se depois disso o conjunto for diferente, ele MUST NOT escrever nada, SHALL dizer quais pentes faltam ou sobram e que um novo comando `halo led ram` grava o conjunto atual, e SHALL terminar com código 3.

#### Scenario: SPD ligado depois do início
- **WHEN** o `restore` começa antes de o `ee1004` publicar o `0-0053`, e ele aparece 2 segundos depois
- **THEN** o `restore` espera e aplica nos dois pentes

#### Scenario: Pente a mais
- **WHEN** o estado salvo tem `spd=0-0052,0-0053` e há um terceiro pente controlável em `0-0051`
- **THEN** depois de 30 segundos nada é escrito, a mensagem cita o `0-0051` e o código de saída é 3

### Requirement: Apagar ao suspender
O `halo led ram suspend` SHALL ler e validar o arquivo de estado como o `restore` e, havendo estado válido, SHALL enviar exatamente as escritas do `halo led ram off`, que são as do `setDRAMDDR5_AllOff` que o FURY CTRL envia ao suspender, nos pentes encontrados. Ele MUST NOT esperar pelos pentes e MUST NOT gravar o estado.

#### Scenario: Suspender com estado salvo
- **WHEN** o estado salvo é `static` `ffffff` e o `suspend` roda
- **THEN** os pentes recebem as escritas do `off`, e o arquivo de estado continua com `effect=static` e `power=on`

### Requirement: Unidades de sistema
O projeto SHALL ter duas unidades do systemd de sistema, que executam só `/usr/local/libexec/halo/halo`, sem chamar nada do diretório de um usuário:
- `halo-ram-restore.service`: oneshot com `WantedBy=multi-user.target`, que roda `halo led ram restore`;
- `halo-ram-sleep.service`: oneshot com `RemainAfterExit`, `StopWhenUnneeded`, `Before=sleep.target` e `WantedBy=sleep.target`, que roda `halo led ram suspend` ao subir e `halo led ram restore` ao parar.

#### Scenario: Unidades válidas
- **WHEN** `systemd-analyze verify` roda sobre as duas unidades do repositório
- **THEN** ele não acusa nenhum erro

#### Scenario: Suspender e acordar
- **WHEN** as duas unidades estão habilitadas, há estado salvo e o PC suspende e acorda
- **THEN** os pentes apagam ao entrar na suspensão e voltam ao estado salvo ao acordar

#### Scenario: Boot depois de corte de energia
- **WHEN** as unidades estão habilitadas, o estado salvo é `static` `ffffff` e o PC liga depois de um corte de energia
- **THEN** os pentes ficam brancos sem nenhum comando do usuário

### Requirement: Restore mesmo depois de um suspend que falhou
Uma falha do `halo led ram suspend` ao entrar na suspensão MUST NOT impedir que o `halo led ram restore` rode ao acordar. A falha SHALL continuar registrada no journal da `halo-ram-sleep.service`.

#### Scenario: Suspend que falha no meio
- **WHEN** o `suspend` apaga o `0x5A` e falha no `0x5B` ao entrar na suspensão
- **THEN** ao acordar o `restore` roda mesmo assim e os dois pentes voltam ao estado salvo
