# Spec Delta

## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Comandos permitidos
O comando de velocidade SHALL enviar ao Castor3 apenas `DE 00`, `DD`, `D9 01`, `D9 02`, `E5`, `B6` e `E6`, este só na gravação de uma curva pelo subcomando `curve` e na restauração de um estado salvo em Customized, cada um como output report `0x99` completado com zeros até 6144 bytes (fonte: `DeviceApi` do `ucCooler.dll` do GCC 26.09.10.01, ver `docs/protocols/castor3.md`). Ele MUST NOT enviar comandos de LED, de LCD, de dados de sistema (`E0`, `E1`) nem `C8`, nem os modos 3 (FixedRPM) e 7 (ZeroRPM) como alvo novo.

#### Scenario: Fan em Quiet com a bomba em Turbo
- **WHEN** o aparelho está com a fan em Performance e a bomba em Turbo, e o usuário pede a fan em Quiet
- **THEN** o sistema envia, nesta ordem, `99 DE 00`, `99 DD`, `99 D9 01`, `99 D9 02`, `99 E5 01 06 00`, `99 E5 02 04 00` e `99 B6`, e nada mais

#### Scenario: Curva na fan
- **WHEN** o aparelho está com a fan em Performance e a bomba em Turbo, e o usuário grava uma curva plana válida na fan
- **THEN** o sistema envia, nesta ordem, `99 DE 00`, `99 DD`, `99 D9 01`, `99 D9 02`, `99 E5 01 01 00`, `99 E5 02 04 00`, `99 E6 00 01 …` e `99 B6`, e nada mais

### Requirement: Snapshot validado antes de restaurar
O sistema SHALL recusar um snapshot que não possa ler, que tenha modo 3 ou maior que 7, ou cuja curva a reenviar não tenha 4 pontos com temperatura até 100 °C, estritamente crescente, e RPM até 3200 (o `MaxSpeed` do GCC). Nesses casos, MUST NOT enviar nada ao aparelho. A restauração MUST NOT aplicar as regras da gravação de curva (curva plana, fan de 900 a 2500, bomba de 2600 a 2800): ela devolve ao aparelho um estado que já estava nele, como o slot de fábrica da fan, que tem degraus, ou o da bomba em 2800.

#### Scenario: Snapshot corrompido
- **WHEN** o arquivo do snapshot anterior está truncado
- **THEN** o sistema informa o caminho do arquivo, não envia nada e termina com código diferente de zero

#### Scenario: Curva salva fora das regras da gravação
- **WHEN** o snapshot anterior tem a fan em Customized com 0 °C 800, 30 °C 800, 50 °C 1500 e 80 °C 2500 RPM, válida pelas regras da restauração, mas com degraus e abaixo do mínimo da gravação de curva
- **THEN** o sistema restaura essa curva com o `E6` sem recusá-la
