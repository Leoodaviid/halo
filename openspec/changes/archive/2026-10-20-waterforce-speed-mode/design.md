# Design

## Context

A motivação está no `proposal.md`, e os requisitos em `specs/waterforce-speed/spec.md`. Pontos do código e do aparelho que definem a abordagem:

- `crates/halo-core/src/waterforce/probe.rs` já tem `packet` (report `0x99` com zeros até 6144 bytes), `wait_response` (filtra pelo comando e descarta os `DA` do kernel), `ReadQuery` com `Model`, `SpeedModes`, `FanCurve` e `PumpCurve`, `SpeedMode` e `CurvePoint`. `led.rs` mostra o padrão de escrita: `apply_to` com os traits `InputReports`/`OutputReports` (testável sem aparelho), `DE 00` conferido antes e `ApplyError` com as mensagens de permissão.
- Leitura de 2026-10-20 com o probe: `DD` = `05 04` (fan em Performance, bomba em Turbo), `D9 01` = 1000/1000/1500/2500 RPM em 0/30/50/80 °C, `D9 02` = 2800 ×4. Telemetria ao vivo pelo hwmon: líquido a 26 °C, fan a 2273 RPM, bomba a 3120 RPM. Se o `D9 01` fosse a curva ativa, a fan estaria perto de 1000 RPM. A bomba bate com a tabela Turbo do GCC (3150) e não com 2800. Isso é coerente com `DeviceApi.GetSpeedCurve`, que só chama o `D9` quando o modo é `Customized`: o `D9` é o slot Customized. Isso ainda é **inferido**, e a change o confirma no aparelho (tasks).
- `halo-core` só depende de `libc`, e o projeto não usa serde.

## Fontes dos bytes

Todos do `ucCooler.dll` do GCC 26.09.10.01 (ilspy, `extracted/ilspy-uccooler/`), já em `docs/protocols/castor3.md`:

| Pacote | Fonte |
|---|---|
| `99 DE 00` → `r[2]` modelo | `DeviceApi.GetDeviceModelName` |
| `99 DD` → `r[2]` fan, `r[3]` bomba | `DeviceApi.GetMode` |
| `99 D9 <tipo>` → pontos em `r[4+3i]`, `r[5+3i]<<8 \| r[6+3i]` | `DeviceApi.GetSpeedCurve` (privado) |
| `99 E5 <tipo> <modo> 00` | `DeviceApi.SetSpeedMode` |
| `99 E6 00 <tipo>` + 4 × `<°C> <rpm_hi> <rpm_lo>` a partir do byte 4 | `DeviceApi.SetSpeedCurve` |
| `99 B6` | `DeviceApi.Save` |
| Ordem `E5` fan, `E5` bomba, `E6` fan, `E6` bomba, `B6` | `FanControl.OnClickApply` |
| Valores de modo (Balanced 0 … ZeroRPM 7) | enum `ucCooler.Models.SpeedMode`; tags dos botões em `views/fan/fancontrol.baml` |
| Limites 900–2500 da fan e `MaxSpeed` 3200 | `DeviceApi.GetFanSpeedLimit` e `DeviceApi.MaxSpeed` |

`<tipo>` é o `SpeedType`: 1 = fan, 2 = bomba. O `E6` só sai na restauração de um estado que já estava no aparelho.

## Goals / Non-Goals

**Goals:**
- Um módulo `waterforce::speed` com funções puras, testáveis sem aparelho, que montam os pacotes e validam o snapshot, e uma camada fina de I/O como a do `led`.
- Um snapshot simples de ler à mão, sem dependências novas.

**Non-Goals:**
- Editar curvas (fica para a `waterforce-speed-curve`).
- Ler a telemetria pelo hidraw. O `DA` continua com o kernel, e a validação usa `halo telemetry waterforce --watch`.
- Daemon, D-Bus ou reaplicar o modo no login.

## Decisions

### 1. Comando `halo speed waterforce {status,set,restore}`

O padrão do CLI é `halo <ação> <aparelho>`. `speed` tem três subcomandos porque `status`, `set` e `restore` têm riscos diferentes e opções diferentes. Alternativa: colocar a leitura no `probe` (já lê `DD`/`D9`). Descartada porque o `status` precisa explicar o slot Customized e as tabelas do GCC, e o probe é uma visão bruta de tudo. O probe continua igual.

Os nomes dos modos no CLI ficam em minúsculas: `default`, `quiet`, `balanced`, `performance`, `turbo`. Fan e bomba usam listas separadas de valores possíveis no clap. Assim, `--pump quiet` é recusado antes de abrir o aparelho.

### 2. Sempre os dois `E5`

O GCC sempre manda o `E5` da fan e o da bomba, e nunca só um. O halo faz igual, e o lado não informado reaproveita o valor bruto lido no `DD`. Mandar um `E5` só seria um caminho que o GCC nunca usa. Não sabemos como o firmware reage a isso, e o custo de copiar o GCC é só um pacote a mais. O valor bruto 3 (FixedRPM) não é repetido: o GCC o lê como Balanced, e mandar 3 de volta seria um caminho que ele nunca percorre.

### 3. Dois snapshots, "anterior" e "original"

Arquivos:
- `$XDG_STATE_HOME/halo/waterforce-speed.previous`: substituído antes de cada escrita.
- `$XDG_STATE_HOME/halo/waterforce-speed.original`: gravado uma vez só, na primeira escrita, e nunca mais tocado pelo halo. Para "zerar" o original, basta apagar o arquivo à mão.

Alternativas:
- **Só o último snapshot**: depois de duas mudanças, o estado de fábrica do usuário (o que o GCC deixou) se perde, e esse é justamente o que mais importa recuperar.
- **Histórico em anel (N entradas)**: dá "desfazer" em vários passos, mas exige política de tamanho e de navegação, e depois de N mudanças o original também some.

Dois arquivos cobrem os dois usos reais: desfazer a última mudança e voltar ao que estava antes do halo. Como o `restore` também grava o atual como anterior antes de escrever, dois `restore` seguidos alternam entre os dois estados, como um desfazer/refazer.

Gravação atômica: escrever num arquivo temporário no mesmo diretório e fazer `rename`. O snapshot precisa estar no disco **antes** do primeiro `E5`: se a gravação falhar, nada é enviado.

### 4. Formato do snapshot: texto `chave=valor`

```
# halo: snapshot de velocidade do Waterforce
version=1
taken_unix=1792540800
model=0
fan_mode=5
pump_mode=4
fan_curve=0:1000,30:1000,50:1500,80:2500
pump_curve=0:2800,30:2800,50:2800,80:2800
```

Sem serde, sem dependência nova, e dá para ler e conferir com `cat`. Ele guarda os valores decodificados, e não os bytes brutos, porque a restauração reconstrói os pacotes pelas mesmas funções do `set`. Assim, os mesmos testes valem para os dois. `version` permite mudar o formato depois. O parser é estrito: chave desconhecida, chave faltando ou valor fora da faixa fazem o arquivo ser recusado.

### 5. O `E6` só para restaurar Customized

Quando o modo salvo é Customized, mandar só o `E5 … 01` funcionaria enquanto o slot não mudar. Mas o slot pode ter mudado entre o snapshot e a restauração: pelo GCC no Windows ou, no futuro, pela `waterforce-speed-curve`. Reenviar a curva salva garante que "voltar" volta mesmo. A curva vem do próprio aparelho, então os limites de segurança da edição de curvas não se aplicam. Mesmo assim, ela passa por uma checagem de sanidade (4 pontos, temperatura ≤ 100 e crescente, RPM ≤ 3200) para o halo não reenviar um arquivo corrompido. O pacote tem 84 bytes no GCC e é completado com zeros até 6144 bytes, como os outros (`CoolerApi.Send`).

### 6. Ordem da escrita e um único `B6`

`DE 00` (modelo) → `DD`, `D9 01`, `D9 02` (snapshot; resposta obrigatória para os três) → gravar o snapshot → `E5` fan → `E5` bomba → `E6` quando houver → `B6`. Entre os pacotes de escrita, 1 ms, o mesmo intervalo do `led`. As respostas aos `E5`/`E6`/`B6` não são esperadas, como no GCC (`CoolerApi.Send`). O `B6` sai uma vez por execução: ele provavelmente grava na flash, e nada no halo o repete.

### 7. `--no-save` como ferramenta de descoberta

Hoje não se sabe se o `E5` vale sem o `B6`. Se valer, a `waterforce-speed-curve` pode testar curvas sem gravar na flash a cada tentativa. A opção fica no `set`, com um aviso na saída. Ela não entra no `restore`, para a restauração ser sempre completa. Se o teste mostrar que o `B6` é obrigatório, a opção pode sair numa change seguinte: o aviso já diz que ela é experimental.

### 8. `status` sem snapshot e com rótulos honestos

O `status` mostra a curva do `D9` como "slot Customized (ativa)" ou "slot Customized (não ativa: modo X)". Para os presets, mostra a tabela do GCC (`FanApi.dcX240`, `dcX280_360` e `PumpApi.dcX240_280_360`, escolhida pelo modelo do `DE`) com o rótulo "referência da interface do GCC". As tabelas ficam como constantes no `halo-core`, copiadas de `docs/protocols/castor3.md`. Para o 280/360, os valores do `FanApi.dcX280_360` precisam ser extraídos do ilspy na implementação, porque o doc só traz os do 240.

## Risks / Trade-offs

- [O `B6` grava na flash a cada `set`/`restore`] → uma escrita por execução, sem laço. O uso esperado é manual e raro. O `--no-save` mede se dá para evitar.
- [O firmware aceitar o `E5`, mas interpretar diferente do GCC] → o snapshot é gravado antes, o `restore` existe e cada preset é conferido no aparelho com `halo telemetry waterforce --watch` antes da change ser concluída.
- [O snapshot fica velho se o GCC mudar o modo no Windows] → o `restore` aplica o que está salvo, e o `status` mostra o estado real antes. O arquivo tem `taken_unix` para o usuário ver a idade.
- [O lado não informado é repetido com o valor lido; se o `DD` mentir, o halo propaga] → o mesmo valor também vai para o snapshot, então o `restore` volta ao mesmo ponto. O 3 e os valores desconhecidos são recusados.
- [Restaurar um ZeroRPM (7) salvo] → só acontece se o aparelho já estava assim antes. Recusar deixaria o usuário sem caminho de volta. O halo não oferece ZeroRPM como alvo novo.

## Migration Plan

Nada para migrar: são comandos e arquivos de estado novos. Rollback: `halo speed waterforce restore --original`, ou o GCC no Windows. O estado de fábrica conhecido é fan Quiet e bomba Turbo (`FanControl.OnClickDefault` para o `0x7A4D`).

## Open Questions

- Se o `E5` sem `B6` muda o RPM e se o modo continua depois de desligar e ligar de novo. A resposta vem do experimento das tasks e não muda a spec, porque o `--no-save` já é experimental.
- Os valores de RPM que o firmware usa em cada preset (Performance ficou ~270 RPM acima da tabela do GCC). Serão registrados no `docs/hardware.md` como observação, sem mudar o comportamento.
