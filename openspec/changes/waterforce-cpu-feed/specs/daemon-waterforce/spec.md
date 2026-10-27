# Spec Delta

## MODIFIED Requirements

### Requirement: Mesmas regras da CLI
Cada chamada D-Bus de escrita no Waterforce SHALL valer como uma execução do comando correspondente da CLI e cumprir os requisitos das capabilities `waterforce-led` e `waterforce-speed`: os mesmos comandos permitidos, o modelo conferido antes de escrever, a aplicação única com no máximo um `B6`, e, na velocidade, o snapshot antes de escrever e as faixas de RPM. O fluxo periódico de CPU e a proteção são exceções: SHALL usar DE/E1/E0 sem B6, conforme os requisitos desta capability; SetFanCurve admite quatro pontos validados. Essas exceções não alteram as sequências legadas.

#### Scenario: Mesma sequência da CLI
- **WHEN** um cliente aplica pelo D-Bus o efeito estático com a cor `ff6600` e brilho 10
- **THEN** o daemon envia, nesta ordem, `99 DE 00`, `99 C9 01 64 0A 02 01`, `99 CD FF 66 00` e `99 B6`, e nada mais na transação de LED

#### Scenario: Snapshot compartilhado com a CLI
- **WHEN** um cliente grava pelo D-Bus o preset Quiet na fan e depois o usuário roda `halo speed waterforce restore`
- **THEN** a CLI restaura o estado de antes da gravação feita pelo daemon

### Requirement: Último LED aplicado
Como o protocolo não tem leitura do LED, a interface `Led` SHALL expor como propriedade o último efeito, cores, brilho e velocidade que o próprio daemon aplicou com sucesso, e emitir a mudança dessa propriedade. O daemon SHALL guardar esse valor no diretório de estado do usuário a cada aplicação bem-sucedida e SHALL voltar a publicá-lo ao iniciar, sem enviar comandos por causa do LastApplied. A propriedade MUST ficar vazia só quando não houver valor guardado válido, e não pode ser apresentada como o estado do aparelho.

#### Scenario: Depois de aplicar
- **WHEN** a aplicação de `cycle` termina com sucesso
- **THEN** a propriedade passa a informar `cycle` com o brilho e a velocidade usados, e um sinal de mudança é emitido

#### Scenario: Aplicação que falhou
- **WHEN** a aplicação falha porque o modelo não foi reconhecido
- **THEN** a propriedade continua com o valor anterior, e o valor guardado também

#### Scenario: Depois de reiniciar
- **WHEN** o daemon aplicou o estático vermelho com brilho 10, foi encerrado e iniciou de novo
- **THEN** a propriedade informa o estático vermelho com brilho 10 desde o início, e nenhum comando é enviado ao aparelho por causa disso

#### Scenario: Sem valor guardado
- **WHEN** o daemon inicia sem nenhum valor guardado, porque o halo nunca aplicou um LED pelo daemon
- **THEN** a propriedade fica vazia

#### Scenario: Valor guardado inválido
- **WHEN** o valor guardado não pode ser lido ou tem um efeito, uma cor, um brilho ou uma velocidade inválidos
- **THEN** o daemon inicia normalmente com a propriedade vazia, registra um aviso no log e não envia nada ao aparelho por causa do LastApplied

#### Scenario: Falha ao guardar
- **WHEN** uma aplicação dá certo, mas o valor não pode ser gravado no diretório de estado
- **THEN** a aplicação continua sendo respondida com sucesso, a propriedade muda, e o daemon registra um aviso no log

#### Scenario: Reboot com corte de energia
- **WHEN** o daemon aplicou o estático branco pelo painel e o PC reinicia depois de desligar com corte de energia
- **THEN** o primeiro Get do LastApplied informa o estático branco guardado, sem abrir hidraw nem enviar comandos ao aparelho por causa do LastApplied

## ADDED Requirements

### Requirement: Alimentação da CPU
O daemon SHALL enviar E1 antes do primeiro E0 de cada sessão lógica validada do X 240 (startup, reconexão, mudança de geração ou resume) e enviar E0 a cada 2 s sob supervisão ativa da unidade. Os reports SHALL ter 6144 bytes, sem B6 ou snapshots; nenhum comando de LCD, E5 ou E6 pertence a esse fluxo.

#### Scenario: Inicialização supervisionada
- **WHEN** o nome D-Bus foi adquirido e o aparelho é X 240 validado
- **THEN** o daemon envia E1 e E0 de 80 °C, mantém proteção por 8 s e depois envia Tctl atual a cada 2 s, mesmo constante; reabrir o fd entre ticks não reinicia E1/DE sem mudança da sessão lógica

#### Scenario: Sem supervisão
- **WHEN** o daemon roda manualmente sem watchdog efetivo da unidade
- **THEN** ele publica a CPU pelo sysfs, não inicia E0 frio e não aceita SetFanCurve

#### Scenario: Modelo sem feed validado
- **WHEN** o DE da sessão lógica informa modelo 1, 2 ou fora da tabela
- **THEN** o daemon não envia E1/E0 até a próxima sessão lógica, CpuFeedActive fica false, SetFanCurve devolve Device e os controles legados seguem como antes

#### Scenario: Reconexão
- **WHEN** o aparelho retorna com outro hidraw
- **THEN** o daemon descobre pelo VID:PID e usage page, valida o modelo e reinicia E1 e a janela segura sem reaplicar curva ou perfil

### Requirement: Fonte e codificação da CPU
A temperatura SHALL vir do canal rotulado Tctl no hwmon k10temp, com validade de 0 a 125000 m°C; no E0, SHALL ser arredondada para cima e limitada a 80 °C. Nome/fabricante vêm de cpuinfo, threads e núcleos da topologia online. GHz, uso, W, campo VRAM/VRM e líquido SHALL ser zero nesta change.

#### Scenario: Índice do hwmon muda
- **WHEN** k10temp está em outro hwmon e Tctl em outro canal
- **THEN** a descoberta usa nome e label e envia a temperatura desse canal, sem assumir temp1_input

#### Scenario: Falha do sensor
- **WHEN** Tctl desaparece, é ambíguo, ilegível ou fora da faixa
- **THEN** o próximo envio usa 80 °C e CpuValid fica false, sem usar líquido ou enviar 0 °C por falha

#### Scenario: Tctl acima do ponto final
- **WHEN** Tctl válido é 85000 m°C
- **THEN** CpuMilliCelsius informa 85000, CpuValid é true e E0 usa 80 °C

### Requirement: Temperatura CPU no D-Bus
Telemetry SHALL expor CpuMilliCelsius (i32, m°C), CpuValid e CpuFeedActive (booleanos) separadamente do líquido. CpuMilliCelsius começa em 0 e mantém o último valor em falha, com CpuValid=false. CpuFeedActive SHALL exigir supervisão, janela inicial de 8 s concluída e E0 aceito há no máximo 4 s; MUST NOT significar confirmação de RPM.

#### Scenario: CPU presente e cooler ausente
- **WHEN** Tctl pode ser lido, mas não existe Waterforce
- **THEN** CpuValid é true, CpuMilliCelsius mostra Tctl e CpuFeedActive é false, sem mudar a semântica de Available e CoolantValid

#### Scenario: Proteção não é leitura de sensor
- **WHEN** o daemon envia proteção por Tctl inválido
- **THEN** CpuValid continua false e CpuMilliCelsius não é substituído por 80000

#### Scenario: Sinais só de mudanças
- **WHEN** duas leituras deixam as três propriedades iguais
- **THEN** nenhum PropertiesChanged CPU é emitido na segunda leitura

#### Scenario: Feed atrasado
- **WHEN** o último E0 aceito tem mais de 4 s
- **THEN** CpuFeedActive fica false sem invalidar uma leitura atual de Tctl

### Requirement: Temperatura de proteção
A proteção SHALL enviar E0 de 80 °C em qualquer modo da fan, inclusive presets; MUST NOT restaurar E0 anterior ou alterar modos, curvas ou flash com E5/E6/B6. O envio SHALL ser descrito como referência alta, sem prometer duty máximo para qualquer curva ou modo.

#### Scenario: Performance depois de falha
- **WHEN** a fan está em Performance e o daemon precisa proteger
- **THEN** ele envia E0 de 80 °C, sem dispensá-lo porque DD não é Customized

#### Scenario: Curva plana baixa
- **WHEN** a curva ativa é plana em 1500 RPM
- **THEN** a proteção envia 80 °C e não declara garantia de duty 100 ou muda a curva

### Requirement: Recuperação independente de D-Bus
O comando halod --waterforce-cpu-safe SHALL funcionar sem daemon, nome D-Bus, Tctl, perfis ou snapshots. SHALL descobrir e validar o X 240, enviar E1/E0 de 80 °C sob flock, liberar o nó e esperar 8 s para observabilidade. A execução SHALL terminar em até 18 s e informar falha de transporte com saída não zero.

#### Scenario: SIGKILL deixou último E0 frio
- **WHEN** o principal morreu e o gerenciador executa o comando de proteção
- **THEN** o comando opera diretamente no hidraw, não ativa outro daemon e envia temperatura alta antes do restart

#### Scenario: Ausente ou ocupado
- **WHEN** não existe aparelho ou seu flock não é obtido em 5 s
- **THEN** ausência termina com sucesso e log de ausência; Busy termina com falha explícita, sem burlar o lock

#### Scenario: Modelo sem feed no comando de proteção
- **WHEN** o DE informa modelo 1 ou 2
- **THEN** o comando termina com sucesso e log de modelo sem feed, sem E1/E0 nem espera de 8 s; modelo fora da tabela termina com falha e diagnóstico

#### Scenario: Idempotência
- **WHEN** SIGTERM e ExecStopPost executam proteção sucessivamente
- **THEN** ambos repetem E0 alto sem B6 nem alteração de snapshots

### Requirement: Curva de quatro pontos da fan
Speed SHALL oferecer SetFanCurve(points: a(yq)), com quatro pares temperatura/RPM: temperaturas crescentes em 0..80, RPM não decrescentes em 900..2500 e último ponto 80:2500. SHALL exigir CPU válida, feed ativo, supervisão e janela inicial concluída; manter snapshot e a transação E5/E6/B6 da fan, sem alterar a API plana ou a CLI.

#### Scenario: Curva supervisionada
- **WHEN** o feed está saudável e o cliente pede 0:2200/30:2200/50:2350/80:2500
- **THEN** o daemon relê Tctl e supervisão ao executar, envia E0 atual e grava o snapshot e a curva com um B6, preservando o modo da bomba

#### Scenario: Curva sem ponto seguro
- **WHEN** o último ponto não é 80:2500, faltam pontos ou há RPM decrescente
- **THEN** a chamada devolve InvalidArgument antes de procurar o aparelho

#### Scenario: Feed indisponível
- **WHEN** CpuValid ou CpuFeedActive é false ou não há supervisão efetiva
- **THEN** SetFanCurve devolve Device antes de abrir hidraw; se a condição mudar na fila, recusa ao executar sem gravar curva

#### Scenario: Uso legado
- **WHEN** um cliente chama SetFlatCurve ou a CLI grava um RPM único
- **THEN** a validação, os snapshots e os pacotes legados permanecem compatíveis

#### Scenario: Cliente durante startup ou restart
- **WHEN** o cliente chama SetFanCurve antes de completar a janela segura de 8 s
- **THEN** recebe Device sem E0 frio ou escrita de curva; CpuFeedActive permanece false até completar a janela e o primeiro ciclo após ela
