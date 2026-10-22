# daemon-waterforce Specification

## Purpose

Oferecer pelo D-Bus da sessão o LED, a velocidade e a telemetria do AORUS Waterforce X (`1044:7a4d`), com as mesmas sequências, validações e proteções da CLI, para que a extensão GNOME e outros clientes não precisem falar com o hidraw.

## Requirements

### Requirement: Objeto do Waterforce
O daemon SHALL publicar o Waterforce no objeto `/io/github/leoodaviid/Halo1/waterforce`, com as interfaces `io.github.leoodaviid.Halo1.Waterforce.Led`, `io.github.leoodaviid.Halo1.Waterforce.Speed` e `io.github.leoodaviid.Halo1.Waterforce.Telemetry`. O objeto MUST existir mesmo com o aparelho ausente, e as chamadas que precisam do hidraw devolvem então o erro `NotFound`.

#### Scenario: Aparelho ausente
- **WHEN** o Waterforce não está no sysfs e um cliente chama a aplicação de LED
- **THEN** a chamada devolve `io.github.leoodaviid.Halo1.Error.NotFound` e nada é enviado

### Requirement: Mesmas regras da CLI
Cada chamada D-Bus de escrita no Waterforce SHALL valer como uma execução do comando correspondente da CLI e cumprir os requisitos das capabilities `waterforce-led` e `waterforce-speed`: os mesmos comandos permitidos, o modelo conferido antes de escrever, a aplicação única com no máximo um `B6`, e, na velocidade, o snapshot antes de escrever e as faixas de RPM. O daemon MUST NOT enviar ao Castor3 nenhum comando que a CLI não envie.

#### Scenario: Mesma sequência da CLI
- **WHEN** um cliente aplica pelo D-Bus o efeito estático com a cor `ff6600` e brilho 10
- **THEN** o daemon envia, nesta ordem, `99 DE 00`, `99 C9 01 64 0A 02 01`, `99 CD FF 66 00` e `99 B6`, e nada mais

#### Scenario: Snapshot compartilhado com a CLI
- **WHEN** um cliente grava pelo D-Bus o preset Quiet na fan e depois o usuário roda `halo speed waterforce restore`
- **THEN** a CLI restaura o estado de antes da gravação feita pelo daemon

### Requirement: Aplicar LED
A interface `Led` SHALL oferecer a aplicação de um efeito pelo nome usado na CLI (`static`, `pulse`, `flash`, `double-flash`, `gradient`, `cycle`, `wave`, `rainbow`, `color-shift`, `tri-color`, `spin`, `switch`, `lego`, `aurora` e `off`), com uma lista de 0 a 8 cores em hexadecimal `rrggbb`, brilho de 1 a 10 e velocidade de 1 a 5. Uma lista vazia SHALL usar as cores padrão do efeito. Valores inválidos MUST ser recusados com `InvalidArgument` antes de procurar o aparelho.

#### Scenario: Efeito com paleta
- **WHEN** um cliente aplica `tri-color` com as cores `ff0000`, `00ff00` e `0000ff`, brilho 10 e velocidade 5
- **THEN** o daemon envia a mesma sequência que `halo led waterforce tri-color --color ff0000 --color 00ff00 --color 0000ff`

#### Scenario: Efeito desconhecido
- **WHEN** um cliente pede o efeito `strobe`
- **THEN** a chamada devolve `InvalidArgument` e o hidraw não é aberto

### Requirement: Padrões do LED pelo D-Bus
Na aplicação de LED, brilho 0 e velocidade 0 SHALL valer o padrão do efeito, como quando a opção não é informada na CLI. No `off`, o brilho MUST ser 0, porque o efeito não aceita brilho.

#### Scenario: Desligar
- **WHEN** um cliente aplica `off` com brilho 0 e velocidade 0
- **THEN** o daemon envia a mesma sequência que `halo led waterforce off`

#### Scenario: Brilho no desligado
- **WHEN** um cliente aplica `off` com brilho 5
- **THEN** a chamada devolve `InvalidArgument` e o hidraw não é aberto

### Requirement: Último LED aplicado
Como o protocolo não tem leitura do LED, a interface `Led` SHALL expor como propriedade o último efeito, cores, brilho e velocidade que o próprio daemon aplicou com sucesso, e emitir a mudança dessa propriedade. Antes da primeira aplicação do processo, a propriedade MUST ficar vazia, e não pode ser apresentada como o estado do aparelho.

#### Scenario: Depois de aplicar
- **WHEN** a aplicação de `cycle` termina com sucesso
- **THEN** a propriedade passa a informar `cycle` com o brilho e a velocidade usados, e um sinal de mudança é emitido

#### Scenario: Aplicação que falhou
- **WHEN** a aplicação falha porque o modelo não foi reconhecido
- **THEN** a propriedade continua com o valor anterior

### Requirement: Status da velocidade
A interface `Speed` SHALL oferecer um status somente leitura com o modo da fan e da bomba, pelo nome e pelo valor bruto, e as duas curvas do slot Customized, com a mesma sequência de leitura do `halo speed waterforce status` (`DE 00`, `DD`, `D9 01`, `D9 02`). O status MUST NOT gravar snapshot.

#### Scenario: Status lido
- **WHEN** um cliente pede o status com a fan em Performance e a bomba em Turbo
- **THEN** a resposta traz a fan como `performance` (5), a bomba como `turbo` (4) e as duas curvas de 4 pontos

### Requirement: Presets pelo D-Bus
A interface `Speed` SHALL oferecer a gravação de presets com a fan e a bomba opcionais, com os mesmos nomes aceitos pela CLI. O lado não informado repete o modo lido no aparelho, como na CLI. Um pedido sem nenhum lado, ou com um preset não aceito, MUST ser recusado com `InvalidArgument` antes de procurar o aparelho.

#### Scenario: Só a fan
- **WHEN** um cliente grava só a fan em `quiet`, com a bomba em Turbo no aparelho
- **THEN** o daemon envia a mesma sequência que `halo speed waterforce set --fan quiet`

#### Scenario: Nenhum lado
- **WHEN** um cliente pede a gravação sem fan e sem bomba
- **THEN** a chamada devolve `InvalidArgument` e o hidraw não é aberto

### Requirement: Curva plana pelo D-Bus
A interface `Speed` SHALL oferecer a gravação da curva Customized plana de um lado (`fan` ou `pump`) com um RPM, como `halo speed waterforce curve`, com as mesmas faixas (fan de 900 a 2500, bomba de 2600 a 2800).

#### Scenario: Curva da fan
- **WHEN** um cliente grava a curva plana da fan em 1500 RPM
- **THEN** o daemon envia a mesma sequência que `halo speed waterforce curve fan 1500`

#### Scenario: RPM fora da faixa
- **WHEN** um cliente pede a curva da bomba em 2000 RPM
- **THEN** a chamada devolve `InvalidArgument` e o hidraw não é aberto

### Requirement: Restauração pelo D-Bus
A interface `Speed` SHALL oferecer a restauração do snapshot anterior e do original, com a mesma sequência e as mesmas verificações de `halo speed waterforce restore` e `restore --original`. Um snapshot ausente ou inválido MUST ser recusado com `Storage` sem abrir o aparelho.

#### Scenario: Sem snapshot
- **WHEN** um cliente pede a restauração do anterior e o arquivo `waterforce-speed.previous` não existe
- **THEN** a chamada devolve `Storage` e o hidraw não é aberto

### Requirement: Opções experimentais fora do D-Bus
A interface D-Bus MUST NOT oferecer o envio sem `B6` nem a simulação. Essas opções continuam só na CLI.

#### Scenario: Escrita pelo D-Bus
- **WHEN** um cliente grava um preset pelo D-Bus
- **THEN** a sequência termina com `99 B6`

### Requirement: Telemetria como propriedades
A interface `Telemetry` SHALL expor como propriedades se a telemetria está disponível, o RPM e o duty bruto (0 a 255) da fan e da bomba, a temperatura do líquido como lida e se essa temperatura é válida, pelas regras da capability `waterforce-telemetry`. Os valores SHALL vir do hwmon, nas tarefas periódicas do daemon, sem abrir o hidraw.

#### Scenario: Líquido fora da faixa
- **WHEN** o hwmon informa `temp1_input` igual a 224000
- **THEN** a propriedade da temperatura informa 224 °C e a de validade informa falso, e nenhum comando é enviado

#### Scenario: Driver não carregado
- **WHEN** não há hwmon do Waterforce
- **THEN** a propriedade de disponibilidade informa falso e as outras mantêm o último valor lido

### Requirement: Sinal só quando a telemetria muda
O daemon SHALL emitir a mudança das propriedades de telemetria só quando algum valor mudar desde a leitura anterior, e MUST NOT emitir sinais quando todos os valores ficarem iguais.

#### Scenario: Valores iguais
- **WHEN** duas leituras seguidas do hwmon trazem os mesmos valores
- **THEN** nenhum sinal de mudança é emitido na segunda
