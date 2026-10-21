# waterforce-speed Specification

## Purpose

Ler e escolher o modo de velocidade da fan e da bomba do AORUS Waterforce X (`1044:7a4d`) pelo hidraw, usando só os presets do firmware e os comandos confirmados no GIGABYTE Control Center, sempre com um snapshot do estado anterior e um jeito de voltar a ele.

## Requirements

### Requirement: Comandos permitidos
O comando de velocidade SHALL enviar ao Castor3 apenas `DE 00`, `DD`, `D9 01`, `D9 02`, `E5`, `B6` e `E6`, este só na gravação de uma curva pelo subcomando `curve` e na restauração de um estado salvo em Customized, cada um como output report `0x99` completado com zeros até 6144 bytes (fonte: `DeviceApi` do `ucCooler.dll` do GCC 26.09.10.01, ver `docs/protocols/castor3.md`). Ele MUST NOT enviar comandos de LED, de LCD, de dados de sistema (`E0`, `E1`) nem `C8`, nem os modos 3 (FixedRPM) e 7 (ZeroRPM) como alvo novo.

#### Scenario: Fan em Quiet com a bomba em Turbo
- **WHEN** o aparelho está com a fan em Performance e a bomba em Turbo, e o usuário pede a fan em Quiet
- **THEN** o sistema envia, nesta ordem, `99 DE 00`, `99 DD`, `99 D9 01`, `99 D9 02`, `99 E5 01 06 00`, `99 E5 02 04 00` e `99 B6`, e nada mais

#### Scenario: Curva na fan
- **WHEN** o aparelho está com a fan em Performance e a bomba em Turbo, e o usuário grava uma curva plana válida na fan
- **THEN** o sistema envia, nesta ordem, `99 DE 00`, `99 DD`, `99 D9 01`, `99 D9 02`, `99 E5 01 01 00`, `99 E5 02 04 00`, `99 E6 00 01 …` e `99 B6`, e nada mais

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
O sistema SHALL recusar um snapshot que não possa ler, que tenha modo 3 ou maior que 7, ou cuja curva a reenviar não tenha 4 pontos com temperatura até 100 °C, estritamente crescente, e RPM até 3200 (o `MaxSpeed` do GCC). Nesses casos, MUST NOT enviar nada ao aparelho. A restauração MUST NOT aplicar as regras da gravação de curva (curva plana, fan de 900 a 2500, bomba de 2600 a 2800): ela devolve ao aparelho um estado que já estava nele, como o slot de fábrica da fan, que tem degraus, ou o da bomba em 2800.

#### Scenario: Snapshot corrompido
- **WHEN** o arquivo do snapshot anterior está truncado
- **THEN** o sistema informa o caminho do arquivo, não envia nada e termina com código diferente de zero

#### Scenario: Curva salva fora das regras da gravação
- **WHEN** o snapshot anterior tem a fan em Customized com 0 °C 800, 30 °C 800, 50 °C 1500 e 80 °C 2500 RPM, válida pelas regras da restauração, mas com degraus e abaixo do mínimo da gravação de curva
- **THEN** o sistema restaura essa curva com o `E6` sem recusá-la

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

### Requirement: Gravação da curva Customized
O subcomando `halo speed waterforce curve` SHALL gravar a curva Customized de um lado (fan ou bomba) com a sequência do botão Aplicar do GCC (`FanControl.OnClickApply`): `E5 01 <modo da fan> 00`, `E5 02 <modo da bomba> 00`, `E6 00 <tipo>` com os pontos e `B6`. O lado da curva recebe o modo `01` (Customized, `DeviceApi.SetSpeedMode`). O `<tipo>` é `01` para a fan e `02` para a bomba, e só o `E6` do lado da curva é enviado.

#### Scenario: Curva da fan
- **WHEN** o `DD` responde `99 DD 05 04` e o usuário grava na fan a curva plana de 1500 RPM
- **THEN** depois do `DE 00` e do snapshot, o sistema envia `99 E5 01 01 00`, `99 E5 02 04 00`, `99 E6 00 01 00 05 DC 1E 05 DC 32 05 DC 50 05 DC` e `99 B6`, e nada mais

#### Scenario: Curva da bomba
- **WHEN** o `DD` responde `99 DD 05 04` e o usuário grava na bomba a curva plana de 2800 RPM
- **THEN** o sistema envia `99 E5 01 05 00`, `99 E5 02 01 00`, `99 E6 00 02 00 0A F0 1E 0A F0 32 0A F0 50 0A F0` e `99 B6`, sem nenhum `E6` com o tipo `01`

### Requirement: Curva plana
O sistema SHALL receber um único RPM por gravação e montar a curva com 4 pontos em 0, 30, 50 e 80 °C, todos com esse RPM, o que cumpre as regras do `GFanCurve`. O sistema MUST NOT gravar uma curva com RPM diferentes entre os pontos, porque a referência do firmware não é a do líquido (design, Decisão 1).

#### Scenario: Pontos da curva plana
- **WHEN** o usuário grava na fan a curva plana de 1500 RPM
- **THEN** a curva enviada é 0 °C 1500, 30 °C 1500, 50 °C 1500 e 80 °C 1500 RPM

#### Scenario: RPM inválido
- **WHEN** o usuário informa `mil` como RPM
- **THEN** o sistema recusa a entrada antes de abrir o aparelho

### Requirement: Outro lado preservado na curva
Na gravação de uma curva, o `E5` do lado sem curva SHALL repetir o valor bruto lido no `DD` (o halo lê o byte cru; quem converte 3 em 0 é o `DeviceApi.GetMode` do GCC). Se esse valor for 3 (FixedRPM) ou maior que 7, o sistema MUST NOT escrever nada e deve pedir que o usuário escolha antes um preset para aquele lado.

#### Scenario: Outro lado em FixedRPM
- **WHEN** o `r[3]` do `DD` é 3 e o usuário grava uma curva na fan
- **THEN** o sistema não envia `E5`, `E6` nem `B6`, informa o valor lido e pede um preset para a bomba

### Requirement: Layout do E6
O pacote de curva SHALL ser `99 E6 00 <tipo>` seguido dos 4 pontos a partir do byte 4, cada um como `<temperatura °C> <rpm alto> <rpm baixo>` (RPM em 16 bits big-endian), como em `DeviceApi.SetSpeedCurve`.

#### Scenario: RPM em big-endian
- **WHEN** a curva plana da fan é de 2500 RPM
- **THEN** os três bytes do último ponto são `50 09 C4`

### Requirement: Faixa de RPM da fan na curva
O RPM da curva da fan SHALL ficar entre 900 e 2500, os limites do GCC para o PID `0x7A4D` (`DeviceApi.GetFanSpeedLimit`). Um valor fora da faixa MUST ser recusado antes de abrir o aparelho.

#### Scenario: Fan abaixo do mínimo
- **WHEN** o usuário pede a fan a 500 RPM
- **THEN** o sistema recusa a curva e informa a faixa de 900 a 2500 RPM

### Requirement: Faixa de RPM da bomba na curva
O RPM da curva da bomba SHALL ficar entre 2600 e 2800. O mínimo é o menor valor dos modos prontos da bomba no GCC (Balanced, `PumpApi.dcX240_280_360`). O máximo é o maior valor que o GCC grava com o `E6`: o valor inicial da Customized e o slot de fábrica lido com o `D9 02`. Um valor fora da faixa MUST ser recusado antes de abrir o aparelho.

#### Scenario: Bomba lenta
- **WHEN** o usuário pede a bomba a 1500 RPM
- **THEN** o sistema recusa a curva, informa a faixa de 2600 a 2800 RPM e explica que o mínimo é o do modo Balanced

#### Scenario: Bomba acima do máximo
- **WHEN** o usuário pede a bomba a 3150 RPM
- **THEN** o sistema recusa a curva e sugere o modo pronto Turbo para rotação máxima

### Requirement: Curva sem salvar
O subcomando de curva SHALL oferecer uma opção experimental que envia os dois `E5` e o `E6` sem o `B6` e avisa que a curva vale na hora, mas que não se sabe se ela continua depois de desligar. O snapshot é gravado do mesmo jeito, e a validação é a mesma.

#### Scenario: Curva da fan sem B6
- **WHEN** o `DD` responde `99 DD 05 04` e o usuário grava na fan a curva plana de 1500 RPM com a opção sem salvar
- **THEN** depois do `DE 00` e do snapshot, o sistema envia `99 E5 01 01 00`, `99 E5 02 04 00` e `99 E6 00 01 00 05 DC 1E 05 DC 32 05 DC 50 05 DC`, e nenhum `B6`

### Requirement: Simulação da curva
O subcomando de curva SHALL oferecer uma simulação que valida o RPM e mostra em hexadecimal o `E5` do lado da curva, o `E6` e, sem a opção sem salvar, o `B6`, sem abrir nem procurar o hidraw e sem gravar snapshot. Como o modo do outro lado só é conhecido lendo o `DD`, a simulação indica que o `E5` daquele lado repetirá o modo lido na gravação.

#### Scenario: Simulação
- **WHEN** o usuário simula a curva plana da fan de 1500 RPM
- **THEN** a saída mostra `99 E5 01 01 00`, o aviso sobre o `E5 02`, `99 E6 00 01 00 05 DC 1E 05 DC 32 05 DC 50 05 DC` e `99 B6`, e nenhum dispositivo é aberto

#### Scenario: Simulação de curva inválida
- **WHEN** o usuário simula a curva da bomba de 1500 RPM
- **THEN** o sistema recusa a curva como faria sem a simulação

### Requirement: Aparelho ausente ou sem permissão
O sistema SHALL informar quando o Waterforce não for encontrado e, quando o hidraw não abrir por falta de permissão, indicar a regra udev do halo. Nos dois casos termina com código diferente de zero sem enviar nada.

#### Scenario: Sem permissão
- **WHEN** o hidraw do `1044:7a4d` existe, mas o usuário não pode abri-lo
- **THEN** o sistema informa a falta de permissão, o caminho e onde está a regra udev do halo
