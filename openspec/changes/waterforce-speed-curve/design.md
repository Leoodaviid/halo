# Design

## Context

O que já existe ou vem da `waterforce-speed-mode`, aplicada antes:

- O `probe.rs` já monta o output report de 6144 bytes (`packet()`), filtra as respostas pelo comando (`wait_response`) e decodifica o `D9` em `CurvePoint` (temperatura e RPM, big-endian).
- A `waterforce-speed-mode` traz o envio do `E5`, o snapshot do estado (`DD`, `D9 01`, `D9 02`), o "voltar ao anterior" e a resposta sobre o `E5` valer sem o `B6`.

Fatos levantados na exploração (fontes em `docs/protocols/castor3.md` e no `ucCooler.dll`, decompilado com o `ilspycmd`):

- `DeviceApi.SetSpeedCurve` monta `99 E6 00 <tipo>` e os pontos `<temp> <rpm_hi> <rpm_lo>` a partir do byte 4, num buffer de 84 bytes que o `CoolerApi.Send` completa até o tamanho do report.
- `DeviceApi.SetSpeedMode` envia `99 E5 <tipo> <modo> 00`. O `Customized` é o modo `1`.
- `FanControl.OnClickApply` envia `E5` da fan, `E5` da bomba, `E6` de cada tipo em Customized e `B6`.
- `DeviceApi.GetSpeedCurve` só lê o `D9` quando o modo é Customized. Para os outros modos, devolve as tabelas da interface. Com a fan em Performance e o líquido a 26 °C, o kernel mostrou 2273 RPM, enquanto o `D9 01` daria cerca de 1000. Conclusão: o `D9` lê o **slot Customized guardado**, não a curva ativa. É uma inferência forte, ainda não confirmada por escrita.
- `GFanCurve.Point_PointerMoved` limita os pontos da fan **e da bomba** pela faixa da fan (900 a 2500), com temperatura de 0 a 100, estritamente crescente, e RPM sem queda. O valor inicial da bomba na Customized (2800) só é gravado se o usuário não mexer no ponto.

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

## Decisions

### 1. A temperatura de referência é confirmada antes de fechar as regras

As faixas da spec só valem se o firmware seguir a temperatura do líquido, a única que ele mede sozinho. A hipótese alternativa é a CPU: o `AorusLcdService.exe` envia dados de CPU ao aparelho (`E0`/`E1`), e no Linux ninguém os envia. Por isso, o primeiro uso no aparelho é um experimento:

```
T = temp1_input atual (°C, arredondado)
fan Customized: 0:1000, T+1:1000, T+3:2500, 100:2500        (bomba fica no modo atual)

esperado (líquido):   a fan fica em ~1000 RPM em repouso e sobe para ~2500
                      quando temp1_input passa de T+3 sob carga
alternativa (CPU):    a fan ignora temp1_input (fica presa em 1000 ou em 2500
                      desde o início, conforme o valor de CPU guardado)
```

A curva respeita a spec (fan de 900 a 2500, monotônica), e a bomba não muda. O pior caso é a fan a 1000 RPM com o líquido quente, acima do Quiet do GCC (900). O usuário observa com o `halo telemetry waterforce --watch` e, no fim, usa o "voltar ao anterior".

**Se a referência for a CPU:** a change é pausada. Gravar curvas sem que ninguém envie a temperatura da CPU deixa o RPM preso num valor velho. Essa situação é pior que um preset, porque o preset tem as curvas internas do firmware. Daria para reescopar a change de duas formas, numa decisão nova com o usuário:
- (a) Enviar `E0`/`E1` periodicamente, o que exige daemon (Fase 2) e análise do `AorusLcdService`.
- (b) Restringir a Customized a curvas planas (um RPM fixo nos 4 pontos), que não dependem da referência.

Alternativa considerada: implementar tudo e conferir a temperatura de referência só no fim. Foi descartada porque o resultado muda as regras e talvez a viabilidade da change.

### 2. Faixa da bomba de 2600 a 2800

O GCC não protege a bomba: aplica a ela a faixa da fan, de 900 a 2500. Isso deixa a bomba muito abaixo de qualquer preset e, ao mesmo tempo, impede os 2800 que ele mesmo usa como valor inicial.

- **Mínimo de 2600:** o menor RPM de qualquer modo pronto da bomba (Balanced, `PumpApi.dcX240_280_360`). Com ele, o halo nunca deixa a bomba mais lenta do que a Gigabyte deixa sem curva.
- **Máximo de 2800:** o maior valor que se sabe que o GCC grava com o `E6`. É o valor inicial da Customized, reenviado sem mudança quando o usuário não mexe no ponto (o `D9 02` deste aparelho guarda exatamente isso). O Turbo (3150) é uma curva interna do firmware, e não se sabe se o `E6` com 3150 é aceito. Quem quer a rotação máxima usa o preset Turbo.

Alternativas consideradas:
- Copiar o GCC (900 a 2500): permitiria uma bomba lenta, e 2500 fica abaixo de todos os presets.
- Ir até 3150: seria um valor nunca enviado com o `E6`, o que é contra a regra de só enviar o que foi observado.

A faixa estreita (200 RPM) é intencional. A Customized da bomba serve para baixar o ruído até o nível do Balanced, sem descer abaixo disso.

As faixas valem só para a **gravação** de uma curva nova. A restauração de um snapshot em Customized não passa por elas, só pela checagem de sanidade da `waterforce-speed-mode` (4 pontos, temperatura crescente até 100 °C, RPM até 3200). Ela devolve ao aparelho um estado que já estava nele, e recusá-lo deixaria o usuário sem caminho de volta. Isso fica explícito na spec para ninguém "corrigir" o restore depois.

### 3. Os dois `E5`, como na `waterforce-speed-mode`

O halo envia sempre os dois `E5`, fan e depois bomba, como o GCC e como a `waterforce-speed-mode`. O lado da curva recebe `01` (Customized). O outro repete o valor **cru** lido no `DD`. Se esse valor for 3 (FixedRPM) ou maior que 7, nada é escrito e o usuário escolhe antes um preset para aquele lado.

A versão anterior desta decisão enviava só o `E5` do lado da curva, com medo de que o reenvio trocasse o FixedRPM por Balanced. A premissa estava errada: quem converte 3 em 0 é o `DeviceApi.GetMode` do GCC, depois de ler. O halo lê o byte cru, vê o 3 e recusa. Com um único caminho de envio nas duas changes, a sequência é igual à do GCC e não há uma variante só da curva para testar.

Se o experimento do `B6` na `waterforce-speed-mode` mostrar que um `E5` sozinho basta, as duas changes mudam juntas, numa revisão nova.

### 6. A curva estende a capability `waterforce-speed`

A curva não é uma capability separada. Ela é um subcomando (`halo speed waterforce curve`) do mesmo comando e usa o mesmo snapshot. A delta adiciona só os requisitos próprios da curva, com nomes distintos dos da `waterforce-speed-mode` para o archive não colidir, e modifica dois requisitos dela:
- "Comandos permitidos": o `E6` passa a valer também no subcomando `curve`.
- "Snapshot validado antes de restaurar": a restauração não aplica as faixas da gravação de curva.

Os requisitos gerais da `waterforce-speed-mode` (modelo conferido, snapshot antes de escrever, aplicação única, aparelho ausente) já cobrem qualquer escrita e não são repetidos. Por isso, esta change só pode ser arquivada depois da `waterforce-speed-mode`.

### 4. Sequência e reaproveitamento

```
DE 00 (modelo) --> snapshot DD, D9 01, D9 02 (speed-mode) --> E5 01 <m> 00 --> E5 02 <m> 00 --> E6 00 <tipo> ... --> B6
                        |
                        +-- falhou: não envia nada
```

- **Curva:** um módulo novo, `waterforce/curve.rs`, com o tipo `Curve` (4 `CurvePoint` validados por tipo de velocidade) e a função pura que gera o `E6`. O `CurvePoint` existente é reaproveitado, sem criar outro.
- **Envio, snapshot e restore:** são chamados nas funções da `waterforce-speed-mode`. Esta change não abre o hidraw por conta própria.
- **CLI:** `halo speed waterforce curve <fan|pump> <temp:rpm> ×4 [--dry-run]`. Os códigos de saída seguem os do comando de LED (modelo desconhecido, aparelho ausente, sem permissão), e a recusa de validação acontece antes de procurar o aparelho.

### 5. O `B6` segue a resposta da `waterforce-speed-mode`

- Se o `E5`/`E6` só valerem com o `B6`, toda gravação termina com um único `B6`, como no GCC.
- Se valerem sem o `B6`, o experimento da Decisão 1 roda sem o `B6`, sem gravar na flash, e a curva definitiva grava com o `B6`.

A spec exige um `B6` por execução. Um modo sem `B6` entraria como requisito novo, só se a resposta permitir.

## Risks / Trade-offs

- **[A referência é a CPU]** → O experimento vem primeiro e bloqueia o resto. A Decisão 1 define a pausa e as duas opções de reescopo.
- **[O firmware recusa ou corta valores do `E6`]** → Depois de gravar, ler o `D9 <tipo>` e comparar com o que foi enviado. Uma diferença é registrada em `docs/hardware.md` e interrompe o uso.
- **[O `D9` não é o slot Customized]** → O experimento confirma: depois do `E6`, o `D9` tem que devolver a curva enviada, e o RPM tem que seguir a curva.
- **[Desgaste da flash pelo `B6`]** → Uma gravação por execução e nada em laço (spec, "Aplicação única").
- **[Faixa da bomba estreita demais para algum uso]** → Fica registrado como decisão do halo, revisável com dados do aparelho. O preset Turbo cobre a rotação máxima.
- **[Dependência de uma change em paralelo]** → Se a API da `waterforce-speed-mode` mudar, ajustar só as chamadas do grupo 2. As regras e o pacote `E6` não dependem dela.

## Migration Plan

Não há estado anterior para migrar. Para reverter, o "voltar ao anterior" da `waterforce-speed-mode` restaura os modos e as duas curvas do snapshot. Se o snapshot se perder, existe o caminho manual: escolher um preset, que já não depende da curva Customized.

## Open Questions

- Regras extras contra uma "curva perigosa" além das faixas. Exemplos: exigir que a fan chegue a pelo menos 2000 RPM em temperaturas acima de 50 °C, ou recusar curvas cujo último ponto fique abaixo de 60 °C. Fica para depois do experimento, com dados reais da temperatura do líquido sob carga.
- O que o firmware faz acima do último ponto: mantém o RPM do último ponto ou vai ao máximo. Não muda a validação, mas deve ir para `docs/hardware.md` se aparecer no experimento.
