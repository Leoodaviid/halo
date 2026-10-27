# Spec Delta

## MODIFIED Requirements

### Requirement: Curva plana
A CLI SHALL receber um único RPM por gravação e montar a curva com 4 pontos em 0, 30, 50 e 80 °C, todos com esse RPM, o que cumpre as regras do `GFanCurve`. A CLI MUST NOT aceitar pontos novos nesta change. O daemon pode gravar quatro pontos da fan pelo contrato SetFanCurve de daemon-waterforce, pois E0 como referência foi confirmado; restaurações históricas mantêm suas regras.

#### Scenario: Pontos da curva plana
- **WHEN** o usuário grava na fan a curva plana de 1500 RPM
- **THEN** a curva enviada é 0 °C 1500, 30 °C 1500, 50 °C 1500 e 80 °C 1500 RPM

#### Scenario: RPM inválido
- **WHEN** o usuário informa `mil` como RPM
- **THEN** o sistema recusa a entrada antes de abrir o aparelho

### Requirement: Comandos permitidos
O comando de velocidade da CLI SHALL enviar ao Castor3 apenas `DE 00`, `DD`, `D9 01`, `D9 02`, `E5`, `B6` e `E6`, este só na gravação de uma curva pelo subcomando `curve` e na restauração de um estado salvo em Customized, cada um como output report `0x99` completado com zeros até 6144 bytes (fonte: `DeviceApi` do `ucCooler.dll` do GCC 26.09.10.01, ver `docs/protocols/castor3.md`). Ele MUST NOT enviar comandos de LED, de LCD, de dados de sistema (`E0`, `E1`) nem `C8`, nem os modos 3 (FixedRPM) e 7 (ZeroRPM) como alvo novo.

#### Scenario: Fan em Quiet com a bomba em Turbo
- **WHEN** o aparelho está com a fan em Performance e a bomba em Turbo, e o usuário pede a fan em Quiet
- **THEN** o sistema envia, nesta ordem, `99 DE 00`, `99 DD`, `99 D9 01`, `99 D9 02`, `99 E5 01 06 00`, `99 E5 02 04 00` e `99 B6`, e nada mais

#### Scenario: Curva na fan
- **WHEN** o aparelho está com a fan em Performance e a bomba em Turbo, e o usuário grava uma curva plana válida na fan
- **THEN** o sistema envia, nesta ordem, `99 DE 00`, `99 DD`, `99 D9 01`, `99 D9 02`, `99 E5 01 01 00`, `99 E5 02 04 00`, `99 E6 00 01 …` e `99 B6`, e nada mais

## ADDED Requirements

### Requirement: Degraus protegidos no daemon
A gravação nova de degraus da fan pelo daemon SHALL manter modelo validado, snapshot antes de E5/E6/B6, modo da bomba preservado e um B6 no fim, conforme SetFanCurve. E0 atual anterior à configuração SHALL ser tratado como dado de CPU e não como parte do snapshot. Restaurações e curva plana legadas MUST preservar compatibilidade.

#### Scenario: Novo ponto máximo
- **WHEN** o cliente pede uma curva nova cuja última temperatura excede 80 °C
- **THEN** a gravação é recusada, pois o E0 seguro escolhido não garante atingir seu último ponto

#### Scenario: Restaurar snapshot histórico
- **WHEN** um snapshot Customized válido pelas regras existentes tem quatro pontos até 100 °C
- **THEN** sua restauração não ganha a validação restrita das curvas novas, e não é apresentada como garantia de duty máximo com E0 de 80 °C
