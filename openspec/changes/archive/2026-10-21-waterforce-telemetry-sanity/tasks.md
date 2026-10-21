# Tasks

Nenhuma task envia comandos ao aparelho. Tudo é lido do sysfs do hwmon ou testado com sysfs falso.

## 1. Classificação no halo-core

- [x] 1.1 No `waterforce/telemetry.rs`, adicionar a constante pública do limite (100000 m°C), documentada com a ocorrência de 2026-10-20, e o método do `Telemetry` que devolve o enum `Coolant` (`Valid`/`Implausible`, os dois com o valor em m°C), mantendo o campo `coolant_millicelsius` e o `read()` como estão (design, "API"). Verificar com testes unitários os cenários da spec: `224000` e `240000` inválidos com o valor preservado, `100000` válido, `101000` inválido, `31000` e `29000` válidos. Os testes atuais devem continuar passando com `cargo test -p halo-core`
- [x] 1.2 Adicionar um teste de `read()` com sysfs falso e `temp1_input = 224000` que confira que a leitura não falha, que o RPM e o duty voltam corretos e que a classificação é inválida. Verificar com `cargo test -p halo-core`

## 2. Saída da CLI

- [x] 2.1 Mudar o `format_telemetry` para mostrar `Líquido:  inválido (o aparelho informou 224 °C)` quando a classificação for inválida, e criar o texto da nota (valores assim já apareceram, só voltaram ao normal depois de desligar o PC com corte de energia, ver a seção "Bomba plana e temperatura errada no `DA`" do `docs/hardware.md`). Verificar com testes de formatação: a saída atual com `29000` não muda; com `240000`, a linha do líquido marca a leitura como inválida, as linhas de fan e bomba continuam iguais e a saída formatada não contém a nota
- [x] 2.2 No laço do `halo telemetry waterforce`, imprimir a nota no stderr só na primeira leitura inválida da execução, mantendo o código de saída 0. Verificar com um teste de integração com sysfs falso (`temp1_input = 224000`, sem `--watch`) que confere o código de saída 0, a linha inválida no stdout e a nota no stderr, e não no stdout, e com um teste de unidade da lógica do indicador que confere que, em duas leituras inválidas seguidas, a nota sai uma vez só
- [x] 2.3 Acrescentar ao `README.md`, junto ao exemplo do `telemetry waterforce`, uma linha sobre a temperatura inválida e o corte de energia. Verificar que o texto bate com a mensagem da CLI

## 3. Documentação do hardware

- [x] 3.1 Na seção "Bomba plana e temperatura errada no `DA`" do `docs/hardware.md`, registrar o critério (acima de 100 °C = inválido, e por que não o padrão `e0`/`f0`) e o que o `halo telemetry waterforce` mostra nesse caso. Verificar que a seção cita o limite igual ao da constante da 1.1
- [x] 3.2 Rodar `cargo fmt --check`, `cargo clippy --workspace --all-targets` e `cargo test --workspace` (com `RUSTUP_HOME=/home/bumblebee/.rustup CARGO_HOME=/home/bumblebee/.cargo`) e conferir no aparelho, só lendo, que `halo telemetry waterforce` mostra a temperatura atual como válida
