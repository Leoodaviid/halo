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
- O slot atual é `D9 01` = 0/30/50/80 °C: 1000/1000/1500/2500, e `D9 02` = 2800 ×4.
- O preset Default foi a duty 255, igual ao Turbo, sem explicação. Isso não afeta a curva, mas mostra que as tabelas da interface do GCC não descrevem o firmware.

Fatos do `ucCooler.dll` (decompilado com o `ilspycmd`, fontes em `docs/protocols/castor3.md`):

- `DeviceApi.SetSpeedCurve` monta `99 E6 00 <tipo>` e os pontos `<temp> <rpm_hi> <rpm_lo>` a partir do byte 4, num buffer de 84 bytes que o `CoolerApi.Send` completa até o tamanho do report.
- `DeviceApi.SetSpeedMode` envia `99 E5 <tipo> <modo> 00`. O `Customized` é o modo `1`.
- `FanControl.OnClickApply` envia `E5` da fan, `E5` da bomba, `E6` de cada tipo em Customized e `B6`.
- `GFanCurve.Point_PointerMoved` limita os pontos da fan **e da bomba** pela faixa da fan (900 a 2500), com temperatura de 0 a 100, estritamente crescente, e RPM sem queda. O valor inicial da bomba na Customized (2800) só é gravado se o usuário não mexer no ponto.

Ainda não testado: se o `E6` vale sem o `B6`, e se o `D9` mostra uma curva enviada sem `B6`.

## Goals / Non-Goals

**Goals:**
- Montar e validar a curva numa função pura, testável byte a byte.
- Confirmar no aparelho a temperatura de referência antes de fixar os limites.
- Não duplicar o envio, o snapshot e o restore da `waterforce-speed-mode`.

**Non-Goals:**
- Curva com mais de 4 pontos ou interpolação feita no halo: o firmware recebe 4 pontos.
- Controle feito pelo software, com o halo lendo a temperatura e ajustando o RPM em laço.
- Enviar a temperatura da CPU ao aparelho (`E0`/`E1`).
- ZeroRPM e FixedRPM.
- Restaurar o slot Customized de um lado que estava num preset.

## Decisions

### 1. A temperatura de referência é confirmada antes de fechar as regras

As faixas da spec só valem se o firmware seguir a temperatura do líquido, a única que ele mede sozinho. A hipótese alternativa é a CPU: o `AorusLcdService.exe` envia dados de CPU ao aparelho (`E0`/`E1`), e no Linux ninguém os envia. Por isso, o primeiro uso no aparelho é um experimento:

```
T = temp1_input atual (°C, arredondado)
fan Customized: 0:1500, T+1:1500, T+3:2500, 100:2500        (bomba fica no modo atual)

em repouso:  ~1500 RPM  -> o E6 entrou (o slot antigo daria ~1000 a 25-30 °C)
             ~1000 RPM  -> a fan continua no slot antigo: o E6 não teve efeito
                           (repetir com B6, Decisão 5)
             ~2500 RPM  -> sinal da CPU: um valor de CPU guardado acima de T+3
sob carga:   sobe para ~2500 quando temp1_input passa de T+3 -> líquido
             não acompanha temp1_input                     -> CPU
```

Os 1500 RPM em repouso separam o "`E6` sem efeito" da hipótese da CPU. Com uma curva começando em 1000, os dois casos dariam os mesmos ~1000 RPM do slot atual. A curva respeita a spec (fan de 900 a 2500, monotônica), e a bomba não muda. O pior caso é a fan a 1500 RPM com o líquido quente, acima do Quiet do GCC. O usuário observa com o `halo telemetry waterforce --watch` e, no fim, usa o "voltar ao anterior". O RPM medido oscila uns ±200 em torno do alvo (`docs/hardware.md`), então a leitura considera também o duty.

**Se a referência for a CPU:** a change é pausada. Gravar curvas sem que ninguém envie a temperatura da CPU deixa o RPM preso num valor velho. Essa situação é pior que um preset, porque o preset tem as curvas internas do firmware. Daria para reescopar a change de duas formas, numa decisão nova com o usuário:
- (a) Enviar `E0`/`E1` periodicamente, o que exige daemon (Fase 2) e análise do `AorusLcdService`.
- (b) Restringir a Customized a curvas planas (um RPM fixo nos 4 pontos), que não dependem da referência.

Alternativa considerada: implementar tudo e conferir a temperatura de referência só no fim. Foi descartada porque o resultado muda as regras e talvez a viabilidade da change.

### 2. Faixa da bomba de 2600 a 2800

O GCC não protege a bomba: aplica a ela a faixa da fan, de 900 a 2500. Isso deixa a bomba muito abaixo de qualquer preset e, ao mesmo tempo, impede os 2800 que ele mesmo usa como valor inicial.

- **Mínimo de 2600:** o menor RPM de qualquer modo pronto da bomba (Balanced, `PumpApi.dcX240_280_360`). Com ele, o halo nunca deixa a bomba mais lenta do que a Gigabyte deixa sem curva. No aparelho, o Balanced da bomba ficou em 2620–2700 RPM.
- **Máximo de 2800:** o maior valor que se sabe que o GCC grava com o `E6`. É o valor inicial da Customized, reenviado sem mudança quando o usuário não mexe no ponto (o `D9 02` deste aparelho guarda exatamente isso). O Turbo (3150 na tabela, 3030–3299 medidos) é uma curva interna do firmware, e não se sabe se o `E6` com 3150 é aceito. Quem quer a rotação máxima usa o preset Turbo.

Alternativas consideradas:
- Copiar o GCC (900 a 2500): permitiria uma bomba lenta, e 2500 fica abaixo de todos os presets.
- Ir até 3150: seria um valor nunca enviado com o `E6`, o que é contra a regra de só enviar o que foi observado.

A faixa estreita (200 RPM) é intencional. A Customized da bomba serve para baixar o ruído até o nível do Balanced, sem descer abaixo disso.

As faixas valem só para a **gravação** de uma curva nova. A restauração de um snapshot em Customized não passa por elas, só pela checagem de sanidade da `waterforce-speed-mode` (`check_curve`: 4 pontos, temperatura crescente até 100 °C, RPM até 3200). Ela devolve ao aparelho um estado que já estava nele, e recusá-lo deixaria o usuário sem caminho de volta. Isso fica explícito na spec para ninguém "corrigir" o restore depois.

### 3. Os dois `E5`, como na `waterforce-speed-mode`

O halo envia sempre os dois `E5`, fan e depois bomba, como o GCC e como a `waterforce-speed-mode`. O lado da curva recebe `01` (Customized). O outro repete o valor **cru** lido no `DD`. Se esse valor for 3 (FixedRPM) ou maior que 7, nada é escrito e o usuário escolhe antes um preset para aquele lado.

A versão anterior desta decisão enviava só o `E5` do lado da curva, com medo de que o reenvio trocasse o FixedRPM por Balanced. A premissa estava errada: quem converte 3 em 0 é o `DeviceApi.GetMode` do GCC, depois de ler. O halo lê o byte cru, vê o 3 e recusa. Com um único caminho de envio nas duas changes, a sequência é igual à do GCC e não há uma variante só da curva para testar.

O teste da `waterforce-speed-mode` mostrou que o `E5` vale sem o `B6`. Isso não muda esta decisão: os dois `E5` continuam saindo em toda gravação.

### 4. Sequência e reaproveitamento

```
DE 00 (modelo) --> snapshot DD, D9 01, D9 02 --> E5 01 <m> 00 --> E5 02 <m> 00 --> E6 00 <tipo> ... --> B6 (sem --no-save)
                        |
                        +-- falhou: não envia nada
```

- **Curva:** um módulo novo, `waterforce/curve.rs`, com o tipo `Curve` (4 `CurvePoint` validados por tipo de velocidade) e a função pura que, a partir do modo cru do outro lado, monta um `Target` com o lado da curva em `01` e `fan_curve` ou `pump_curve` preenchido. Os bytes saem do `Target::packets` e do `set_curve_payload` existentes, sem outro montador de `E5`/`E6`. O `CurvePoint` existente é reaproveitado.
- **Envio, snapshot e restore:** a escrita passa pelo `write_to` da `speed.rs`, exposto por uma função pública nova (por exemplo `curve_to`), com a mesma ordem, o mesmo snapshot e as mesmas recusas do `set_to`. Esta change não abre o hidraw por conta própria e não duplica o snapshot.
- **CLI:** `halo speed waterforce curve <fan|pump> <temp:rpm> ×4 [--no-save] [--dry-run]`. Os códigos de saída seguem os do `set` (2, 3, 4 e 5), e a recusa de validação acontece antes de procurar o aparelho.

### 5. `--no-save` experimental, e o `B6` na curva definitiva

O `E5` vale sem o `B6`. Se o `E6` também valer, dá para testar curvas sem gravar na flash a cada tentativa. Por isso:

- O `curve` ganha um `--no-save`, como o `set`, que envia os `E5` e o `E6` sem o `B6` e avisa que o efeito não está confirmado. O snapshot é gravado do mesmo jeito.
- O experimento da Decisão 1 roda primeiro com `--no-save`. Se a fan ficar em ~1000 RPM em repouso, o `E6` não teve efeito sem o `B6`, e o experimento é repetido com o `B6`, com novo aval do usuário.
- Sem a opção, toda gravação termina com um único `B6`, como no GCC. A curva definitiva do usuário sempre leva o `B6`, porque sem ele o modo se perde ao desligar.

Alternativa considerada: o experimento sempre com `B6`. Foi descartada porque gravaria a curva de teste na flash, e o `B6` não tem o desgaste por chamada conhecido.

### 6. A curva estende a capability `waterforce-speed`

A curva não é uma capability separada. Ela é um subcomando (`halo speed waterforce curve`) do mesmo comando e usa o mesmo snapshot. A delta adiciona só os requisitos próprios da curva, com nomes distintos dos da `waterforce-speed-mode` para o archive não colidir, e modifica dois requisitos dela:
- "Comandos permitidos": o `E6` passa a valer também no subcomando `curve`.
- "Snapshot validado antes de restaurar": a restauração não aplica as faixas da gravação de curva.

Os requisitos gerais da `waterforce-speed-mode` (modelo conferido, snapshot antes de escrever, aplicação única, aparelho ausente) já cobrem qualquer escrita e não são repetidos.

## Risks / Trade-offs

- **[A referência é a CPU]** → O experimento vem primeiro e bloqueia o resto. A Decisão 1 define a pausa e as duas opções de reescopo.
- **[O `E6` sem `B6` não ter efeito]** → A leitura em repouso separa esse caso da CPU (Decisão 1), e o experimento é repetido com `B6` (Decisão 5).
- **[O firmware recusa ou corta valores do `E6`]** → Depois de gravar, ler o `D9 <tipo>` e comparar com o que foi enviado. Uma diferença é registrada em `docs/hardware.md` e interrompe o uso. Sem o `B6`, o `D9` pode não mostrar a curva nova, então a leitura vale só depois de um envio com `B6`.
- **[O slot inativo fica com a curva de teste]** → O "voltar ao anterior" só reenvia a curva de um lado que estava em Customized no snapshot. Se um `B6` gravar a curva de teste, o slot antigo pode ser recriado com o próprio `curve` (`fan 0:1000 30:1000 50:1500 80:2500`, `pump` 2800 ×4, dentro das faixas), seguido de um preset. Fica registrado no `docs/hardware.md`.
- **[Desgaste da flash pelo `B6`]** → Uma gravação por execução, nada em laço (spec, "Aplicação única"), e o `--no-save` para os testes.
- **[Faixa da bomba estreita demais para algum uso]** → Fica registrado como decisão do halo, revisável com dados do aparelho. O preset Turbo cobre a rotação máxima.

## Migration Plan

Não há estado anterior para migrar. Para reverter, o "voltar ao anterior" da `waterforce-speed-mode` volta os modos e reenvia a curva de um lado que estava em Customized. O slot de um lado que estava num preset não é restaurado (ver Risks). Também existe o caminho manual: escolher um preset, que não depende da curva Customized.

## Open Questions

- Regras extras contra uma "curva perigosa" além das faixas. Exemplos: exigir que a fan chegue a pelo menos 2000 RPM em temperaturas acima de 50 °C, ou recusar curvas cujo último ponto fique abaixo de 60 °C. Fica para depois do experimento, com dados reais da temperatura do líquido sob carga.
- O que o firmware faz acima do último ponto: mantém o RPM do último ponto ou vai ao máximo. Não muda a validação, mas deve ir para `docs/hardware.md` se aparecer no experimento.
- Se o `E6` vale sem o `B6`, se o `D9` mostra uma curva enviada sem `B6` e se ela se perde ao desligar, como o `E5`.
