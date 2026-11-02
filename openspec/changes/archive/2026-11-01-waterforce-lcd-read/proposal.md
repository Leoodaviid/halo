# Proposal

## Why

A tela LCD do Waterforce está ligada, mas toda preta, desde 2026-11-01 por volta das 19:30. Um `halo probe waterforce` às 19:44 mostrou que o modo continua 3, o mesmo de 2026-10-20. O halo ainda não lê mais nada do LCD, então não dá para saber se os itens exibidos foram desligados, se as cores ficaram pretas ou se o firmware travou. Essa leitura também é o primeiro passo do controle do LCD (texto, imagens e vídeo no Windows), e pela regra do projeto todo controle novo começa com uma ferramenta que só lê.

## What Changes

- Novo comando `halo probe waterforce-lcd [--raw]`, só de leitura. Ele lê o modo (`99 E8`) e depois só as leituras que o GCC faz para aquele modo ao abrir a página do LCD (`LcdControl.cs`, linhas 195–330):
  - modos 0 a 3 e 5: itens exibidos (`99 DF`) e cores das zonas (`99 EA <modo+1>`);
  - modo 4: logo (`99 E9`) e cores;
  - modo 6: cores;
  - modos 7 a 9: lista de mídia (`99 F3`, 500 ms, `99 F4`, `99 F5` por item), itens em loop (`99 FD`) e espaço livre (`99 FA <disco>`);
  - modo 11: carrossel (`99 FC`).
- A saída dá o nome de cada modo pelo `enum DisplayMode` do GCC (`Faith1` a `Faith5`, `Function`, `Text`, `Image`, `Gif`, `Video`, `Pet`, `Carousel`), os itens do `DF` pelo `enum Display` e as duas cores do `EA` em `RRGGBB`. O `--raw` mostra as respostas brutas.
- O comando nunca envia comandos de escrita do LCD (`E7`, `E2`, `E4`, `FB`, `F0`, `F1`, `F2`, `F6`, `FE`), nem `B6`, nem nada de LED, fan ou bomba. O `F3` só é enviado nos modos de mídia, como no GCC. O `halo probe waterforce` não muda.
- Docs:
  - `docs/protocols/castor3.md`: os nomes dos modos passam a vir do `DisplayMode` (os modos 0 a 4 são os cinco temas `Faith`, e não "CPU", "temperatura" e "logo", como estava inferido), e entram os bits do `DF` e o desvio do GCC no intervalo do `DF`;
  - `docs/hardware.md`: o probe de 2026-11-01 com a tela preta e o resultado do diagnóstico.
- Fora de escopo:
  - qualquer escrita no LCD, como trocar o modo, as cores ou os itens, ou religar a tela (change `waterforce-lcd-config`);
  - enviar imagens, texto ou vídeo (change `waterforce-lcd-media`);
  - o LCD no `halod` e no painel;
  - mexer no momento do `E1` do `halod` (o GCC espera 20 s), antes de o diagnóstico apontar para ele.

## Capabilities

### New Capabilities
- `waterforce-lcd`: leitura do estado do LCD do Waterforce (modo, itens, cores, logo, mídia e carrossel), com os mesmos pedidos que o GCC faz para o modo atual. As changes de configuração e de mídia vão acrescentar a ela.

### Modified Capabilities
(nenhuma; o `waterforce-probe` continua com os sete pedidos dele)

## Impact

- `crates/halo-core/src/waterforce/lcd.rs` (novo): os pedidos, a decodificação e a sequência por modo, sobre os traits `OutputReports` (de `led.rs`) e `InputReports` (de `probe.rs`), para testar sem o aparelho.
- `crates/halo-cli`: o subcomando `probe waterforce-lcd`, com testes em `tests/probe.rs`.
- Docs: `docs/protocols/castor3.md`, `docs/hardware.md`, `README.md` e `docs/roadmap.md` (Fase 3, item 5).
- Sem dependências novas.
