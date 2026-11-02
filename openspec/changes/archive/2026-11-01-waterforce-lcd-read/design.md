# Design

## Context

- O `halo probe waterforce` (`crates/halo-core/src/waterforce/probe.rs`) envia uma lista fixa de sete leituras e lê as respostas com o `wait_response`, que descarta os input reports de outro comando, como os `DA` do kernel, por até 2 s (`RESPONSE_TIMEOUT`). Só o `wait_response` é testado com um falso (`InputReports`); o laço do `probe()` escreve direto no `Hidraw`.
- O `led.rs` já tem o trait `OutputReports`, implementado pelo `Hidraw` e por falsos nos testes. O `Hidraw::open` toma um `flock` exclusivo, e é isso que serializa o probe com o `halod`, que manda `E0` a cada 2 s. O `probe waterforce` de 2026-11-01, às 19:44, rodou com o `halod` ativo e saiu com código 0.
- O que o GCC 26.09.10.01 faz, em `ucCooler.Views/LcdControl.cs`, linhas 195–330, e `ucCooler.Api/LcdApi.cs`:
  - lê o `E8` e só as leituras do modo atual;
  - cada `Get*` monta um pedido de 8 ou 9 bytes e chama o `CoolerApi.Receive`, que completa até o `OutputReportLen` e confere os primeiros bytes da resposta (2 por padrão; 4 no `F5`, com `nRecvCheckLength = 3`).

| Pedido | Bytes | Decodificação no GCC | Fonte |
|---|---|---|---|
| Modo | `99 E8` | `r[2] - 1` | `LcdApi.GetMode` (L44) |
| Itens | `99 DF` | bit `i` ligado se `r[2+i] == 1`, `i` de 0 a 8; o GCC guarda o intervalo em `r[2]` (o mesmo byte do bit 0), e o `SetDisplay` o escreve em `[11]` | `LcdApi.GetDisplay` (L126), `SetDisplay` (L97) |
| Cores | `99 EA <modo+1>` | zona 1 `RGBUtil(r[3], r[4], r[5])`, zona 2 `r[6..8]` | `LcdApi.GetZoneColor` (L182) |
| Logo | `99 E9` | `r[2]`; 1 escolhe a imagem `teamup` no `GLed_0x4016_*` | `LcdApi.GetGBTLogo` (L211) |
| Lista de mídia | `99 F3 <modo-6>` sem resposta, 500 ms, `99 F4 <modo-6>` → `r[3]` itens; `99 F5 <modo-6> <i>` → loop `r[4] == 1`, nome em `r[6..]` | o GCC descarta nomes com menos de 5 caracteres e troca a extensão por `.gif` no modo 8 | `LcdApi.GetMediaList` (L231) |
| Loop | `99 FD <modo+1>` | intervalo `r[3]`, índices `r[4..] - 1` até 0 | `LcdApi.GetMediaLoopIndex` (L370) |
| Espaço livre | `99 FA <disco>` | `n = r[2]`, tamanho em `r[3..3+n]` little-endian | `LcdApi.GetDiskFree` (L409), `GetDisk` (L31) |
| Carrossel | `99 FC` | intervalo `r[2]`, modos `r[3..] - 1` até 0 | `LcdApi.GetLoop` (L526) |

Os nomes dos modos e dos bits vêm de `ucCooler.Models.Lcd/DisplayMode.cs` e `Display.cs`.

## Goals / Non-Goals

**Goals:**
- O mesmo conjunto e a mesma ordem de pedidos que o GCC envia para o modo atual, provados por testes contra um aparelho falso que registra cada pacote.
- Ler os três dados que faltam ao diagnóstico da tela preta no modo 3: o `DF`, o `EA 04` e o modo pelo nome certo.

**Non-Goals:**
- Ler o estado de outros modos, como as cores do modo 7 com a tela no modo 3. O GCC nunca faz isso, e o halo não inventa pedidos.
- Interpretar o `r[11]` do `DF` como intervalo. Ele é mostrado cru, porque o GCC lê o intervalo do byte errado.

## Decisions

- **Comando separado, `halo probe waterforce-lcd`**, e não mais pedidos no `probe waterforce`. A spec `waterforce-probe` fixa os sete pedidos dele, e o probe atual continua sendo o mais seguro. O LCD ganha uma capability própria (`waterforce-lcd`), que vai crescer com a config e a mídia.
- **Sequência por modo, numa tabela.** O módulo novo `waterforce/lcd.rs` tem um `LcdQuery` para cada pedido, com bytes e decodificação, e uma função que devolve os pedidos de um modo, conforme a tabela acima. O `probe_lcd` envia o `E8` e então percorre essa lista. Isso deixa a ordem testável sem o aparelho.
  - *Alternativa:* um laço com `match` por modo, enviando direto. Descartada porque a ordem só seria vista com o aparelho.
- **Transporte por traits.** O `probe_lcd` recebe `&(impl OutputReports + InputReports)`, que o `Hidraw` já implementa. O falso dos testes responde por comando e guarda cada pacote enviado. Assim os cenários "só `E8`, `DF` e `EA 04` no modo 3" e "nenhum comando de escrita" viram testes.
- **A lista de mídia como o GCC:**
  - o `F3` é escrito sem esperar resposta;
  - a espera de 500 ms é parâmetro do `probe_lcd`, 500 ms no comando e 0 nos testes;
  - o `F5` aceita só a resposta que repete `F5 <modo-6> <i>`, como o `nRecvCheckLength = 3`;
  - os nomes são mostrados como vêm, sem o filtro de menos de 5 caracteres nem a troca de extensão do GCC, que são coisas da interface. Mostrar mais não envia nada a mais.
- **Erros:** se o `E8` não responder, nada mais é enviado, porque sem o modo não há como escolher as leituras, e o comando sai com 5. Se outra leitura não responder, as demais são mostradas e o comando também sai com 5. Sem o aparelho, 3; sem permissão, 4, com a mesma mensagem da regra udev do `probe waterforce` (`ProbeError`).
- **O `E1` do `halod` fica como está.** O `castor3.md` registra que o GCC espera 20 s antes do `E1` e que isso nunca foi testado como causa. Esta change só lê. Se o diagnóstico apontar para o firmware, mudar o `E1` é outra change.

- **Prazo total de 10 s com o aparelho aberto (`PROBE_BUDGET`)**, achado do juiz externo na revisão da branch (2026-11-01). O probe segura o `flock` do Castor3 do começo ao fim, e o `halod` espera o lock por só 5 s e renova o watchdog de 20 s só com envios aceitos. Sem prazo, uma lista de mídia com dezenas de `F5` mudos (2 s cada) derrubaria o `halod` e impediria até o `E0` de 80 °C do `ExecStopPost`. Com o prazo, cada espera é limitada ao que sobra, e a lista para no primeiro `F5` sem resposta. O `E8` sempre vai, com até 2 s.
  - *Observação, fora desta change:* o `probe waterforce` antigo tem o mesmo padrão, com 7 pedidos de até 2 s, ou 14 s no pior caso, abaixo dos 20 s do watchdog, mas sem folga.

## Risks / Trade-offs

- [O `F3` não é uma leitura pura: ele não tem resposta, e o efeito no firmware não é conhecido] → Ele só sai nos modos 7 a 9, como no GCC. A tela de hoje está no modo 3, então o ensaio desta change não o envia. Isso fica registrado como não ensaiado no aparelho.
- [Os nomes dos modos podem não ser o que a tela desenha] → Eles vêm do enum do GCC, e não de uma inferência. A correspondência com o desenho de cada tema continua não confirmada e fica assim no `castor3.md`.
- [Ler no meio do feed do `halod`] → O `flock` serializa o acesso, e o probe de 2026-11-01 já rodou assim. O ensaio confere que o `halod` continua enviando depois.
- [O diagnóstico pode não explicar a tela preta, se o `DF` e o `EA` estiverem normais] → Isso aponta para o firmware (H4). O próximo passo, com o aval do usuário, é um corte de energia, registrando o antes e o depois. O comando de religar a tela é de outra change.
