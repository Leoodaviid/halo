# Tasks

## 1. Probe com as leituras do GCC

- [x] 1.1 Trocar o `FirmwareQuery` em `waterforce/probe.rs` por uma lista de pedidos de leitura (`D6`, `DE 00`, `DD`, `D9 01`, `D9 02`, `C7`, `E8`) com pacote e decodificação de cada um (modelo, firmware maior.menor, modos com 3 lido como Balanced, curva de 4 pontos big-endian, quantidade de fans, modo do LCD). Verificar com testes unitários que cobrem os cenários da spec `waterforce-probe` (`cargo test -p halo-core`)
- [x] 1.2 Atualizar o `format_waterforce_probe` e os testes de integração do `halo-cli` para a saída nova (modelo, modos, curvas, fans, LCD, `--raw` com as sete respostas). Verificar com `cargo test -p halo-cli`
- [x] 1.3 Atualizar o `README.md` com a saída nova do `halo probe waterforce` e verificar que o exemplo bate com o formatador
- [x] 1.4 Com o aval do usuário, rodar `halo probe waterforce --raw` no aparelho. Comandos enviados: `99 D6` (kernel) e `99 DE 00`, `99 DD`, `99 D9 01`, `99 D9 02`, `99 C7`, `99 E8` (`DeviceApi`/`LcdApi` do `ucCooler.dll`, ver `docs/protocols/castor3.md`). Registrar as respostas em `docs/hardware.md` e verificar que cada pedido respondeu com o próprio comando. Se algum não responder, parar e reavaliar antes do grupo 3

## 2. Montagem dos pacotes de LED

- [x] 2.1 Criar `waterforce/led.rs` com `Effect`, `Brightness`, `Speed` e as paletas padrão de `CoolerLedProperty`, e a função pura que gera a sequência de cada efeito conforme o `FUN_1800402a0` da `GcLedLib.dll`. Verificar com testes unitários que comparam cada efeito, byte a byte, com a tabela "Sequência por efeito" do `castor3.md` e com os cenários da spec `waterforce-led` (layout do `C9`, paleta completada, desligar, um `B6` só)
- [x] 2.2 Validar as entradas (brilho de 1 a 10, velocidade de 1 a 5, de 1 a 8 cores, cor recusada nos efeitos sem cor, uma cor só nos de cor única). Verificar com testes unitários dos casos de erro

## 3. Envio e CLI

- [x] 3.1 Implementar o envio: abrir o hidraw pelo `find_hidraw`, mandar `99 DE 00` e conferir `r[2] ∈ {0, 1, 2}`, e então escrever os pacotes de 6144 bytes com 1 ms entre eles, atrás de uma trait de saída. Verificar com testes que usam um falso para mostrar que, com modelo desconhecido ou sem resposta ao `DE`, nenhum pacote de LED sai, e que, com modelo válido, sai a sequência exata
- [x] 3.2 Adicionar `halo led waterforce <efeito> [--color]... [--brightness] [--speed] [--dry-run]` com os códigos de saída 3, 4 e 5 e a mensagem da regra udev. Verificar com testes de integração (sysfs falso para não encontrado e sem permissão, e `--dry-run` sem sysfs nenhum)
- [x] 3.3 Documentar o comando no `README.md` com exemplos (estático, ciclo, três cores, desligar, `--dry-run`) e verificar que cada exemplo roda com `--dry-run`

## 4. Validação no aparelho

- [x] 4.1 Com o aval do usuário, rodar `halo led waterforce static --color ff6600`. Comandos enviados: `99 DE 00`, `99 C9 01 64 0A 02 01` (`FUN_180040fc0`), `99 CD FF 66 00` (`FUN_180041470`) e `99 B6` (`FUN_180040780`), da `GcLedLib.dll`. Verificar com o usuário se o anel e as fans ficaram laranja juntos
- [x] 4.2 Com o aval do usuário, rodar `halo led waterforce cycle` e depois `rainbow` (`99 C9 03 64 0A 02 01` / `99 C9 0A 64 0A 02 01`, `99 B6`). Verificar com o usuário qual se parece com o modo aleatório de fábrica
- [x] 4.3 Com o aval do usuário, rodar um efeito de paleta (`tri-color` com três cores: `C9 0B …`, `B0`–`B3`, `B6`, de `FUN_1800410d0`) e um com velocidade (`pulse --speed 2`), e verificar com o usuário se a cor e o ritmo mudam como esperado
- [x] 4.4 Atualizar o `docs/hardware.md` (resultado visual, anel e fans juntos ou não, modo mais parecido com o de fábrica) e marcar no `docs/protocols/castor3.md` os comandos de LED como confirmados no aparelho, e verificar que o roadmap da Fase 3 reflete o que foi feito

## Workflow follow-up

- Sincronizar as specs (`waterforce-probe` e `waterforce-led`), arquivar a change e commitar, quando o usuário pedir.
