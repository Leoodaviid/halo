# Proposal

## Why

Em 2026-10-20, cerca de 33 s depois de uma curva plana da bomba, o Castor3 passou a informar no byte `0x0D` do status `99 DA` os valores `e0` e `f0`, e o `temp1_input` do hwmon foi a 224 e 240 °C. O valor só voltou a uma temperatura plausível (31 °C) depois de desligar o PC com corte de energia (`docs/hardware.md`, "Bomba plana e temperatura errada no `DA`"). O driver do kernel repassa o byte sem validação (`waterforce_raw_event`), e o `halo telemetry waterforce` mostra `Líquido: 224,0 °C` como se fosse uma leitura válida.

A causa não é conhecida e aconteceu uma vez só, mas um líquido a 224 °C é fisicamente impossível num watercooler. Hoje o erro só engana quem lê o terminal. Quando o daemon e o painel da Fase 2 usarem a telemetria, ele vai aparecer na barra do GNOME e pode alimentar alertas ou perfis. É melhor tratar o valor agora, enquanto a ocorrência está documentada e o código da telemetria é pequeno.

## What Changes

- O `halo-core` passa a classificar a temperatura do líquido lida no hwmon como válida ou fora da faixa. O valor fica fora da faixa quando passa de 100 °C, o ponto de ebulição da água. O valor bruto continua disponível para quem quiser registrar a ocorrência.
- O `halo telemetry waterforce` mostra a temperatura fora da faixa como inválida, com o valor que o aparelho informou, e não como uma temperatura. Uma nota diz que o aparelho já informou valores assim antes, que eles só voltaram depois de desligar o PC com corte de energia, e aponta para o `docs/hardware.md`. Fan e bomba continuam aparecendo normalmente, e o código de saída continua 0, porque o RPM e o duty seguem válidos.
- No `--watch`, a nota aparece uma vez por execução, e não a cada leitura.
- `docs/hardware.md` registra o critério adotado e como o halo mostra o erro.
- Nenhum comando novo vai ao aparelho. A leitura continua só no sysfs do hwmon, sem abrir o hidraw.
- Fora do escopo:
  - descobrir a causa do travamento;
  - ler o byte 13 direto do hidraw;
  - qualquer ação automática de recuperação (o halo não manda nada ao aparelho por causa disso);
  - alertas e notificações, que ficam para o daemon.

## Capabilities

### New Capabilities
- Nenhuma.

### Modified Capabilities
- `waterforce-telemetry`: o requisito "Valores apresentados" passa a tratar a temperatura do líquido acima de 100 °C como leitura inválida, e um requisito novo define essa validação no `halo-core` e o que a CLI mostra.

## Impact

- `crates/halo-core/src/waterforce/telemetry.rs`: o limite de plausibilidade e a classificação da temperatura do líquido, com testes que incluem os valores reais `224000` e `240000` e a referência `31000`.
- `crates/halo-cli/src/main.rs`: o `format_telemetry` e o laço do `--watch`, com testes de formatação.
- `docs/hardware.md`: o critério e o comportamento do halo diante do erro.
- `README.md`: uma linha sobre a temperatura inválida, na seção da telemetria.
- Hardware: nenhum envio. A change só lê o sysfs.
