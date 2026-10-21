# Design

## Context

A telemetria (`crates/halo-core/src/waterforce/telemetry.rs`) lê o hwmon `waterforce` e devolve um `Telemetry` com `coolant_millicelsius: i32`. O `format_telemetry` do `halo-cli` só imprime esse valor. Só a CLI usa o campo hoje. O daemon da Fase 2 vai ser o próximo consumidor.

O valor vem do driver `gigabyte_waterforce`, que faz `temp = data[0x0D] × 1000` sem sinal e sem validação (`waterforce_raw_event`, decompilado com o REA a partir do `gigabyte_waterforce.ko` do kernel 7.0.0-38, ver `docs/hardware.md`). O byte é o `0x0D` da resposta ao `99 DA`. A faixa possível no hwmon vai de 0 a 255000 m°C. Esta change não usa nenhum byte novo de protocolo: só interpreta o valor que o hwmon já entrega.

Valores reais disponíveis: `224000` e `240000` com o aparelho travado (bytes `e0` e `f0`), e `29000` e `31000` com o aparelho normal.

## Goals / Non-Goals

**Goals:**
- Um único ponto no `halo-core` decide se a temperatura do líquido vale, para a CLI e o daemon futuro usarem a mesma regra.
- Manter o valor bruto acessível, para registrar a ocorrência.

**Non-Goals:**
- Detectar outros sintomas do travamento (os bytes 10 a 12 do `DA` não mudaram com o erro).
- Ler o hidraw para comparar com o hwmon.
- Validar RPM ou duty, que continuaram corretos durante o erro.

## Decisions

### Limite de 100 °C, e não um padrão de bits

A temperatura é inválida acima de `100000` m°C.

- **Por que 100 °C:** é o ponto de ebulição da água. Nenhum líquido de watercooler chega lá com o aparelho funcionando. O limite pega os dois valores observados com uma margem grande e não depende de entender o erro.
- **Alternativa: padrão `0xE0`/`0xF0`, ou nibble baixo 0 e ≥ `0x80`.** Pega exatamente o que foi visto, mas se apoia numa hipótese (valor-sentinela de um sensor desconectado) tirada de uma ocorrência só. Um erro com outro byte, `0xC3` por exemplo, passaria como 195 °C.
- **Alternativa: tratar o byte como com sinal (≥ `0x80` é negativo, logo inválido).** Dá na prática o mesmo corte (128 °C), mas a justificativa é a hipótese sobre o firmware, e não um fato físico.
- **Alternativa: um limite mais baixo, como 60 ou 80 °C.** Um líquido muito quente numa falha real da bomba é justamente o caso em que o usuário precisa ver o número. Marcar como inválido um valor alto e possível esconderia um problema real. O limite separa o impossível do improvável, e não o normal do preocupante.

O limite fica numa constante pública do módulo, documentada com a ocorrência.

### API: a classificação ao lado do valor bruto

O `Telemetry` mantém o `coolant_millicelsius` como está e ganha um método que devolve um enum:

```text
enum Coolant { Valid(m°C), Implausible(m°C) }
```

- **Por quê:** o `Telemetry` continua sendo o retrato fiel do hwmon (os testes atuais e o `PartialEq` não mudam), e a regra fica num lugar só. Quem quiser só o número bruto continua tendo.
- **Alternativa: trocar o campo por um `Option` ou pelo enum.** Força todo consumidor a lidar com a validade, mas perde a leitura bruta ou muda a forma do struct sem ganho real, já que a CLI é o único consumidor.
- **Alternativa: devolver um erro em `read()`.** Derrubaria o RPM e o duty, que estão corretos. Isso contraria a spec, que manda mostrar fan e bomba.

### Saída da CLI

- Linha do líquido com o valor válido: igual a hoje (`Líquido:  29,0 °C`).
- Linha do líquido com o valor inválido: `Líquido:  inválido (o aparelho informou 224 °C)`.
- A marca de inválido fica no stdout, na própria linha do valor. É o que o `sensors` do lm-sensors faz com um sensor em falha: em `print_chip_temp` (`prog/sensors/chips.c`), ele imprime `FAULT` no lugar do valor e continua com os outros sensores.
- A nota explicativa vai para o **stderr**. O [Command Line Interface Guidelines](https://clig.dev/) diz que a saída principal vai para o stdout e que "log messages, errors, and so on" vão para o stderr, para que apareçam ao usuário sem entrar num pipe. Assim, quem processa o stdout recebe só os dados. O texto diz que o aparelho já informou valores assim, que eles só voltaram ao normal depois de desligar o PC com corte de energia, e aponta para a seção "Bomba plana e temperatura errada no `DA`" do `docs/hardware.md`.
- O código de saída continua 0. O mesmo guia manda devolver zero no sucesso e reservar os códigos diferentes de zero para as falhas mais importantes. A leitura em si deu certo, e o RPM e o duty são válidos. Um código diferente de zero faria um script descartar dados bons por causa de um sensor, enquanto o lm-sensors segue mostrando os demais sensores. Se o daemon precisar reagir, ele usa a classificação do `halo-core`, e não o código de saída.
- No `--watch`, um booleano no laço controla se a nota já saiu. O `format_telemetry` continua puro e formata só o stdout. A nota é um texto à parte, que o laço imprime no stderr.

### Limite e a faixa de operação

A Asetek indica 60 °C como temperatura máxima de operação do líquido num AIO (FAQ do Asetek 645LT, [SFF Forum](https://smallformfactor.net/forum/threads/asetek-645lt-and-more-faq-ama.11875/post-223284)). A Gigabyte não publica esse limite para o Waterforce X. Um valor entre 60 e 100 °C é, portanto, um aparelho fora da especificação, mas possível. Ele precisa aparecer como número, e não como erro do sensor. Por isso o limite de invalidade continua em 100 °C, e não em 60.

## Risks / Trade-offs

- [Um travamento que dê um valor plausível, como `0x1F` preso, passa como válido] → A change não promete detectar travamento, só o impossível. A spec e o hardware.md deixam isso explícito.
- [A nota recomenda corte de energia com base numa ocorrência só] → O texto descreve o que aconteceu ("só voltaram depois de...") e não promete que resolve. Ele aponta para o registro completo.
- [O limite de 100 °C pode esconder um caso entre 100 e 128 °C] → Isso não é físico para o líquido. Se aparecer, já é sinal de erro do sensor, e mostrar como inválido é o certo.

## Migration Plan

Não há dados persistidos nem interface pública fora do workspace. Para desfazer, basta reverter o commit.
