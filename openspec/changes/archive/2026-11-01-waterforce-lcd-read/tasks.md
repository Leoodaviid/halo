# Tasks

Esta change acrescenta o `halo probe waterforce-lcd`, só de leitura. Ele envia o `99 E8` e, depois, só as leituras que o GCC 26.09.10.01 faz para o modo lido (`LcdControl.cs`, linhas 195–330; bytes do `LcdApi.cs`, tabela em `design.md` e em `docs/protocols/castor3.md`, seção "LCD"). Nenhum comando de escrita do LCD, nenhum `B6` e nada de LED, fan ou bomba. O ensaio roda com a tela preta de 2026-11-01, no modo 3, e por isso envia só `99 E8`, `99 DF` e `99 EA 04`.

## 1. Leitura no core

- [x] 1.1 Criar `crates/halo-core/src/waterforce/lcd.rs`, com o comentário citando cada função do `LcdApi`:
  - o `LcdQuery` de cada pedido (`E8`, `DF`, `EA`, `E9`, `F3`, `F4`, `F5`, `FD`, `FA`, `FC`) com os bytes e a decodificação da tabela da `design.md`;
  - o nome dos modos (`DisplayMode`) e dos bits do `DF` (`Display`);
  - a função que dá os pedidos de cada modo.

  Verificar com testes em `cargo test -p halo-core lcd`:
  - pacotes de 6144 bytes completados com zeros;
  - `99 e8 04` dá o modo 3, `Faith4`, e `99 e8 00` dá modo desconhecido;
  - o `DF` com 1 em `r[6]` e `r[8]` dá `CPUTemperature` e `LiquidTemperature`, e com tudo em 0 dá "nenhum item";
  - o `EA` `99 ea 04 ff 00 00 00 00 ff` dá `ff0000` e `0000ff`;
  - o `FA` com `n = 3` decodifica little-endian;
  - o `FC` e o `FD` param no primeiro 0;
  - o `F5` lê o nome até o primeiro byte zero e a marca de loop em `r[4]`;
  - o disco é `0x42` nos modos 7 e 8 e `0x45` no 9.
- [x] 1.2 Acrescentar o `probe_lcd` sobre `OutputReports + InputReports`: o `E8`; nada mais se ele não responder; as leituras do modo, cada uma esperando a resposta do próprio comando por até 2 s (o `F5` confere também modo e índice); o `F3` sem resposta e com a espera de 500 ms como parâmetro. Verificar com um falso que responde por comando e grava cada pacote, em `cargo test -p halo-core lcd`:
  - o modo 3 envia exatamente `E8`, `DF`, `EA 04`;
  - o modo 4 envia `E8`, `E9`, `EA 05`;
  - o modo 6 envia `E8`, `EA 07`;
  - o modo 7 com 2 itens envia `E8`, `F3 01`, `F4 01`, `F5 01 00`, `F5 01 01`, `FD 08`, `FA 42`;
  - o modo 11 envia `E8`, `FC`;
  - o modo 10 envia só o `E8`;
  - `E8` sem resposta envia só o `E8`;
  - um `DA` antes do `DF` é descartado;
  - um `F5` de outro índice é descartado;
  - em todos os modos de 0 a 12, nenhum pacote tem `E7`, `E2`, `E4`, `FB`, `F0`, `F1`, `F2`, `F6`, `FE` ou `B6` como comando, e o `F3` só aparece de 7 a 9.

## 2. Comando na CLI

- [x] 2.1 Em `crates/halo-cli/src/main.rs`, acrescentar `halo probe waterforce-lcd [--raw]`:
  - a saída tem o hidraw, o modo com o nome, os itens, as cores, o logo, a mídia, o loop, o espaço livre e o carrossel, conforme o modo, e o `r[11]` do `DF` cru;
  - o `--raw` acrescenta os primeiros 64 bytes de cada resposta;
  - códigos de saída: 0 se tudo respondeu; 5 se algo não respondeu; 3 sem o aparelho; 4 sem permissão, com a mensagem da regra udev;
  - a ajuda diz que o comando só lê e quais pedidos envia.

  Verificar com testes em `crates/halo-cli/tests/probe.rs`: sem o Waterforce no sysfs falso, código 3; a ajuda cita o `E8` e "só lê"; o `probe waterforce --help` continua igual.
- [x] 2.2 Documentar:
  - em `docs/protocols/castor3.md`, os nomes dos modos pelo `DisplayMode` (0 a 4 são os temas `Faith1` a `Faith5`; tirar a inferência "CPU, temperatura, logo"), os bits do `DF` pelo `Display`, o `r[11]` e o desvio do GCC no intervalo do `DF`, e a sequência de leituras por modo do `LcdControl`;
  - no `README.md` e no `docs/roadmap.md` (Fase 3, item 5), o `probe waterforce-lcd`.

  Verificar que o comando citado aparece no `halo probe --help` e que a tabela de modos do `castor3.md` cita o `DisplayMode.cs`

## 3. Ensaio no aparelho

- [x] 3.1 Com a tela preta e o `halod` ativo, com o aval do usuário, rodar `halo probe waterforce-lcd --raw` (no modo 3: `99 E8`, `99 DF`, `99 EA 04`, do `LcdApi.GetMode`, `GetDisplay` e `GetZoneColor`) e, logo depois, `halo telemetry waterforce` e o journal do `halod`. Conferir:
  - código 0 e as três respostas;
  - o `halod` continua enviando o feed, sem erro no journal;
  - a tela, o anel e a fan não mudam com o probe.

  Registrar em `docs/hardware.md`, com a hora, os bytes do `DF` e do `EA` e qual hipótese eles sustentam: H2 (nenhum item ligado), H3 (cores pretas) ou H4 (tudo normal, e a suspeita fica no firmware). Não enviar nada além disso.
- [x] 3.2 Se o resultado for H4: com o aval do usuário, desligar com corte de energia, ligar e repetir o `halo probe waterforce-lcd --raw`, registrando em `docs/hardware.md` se a tela voltou e se o `DF` e o `EA` mudaram. Se for H2 ou H3, registrar que religar a tela depende da `waterforce-lcd-config` e não fazer o corte de energia

## 4. Verificação

- [x] 4.1 Rodar `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` e `openspec validate waterforce-lcd-read --strict`, todos passando

## Workflow follow-up

- `/cross-review` da branch antes do sync e do archive.
- Sync da spec `waterforce-lcd`, archive e commit, só a pedido.
