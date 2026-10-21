# waterforce-speed Specification

## Purpose

Ler e escolher o modo de velocidade da fan e da bomba do AORUS Waterforce X (`1044:7a4d`) pelo hidraw, usando só os presets do firmware e os comandos confirmados no GIGABYTE Control Center, sempre com um snapshot do estado anterior e um jeito de voltar a ele.

## Requirements

### Requirement: Comandos permitidos
O comando de velocidade SHALL enviar ao Castor3 apenas `DE 00`, `DD`, `D9 01`, `D9 02`, `E5`, `B6` e, só na restauração de um estado salvo em Customized, `E6`, cada um como output report `0x99` completado com zeros até 6144 bytes (fonte: `DeviceApi` do `ucCooler.dll` do GCC 26.09.10.01, ver `docs/protocols/castor3.md`). Ele MUST NOT enviar comandos de LED, de LCD nem `C8`.

#### Scenario: Fan em Quiet com a bomba em Turbo
- **WHEN** o aparelho está com a fan em Performance e a bomba em Turbo, e o usuário pede a fan em Quiet
- **THEN** o sistema envia, nesta ordem, `99 DE 00`, `99 DD`, `99 D9 01`, `99 D9 02`, `99 E5 01 06 00`, `99 E5 02 04 00` e `99 B6`, e nada mais

### Requirement: Status somente leitura
O sistema SHALL oferecer um status que envia só `DE 00`, `DD`, `D9 01` e `D9 02`, não grava snapshot e não altera o estado do aparelho.

#### Scenario: Status
- **WHEN** o usuário pede o status do Waterforce
- **THEN** o sistema envia exatamente `99 DE 00`, `99 DD`, `99 D9 01` e `99 D9 02`, e os modos e as rotações continuam como antes

### Requirement: Apresentação do status
O status SHALL mostrar o modo da fan e da bomba pelo nome e pelo valor bruto e as curvas do `D9`, identificadas como o slot Customized guardado no aparelho. Quando o modo de um lado não é Customized, o sistema MUST indicar que aquela curva não é a ativa, e, se mostrar a curva do preset, indicar que ela é uma tabela de referência da interface do GCC.

#### Scenario: Fan em preset
- **WHEN** o `DD` responde `99 DD 05 04` e o `D9 01` traz 0 °C 1000, 30 °C 1000, 50 °C 1500 e 80 °C 2500 RPM
- **THEN** a saída mostra a fan em Performance (5), a bomba em Turbo (4) e a curva da fan como slot Customized, não ativa

#### Scenario: Fan em Customized
- **WHEN** o `r[2]` do `DD` é 1
- **THEN** a saída indica que a curva do `D9 01` é a curva ativa da fan

### Requirement: Presets aceitos
O sistema SHALL aceitar para a fan os presets Default (2), Quiet (6), Balanced (0), Performance (5) e Turbo (4), e para a bomba Balanced (0) e Turbo (4), que são os valores do `SpeedMode` do GCC. Outros modos, entre eles ZeroRPM, FixedRPM, Customized e Quiet na bomba, MUST ser recusados antes de abrir o aparelho.

#### Scenario: ZeroRPM recusado
- **WHEN** o usuário pede a fan em ZeroRPM
- **THEN** o sistema informa os presets válidos, não abre o aparelho e termina com código diferente de zero

#### Scenario: Quiet na bomba recusado
- **WHEN** o usuário pede a bomba em Quiet
- **THEN** o sistema informa que a bomba aceita só Balanced e Turbo e não abre o aparelho

#### Scenario: Nenhum lado informado
- **WHEN** o usuário pede uma mudança sem informar a fan nem a bomba
- **THEN** o sistema recusa a entrada e não abre o aparelho

### Requirement: Lado não informado preserva o modo atual
O sistema SHALL enviar sempre os dois `E5`, primeiro o da fan (`E5 01`) e depois o da bomba (`E5 02`), como o `FanControl.OnClickApply` do GCC. Para o lado que o usuário não informou, ele repete o valor bruto lido no `DD`. Se esse valor for 3 (FixedRPM) ou maior que 7, o sistema MUST NOT escrever nada e deve pedir que o usuário informe aquele lado.

#### Scenario: Só a bomba informada
- **WHEN** o `DD` responde `99 DD 05 04` e o usuário pede a bomba em Balanced
- **THEN** os `E5` são `99 E5 01 05 00` e `99 E5 02 00 00`

#### Scenario: Lado atual desconhecido
- **WHEN** o `r[3]` do `DD` é 3 e o usuário informa só a fan
- **THEN** o sistema não envia `E5` nem `B6`, informa o valor lido e pede o modo da bomba

### Requirement: Modelo conferido antes de escrever
O sistema SHALL enviar `DE 00` e só prosseguir se a resposta chegar em 2 segundos com `r[2]` igual a 0, 1 ou 2 (Waterforce X 240, 280 ou 360). Caso contrário, MUST NOT enviar `E5`, `E6` nem `B6`.

#### Scenario: Modelo não reconhecido
- **WHEN** a resposta ao `DE 00` tem `r[2]` igual a 5
- **THEN** o sistema não envia `E5`, `E6` nem `B6`, informa o valor lido e termina com código diferente de zero

### Requirement: Snapshot antes de escrever
Antes de qualquer `E5`, `E6` ou `B6`, o sistema SHALL ler `DD`, `D9 01` e `D9 02` e gravar o estado lido como o snapshot "anterior" em `$XDG_STATE_HOME/halo/` (ou `~/.local/state/halo/` sem a variável). Se ainda não existir um snapshot "original", grava o mesmo estado também como original. Se uma leitura ficar sem resposta ou a gravação falhar, MUST NOT escrever nada no aparelho.

#### Scenario: Primeira mudança
- **WHEN** não existe snapshot e o usuário aplica um preset
- **THEN** o sistema grava os snapshots anterior e original com o estado lido antes de enviar o primeiro `E5`

#### Scenario: Mudanças seguintes
- **WHEN** já existe um snapshot original e o usuário aplica outro preset
- **THEN** o sistema substitui só o snapshot anterior, e o original continua igual

#### Scenario: Diretório sem permissão de escrita
- **WHEN** o diretório de estado não pode ser criado ou gravado
- **THEN** o sistema informa o caminho, não envia `E5`, `E6` nem `B6` e termina com código diferente de zero

#### Scenario: Curva sem resposta
- **WHEN** nenhuma resposta `99 D9` ao `D9 02` chega em 2 segundos
- **THEN** o sistema não grava snapshot, não envia `E5`, `E6` nem `B6` e termina com código diferente de zero

### Requirement: Voltar ao anterior
O sistema SHALL oferecer uma restauração que aplica o snapshot anterior. Ela envia `E5 01 <modo da fan>` e `E5 02 <modo da bomba>`, depois `E6` com a curva salva de cada lado cujo modo salvo é Customized, fan antes da bomba, e um `B6` no fim. Como qualquer escrita, ela grava antes o estado atual como snapshot anterior, então duas restaurações seguidas alternam entre os dois estados.

#### Scenario: Volta de um preset
- **WHEN** o snapshot anterior tem a fan em Performance (5) e a bomba em Turbo (4)
- **THEN** o sistema envia `99 E5 01 05 00`, `99 E5 02 04 00` e `99 B6`, sem `E6`

#### Scenario: Volta para Customized
- **WHEN** o snapshot anterior tem a fan em Customized com 0 °C 1000, 30 °C 1000, 50 °C 1500 e 80 °C 2500 RPM, e a bomba em Turbo
- **THEN** depois dos dois `E5` o sistema envia `99 E6 00 01 00 03 E8 1E 03 E8 32 05 DC 50 09 C4` e então `99 B6`

#### Scenario: Sem snapshot
- **WHEN** não existe snapshot anterior
- **THEN** o sistema informa que não há estado para restaurar e não abre o aparelho

### Requirement: Voltar ao original
O sistema SHALL oferecer uma opção de restauração que aplica o snapshot original, o estado de antes da primeira escrita do halo, com a mesma sequência e as mesmas verificações da restauração do anterior. O snapshot original MUST NOT ser alterado pela restauração.

#### Scenario: Restaurar o original
- **WHEN** o usuário pede a restauração do original depois de várias mudanças
- **THEN** o aparelho volta aos modos e, nos lados em Customized, às curvas lidos antes da primeira mudança, e o snapshot original continua igual

### Requirement: Snapshot validado antes de restaurar
O sistema SHALL recusar um snapshot que não possa ler, que tenha modo 3 ou maior que 7, ou cuja curva a reenviar não tenha 4 pontos com temperatura até 100 °C, estritamente crescente, e RPM até 3200 (o `MaxSpeed` do GCC). Nesses casos, MUST NOT enviar nada ao aparelho.

#### Scenario: Snapshot corrompido
- **WHEN** o arquivo do snapshot anterior está truncado
- **THEN** o sistema informa o caminho do arquivo, não envia nada e termina com código diferente de zero

### Requirement: Aplicação única
Cada execução de mudança ou restauração SHALL enviar uma única sequência, com no máximo um `B6` no fim, e terminar. O sistema MUST NOT oferecer repetição nem reenvio periódico.

#### Scenario: Uma execução
- **WHEN** o usuário aplica um preset
- **THEN** o aparelho recebe exatamente um `B6`, e o processo termina

### Requirement: Envio sem salvar
O sistema SHALL oferecer uma opção experimental, na mudança de preset, que envia os `E5` sem o `B6` e avisa que o efeito e a persistência dessa forma não estão confirmados. O snapshot é gravado do mesmo jeito.

#### Scenario: Sem B6
- **WHEN** o usuário pede a fan em Quiet com a opção sem salvar
- **THEN** o sistema envia `99 DE 00`, `99 DD`, `99 D9 01`, `99 D9 02` e os dois `E5`, e nenhum `B6`

### Requirement: Simulação sem aparelho
O sistema SHALL oferecer, na mudança de preset, uma simulação que exige os dois lados informados e mostra em hexadecimal os `E5` e o `B6`, sem abrir nem procurar o hidraw e sem gravar snapshot.

#### Scenario: Simulação
- **WHEN** o usuário simula a fan em Quiet e a bomba em Turbo
- **THEN** a saída mostra `99 E5 01 06 00`, `99 E5 02 04 00` e `99 B6`, nenhum dispositivo é aberto e nenhum snapshot é gravado

#### Scenario: Simulação com um lado só
- **WHEN** o usuário simula só a fan
- **THEN** o sistema recusa a entrada e pede os dois lados

### Requirement: Aparelho ausente ou sem permissão
O sistema SHALL informar quando o Waterforce não for encontrado e, quando o hidraw não abrir por falta de permissão, indicar a regra udev do halo. Nos dois casos termina com código diferente de zero sem enviar nada.

#### Scenario: Sem permissão
- **WHEN** o hidraw do `1044:7a4d` existe, mas o usuário não pode abri-lo
- **THEN** o sistema informa a falta de permissão, o caminho e onde está a regra udev do halo
