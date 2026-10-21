# Design

## Context

O que já existe na `main`, com a `waterforce-speed-mode` aplicada e arquivada (PR #6):

- O `probe.rs` monta o output report de 6144 bytes (`packet()`), filtra as respostas pelo comando (`wait_response`) e decodifica o `D9` em `CurvePoint` (temperatura e RPM, big-endian).
- O `speed.rs` da `waterforce-speed-mode` tem:
  - `Target { fan, pump, fan_curve, pump_curve }`, cujo `packets(save)` já monta `E5` fan, `E5` bomba, `E6` de cada lado com curva e, com `save`, um `B6`, na ordem do `FanControl.OnClickApply`;
  - `set_curve_payload` (o `E6`) e `check_curve` (a checagem de sanidade da restauração);
  - `SnapshotStore` (snapshots `previous` e `original`) e `restore_to`/`restore`;
  - `write_to`, que lê `DE 00`, `DD`, `D9 01` e `D9 02`, grava o snapshot e só então envia. Hoje ele é privado, e `set_to` e `restore_to` são os únicos caminhos públicos.

Confirmado no aparelho pela `waterforce-speed-mode` (`docs/hardware.md`, X 240, firmware 1.6):

- O `E5` vale na hora **sem o `B6`**, mas se perde ao desligar o PC com corte de energia. O `B6` persiste o modo.
- O `D9` lê o **slot Customized guardado**, e não a curva ativa. Ele não mudou depois de nenhum preset, e o RPM seguiu o preset.
- O slot de fábrica era `D9 01` = 0/30/50/80 °C: 1000/1000/1500/2500, e `D9 02` = 2800 ×4.
- O preset Default foi a duty 255, igual ao Turbo, sem explicação.

Confirmado no aparelho pelo experimento desta change (2026-10-20, `docs/hardware.md`), sempre com `--no-save`:

- O firmware aceita o `E6` e segue a curva. A curva plana de 1500 RPM levou a fan a duty 153 em ~3 s, e o duty é RPM/2500 × 255.
- O `E6` vale **sem o `B6`**, e o `D9` mostra a curva enviada sem `B6`.
- A referência **não é o líquido**. Com o líquido a 25–26 °C, a curva `0:1500, 27:1500, 29:2500, 100:2500` ficou no ponto de 2500 (duty 255) por quase um minuto. A referência estava acima de 29 °C.
- Os duties dos presets da speed-mode caem no ponto de 50 °C das tabelas do GCC (Quiet 143 ≈ 1400, Balanced 184 ≈ 1800, Performance 224 ≈ 2200). Isso sugere uma referência fixa perto de 50 °C quando ninguém envia a temperatura da CPU. É uma hipótese.
- O `restore --original` voltou os modos, e o `D9 01` ficou com a última curva gravada (`1500 ×4`).

Fatos do código do GCC (decompilado com o `ilspycmd`, fontes em `docs/protocols/castor3.md`):

- `DeviceApi.SetSpeedCurve` monta `99 E6 00 <tipo>` e os pontos `<temp> <rpm_hi> <rpm_lo>` a partir do byte 4, num buffer de 84 bytes que o `CoolerApi.Send` completa até o tamanho do report.
- `DeviceApi.SetSpeedMode` envia `99 E5 <tipo> <modo> 00`. O `Customized` é o modo `1`.
- `FanControl.OnClickApply` envia `E5` da fan, `E5` da bomba, `E6` de cada tipo em Customized e `B6`.
- `GFanCurve.Point_PointerMoved` limita os pontos da fan **e da bomba** pela faixa da fan (900 a 2500), com temperatura de 0 a 100, estritamente crescente, e RPM sem queda.
- A tela da fan (`FanControl`) mostra a **temperatura da CPU** (`tb_CPUTemp`) em todos os modelos menos o 7A51. O valor vem do `DeviceApi.GetCPUTemperature`, que lê o `CpuInfo.ini` do `AorusLcdService`. A tela não mostra a temperatura do líquido.
- Para o `7A4D`, o `AorusLcdService` (`CoolerDataApi.GetCoolerData`) espera 20 s, envia `99 E1` (nome da CPU) uma vez e depois envia `99 E0` em laço, sem `Sleep`, no ritmo das leituras do `MakeCpuData`. O `E0` leva a temperatura da CPU em `r[3]`. O byte da temperatura do líquido (`r[9]`) nunca é preenchido nesse ramo e vai como 0.

## Goals / Non-Goals

**Goals:**
- Gravar a Customized com efeito previsível sem ninguém enviar a temperatura da CPU.
- Montar e validar a curva numa função pura, testável byte a byte.
- Não duplicar o envio, o snapshot e o restore da `waterforce-speed-mode`.

**Non-Goals:**
- Curvas com degraus, que dependem da temperatura de referência.
- Enviar a temperatura da CPU ao aparelho (`E0`/`E1`). Exige um processo em laço (daemon, Fase 2) e fica para uma change futura.
- Controle feito pelo software, com o halo lendo a temperatura e ajustando o RPM em laço.
- ZeroRPM e FixedRPM.
- Restaurar o slot Customized de um lado que estava num preset.

## Decisions

### 1. A referência é a CPU, e a Customized fica plana

A Decisão 1 original exigia confirmar a referência antes de fechar as regras. O experimento e o código do GCC apontam para a CPU: o firmware não seguiu o líquido, a tela da fan do GCC mostra a temperatura da CPU ao lado da curva, e só o `AorusLcdService` a envia (`E0`). No Linux, o valor que o firmware usa sem o `E0` é desconhecido e parece fixo. Uma curva com degraus ficaria presa num ponto que o usuário não escolheu.

Das duas opções de reescopo, esta change fica com a **(b): curvas planas**. Um RPM fixo nos 4 pontos não depende da referência, e o aparelho seguiu a curva plana de 1500 RPM.

Alternativa considerada: a **(a)**, enviar `E0`/`E1` periodicamente. Ela exige daemon (Fase 2), uma fonte de temperatura da CPU no Linux (hwmon `k10temp`/`coretemp`) e a definição do que fazer quando o envio para. Como isso muda a capability de "uma escrita e sai" para um serviço, fica para outra change. As curvas planas continuam válidas depois dela.

### 2. Faixa da bomba de 2600 a 2800

O GCC não protege a bomba: aplica a ela a faixa da fan, de 900 a 2500. Isso deixa a bomba muito abaixo de qualquer preset e, ao mesmo tempo, impede os 2800 que ele mesmo usa como valor inicial.

- **Mínimo de 2600:** o menor RPM de qualquer modo pronto da bomba (Balanced, `PumpApi.dcX240_280_360`). Com ele, o halo nunca deixa a bomba mais lenta do que a Gigabyte deixa sem curva. No aparelho, o Balanced da bomba ficou em 2620–2700 RPM.
- **Máximo de 2800:** o maior valor que se sabe que o GCC grava com o `E6`. É o valor inicial da Customized, reenviado sem mudança quando o usuário não mexe no ponto (o `D9 02` deste aparelho guarda exatamente isso). O Turbo (3150 na tabela, 3030–3299 medidos) é uma curva interna do firmware, e não se sabe se o `E6` com 3150 é aceito. Quem quer a rotação máxima usa o preset Turbo.

Alternativas consideradas:
- Copiar o GCC (900 a 2500): permitiria uma bomba lenta, e 2500 fica abaixo de todos os presets.
- Ir até 3150: seria um valor nunca enviado com o `E6`, o que é contra a regra de só enviar o que foi observado.

A faixa estreita (200 RPM) é intencional. A Customized da bomba serve para baixar o ruído até o nível do Balanced, sem descer abaixo disso.

As regras valem só para a **gravação** de uma curva nova. A restauração de um snapshot em Customized não passa por elas, nem exige curva plana, só pela checagem de sanidade da `waterforce-speed-mode` (`check_curve`: 4 pontos, temperatura crescente até 100 °C, RPM até 3200). Ela devolve ao aparelho um estado que já estava nele, como o slot de fábrica da fan, que tem degraus. Recusá-lo deixaria o usuário sem caminho de volta. Isso fica explícito na spec para ninguém "corrigir" o restore depois.

### 3. Os dois `E5`, como na `waterforce-speed-mode`

O halo envia sempre os dois `E5`, fan e depois bomba, como o GCC e como a `waterforce-speed-mode`. O lado da curva recebe `01` (Customized). O outro repete o valor **cru** lido no `DD`. Se esse valor for 3 (FixedRPM) ou maior que 7, nada é escrito e o usuário escolhe antes um preset para aquele lado.

A versão anterior desta decisão enviava só o `E5` do lado da curva, com medo de que o reenvio trocasse o FixedRPM por Balanced. A premissa estava errada: quem converte 3 em 0 é o `DeviceApi.GetMode` do GCC, depois de ler. O halo lê o byte cru, vê o 3 e recusa. Com um único caminho de envio nas duas changes, a sequência é igual à do GCC e não há uma variante só da curva para testar.

### 4. Sequência e reaproveitamento

```
DE 00 (modelo) --> snapshot DD, D9 01, D9 02 --> E5 01 <m> 00 --> E5 02 <m> 00 --> E6 00 <tipo> ... --> B6 (sem --no-save)
                        |
                        +-- falhou: não envia nada
```

- **Curva:** o módulo `waterforce/curve.rs`, com o tipo `Curve` (lado e 4 `CurvePoint`, montados por `Curve::flat(side, rpm)`) e a função pura que, a partir do modo cru do outro lado, monta um `Target` com o lado da curva em `01` e `fan_curve` ou `pump_curve` preenchido. Os bytes saem do `Target::packets` e do `set_curve_payload` existentes, sem outro montador de `E5`/`E6`. O `CurvePoint` existente é reaproveitado.
- **Envio, snapshot e restore:** a escrita passa pelo `write_to` da `speed.rs`, exposto pela função pública `curve_to`, com a mesma ordem, o mesmo snapshot e as mesmas recusas do `set_to`. Esta change não abre o hidraw por conta própria e não duplica o snapshot.
- **CLI:** `halo speed waterforce curve <fan|pump> <rpm> [--no-save] [--dry-run]`. Os códigos de saída seguem os do `set` (2, 3, 4 e 5), e a recusa de validação acontece antes de procurar o aparelho.

### 5. `--no-save` experimental, e o `B6` na curva definitiva

O `E5` e o `E6` valem na hora sem o `B6` (confirmado). O `E5` sem `B6` se perde ao desligar, e o `E6` provavelmente também, mas isso não foi testado. Por isso:

- O `curve` tem um `--no-save`, como o `set`, que envia os `E5` e o `E6` sem o `B6` e avisa que a curva vale na hora, mas talvez não continue depois de desligar. O snapshot é gravado do mesmo jeito.
- Os testes no aparelho usam o `--no-save`, para não gravar na flash a cada tentativa.
- Sem a opção, toda gravação termina com um único `B6`, como no GCC. A curva definitiva do usuário sempre leva o `B6`.

### 6. Interface plana: um RPM, temperaturas fixas

O usuário informa um RPM, e o halo monta os pontos em 0, 30, 50 e 80 °C, as temperaturas do slot de fábrica. Elas cumprem por construção as regras do `GFanCurve` (de 0 a 100, estritamente crescentes), e o RPM igual nos 4 pontos cumpre o "sem queda". Assim, a validação do usuário é só a faixa de RPM do lado.

Alternativa considerada: manter a entrada de 4 pontos `temp:rpm` e recusar RPM diferentes. Foi descartada porque obriga a digitar temperaturas que não têm efeito e mostra uma forma de curva que o halo não aceita.

### 7. A curva estende a capability `waterforce-speed`

A curva não é uma capability separada. Ela é um subcomando (`halo speed waterforce curve`) do mesmo comando e usa o mesmo snapshot. A delta adiciona só os requisitos próprios da curva, com nomes distintos dos da `waterforce-speed-mode` para o archive não colidir, e modifica dois requisitos dela:
- "Comandos permitidos": o `E6` passa a valer também no subcomando `curve`, e os dados de sistema (`E0`, `E1`) ficam explicitamente proibidos.
- "Snapshot validado antes de restaurar": a restauração não aplica as regras da gravação de curva.

Os requisitos gerais da `waterforce-speed-mode` (modelo conferido, snapshot antes de escrever, aplicação única, aparelho ausente) já cobrem qualquer escrita e não são repetidos.

## Risks / Trade-offs

- **[Referência desconhecida]** → A curva plana não depende dela. Se uma change futura enviar o `E0`, a curva plana continua igual.
- **[Fan plana baixa sob carga]** → Uma curva plana não sobe com a temperatura. A fan a 900 RPM fica abaixo do Quiet, que no aparelho vai a ~1400 RPM (duty 143). Decidido na task 5.2: o mínimo continua 900, e o README avisa do risco (ver Open Questions).
- **[O slot inativo fica com a curva de teste]** → O "voltar ao anterior" só reenvia a curva de um lado que estava em Customized no snapshot. Depois do experimento, o `D9 01` ficou com `1500 ×4`. O slot de fábrica pode ser recriado. Ele tem degraus e não passa pela gravação plana, então o caminho é o restore de um snapshot em Customized com essa curva, ou o GCC no Windows. Fica registrado no `docs/hardware.md`.
- **[Sensor do líquido travado]** → Na task 5.1, ~33 s depois da curva da bomba, o byte 13 do `DA` ficou em 224/240 °C até um corte de energia. Ocorreu uma vez, e a causa não é conhecida. O firmware não reagiu ao valor. Registrado no `docs/hardware.md`, com a captura de referência.
- **[Desgaste da flash pelo `B6`]** → Uma gravação por execução, nada em laço (spec, "Aplicação única"), e o `--no-save` para os testes.
- **[Faixa da bomba estreita demais para algum uso]** → Fica registrado como decisão do halo, revisável com dados do aparelho. O preset Turbo cobre a rotação máxima.

## Migration Plan

Não há estado anterior para migrar. Para reverter, o "voltar ao anterior" da `waterforce-speed-mode` volta os modos e reenvia a curva de um lado que estava em Customized. O slot de um lado que estava num preset não é restaurado (ver Risks). Também existe o caminho manual: escolher um preset, que não depende da curva Customized.

## Open Questions

- ~~RPM mínimo da fan na curva plana.~~ Decidido na task 5.2 (2026-10-21): mantém 900, o limite do GCC (`DeviceApi.GetFanSpeedLimit`). O Quiet medido (1221–1617 RPM) varia demais para servir de piso, e subir o mínimo seria uma regra do halo sem base no firmware. A curva plana é uma escolha explícita de RPM fixo, e o README avisa que ela não sobe com a carga.
- Se o `E6` enviado sem o `B6` continua depois de desligar o PC com corte de energia, como acontece com o `E5`.
- Qual temperatura o firmware usa quando ninguém envia o `E0`: um padrão perto de 50 °C ou outro valor. Não muda a curva plana e só importa para a change futura que enviar o `E0`.
