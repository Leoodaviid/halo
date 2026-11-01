# Tasks

O halo só endereça `0x58` + slot dos pentes Kingston suportados nesta change (hoje `0x5A` e `0x5B`). Primeiro vêm as leituras do FURY CTRL (`0x0B`, `0x09`, `0x0E`). As escritas `setDRAMDDR5_AllOff`, brilho 80 e `setDRAMDDR5_DefaultRainbow` só vêm depois do ensaio de leitura. As fontes são o `FuryCTRL.dll` 2.0.66.0 (`SetKingstonFuryDDR5*`, `GetKingstonFuryDDR5*`, decompilados com o REA) e o serviço (`QueryDDRInformation`, `CommandStart`, `CommandDoneAndRun`), conferidos com o `KingstonFuryDRAMController` do OpenRGB `72262b8`. Os comandos que tocam o barramento só rodam no terminal do usuário, com `sudo` e com o aval dele. O grupo 4 (escrita) só começa depois que a tarefa 3.1 confirmar que os controladores respondem.

## 1. Controlador no core, só leitura

- [x] 1.1 Em `crates/halo-core/src/ram.rs`, acrescentar ao `SpdSlot` o número do adaptador (`i2c-N`). Verificar que os testes atuais de `cargo test -p halo-core ram` continuam passando e que o `ram_acha_pelo_nome_do_adaptador` confere o número `7`
- [x] 1.2 Criar o módulo do controlador (`ram::controller`), com o comentário citando as funções do `FuryCTRL.dll` e do OpenRGB:
  - `Controller` construído só de um `SpdSlot` suportado com marca `F` e modelo 2, 3 ou 4 (API v2), com `0x58 + slot` e slot de 0 a 3; os modelos da API v1 e os sem nome dão um erro próprio;
  - `locate`, só pelo sysfs: os controladores, o adaptador e o nó (`<adaptador>/i2c-dev/i2c-N` → `<dev>/i2c-N`), com "SPD ilegível" separado de "sem pente suportado";
  - o trait `Smbus` com `read_word_data(Controller, ReadReg)` e `write_byte_data(Controller, WriteReg, u8)`;
  - o `I2cDev`: um único `O_RDWR`, o `flock` (5 s, como o `Hidraw`, com o laço extraído para uma função comum), o `I2C_SLAVE` a cada troca de controlador (nunca `I2C_SLAVE_FORCE`, e `EBUSY` vira `AddressBusy`) e o `I2C_SMBUS` de word data;
  - `read_controllers`: `0x0B`, `0x09` e `0x0E` de cada controlador, com o byte alto e 5 ms entre as leituras, separando `ENXIO` de outra falha.

  Verificar com testes de unidade em `cargo test -p halo-core ram`:
  - sysfs falso com SPDs suportados em `0x52`/`0x53` dá `0x5A`/`0x5B` e o nó certo;
  - SPD reprovado dá "sem pente suportado", e SPD curto ou com modo `000` dá "SPD ilegível";
  - Fury Beast (modelo 1), Fury Renegade (0), HyperX (`H`) e `H` com modelo 3 dão "pente da API v1" ou "sem pente suportado", nunca um `Controller`; Beast SE (2), Renegade Refresh (3) e Beast Refresh (4) dão `Controller`;
  - adaptador sem `i2c-dev` dá "sem nó";
  - um `Smbus` falso cobre a leitura dos três registradores (byte alto), o `ENXIO` e o `EIO`;
  - o `I2cDev` num arquivo comum dá o erro do ioctl;
  - o `flock` tomado dá `Busy`.

## 2. Comando de leitura na CLI

- [x] 2.1 Em `crates/halo-cli/src/main.rs`, acrescentar `halo probe ram-controller` e o `--dev-root` oculto (padrão `/dev`). A ajuda diz que o comando só lê os registradores `0x0B`, `0x09` e `0x0E` em `0x58` + slot e precisa de `sudo`. Por pente, a saída mostra a instância, o endereço, o papel, o estilo (com nome quando conhecido) e a velocidade; os códigos de saída seguem a design. Verificar com testes em `crates/halo-cli/tests/probe.rs`, num sysfs e num `/dev` falsos:
  - sem adaptador, com SPD reprovado ou com um Fury Beast (API v1): código 3, sem abrir o nó (o nó falso em modo `000` daria 4);
  - adaptador sem `i2c-dev`: código 3;
  - nó com modo `000`: código 4, `sudo` no stderr e nada de "grupo" nem `usermod` (pulado com aviso se rodar como root);
  - nó que é um arquivo comum: código 5, com o erro original e sem "nada respondeu";
  - `probe ram-controller --help` no `ajuda_dos_comandos`.

  Tudo em `cargo test -p halo-cli --test probe`
- [x] 2.2 Documentar em `docs/permissions.md` por que os comandos de RAM usam `sudo` e não o grupo `i2c` (a regra 72; o VRM e os DACs no mesmo barramento) e acrescentar o `probe ram-controller` ao `README.md`. Verificar que o comando citado aparece no `halo probe --help` e que o texto não sugere o `usermod`

## 3. Ensaio de leitura no aparelho

- [x] 3.1 No terminal do usuário, com o aval dele: `sudo target/debug/halo probe ram-controller` e `sudo strace -f -e trace=openat,ioctl target/debug/halo probe ram-controller`. Esperado:
  - `0x5A` e `0x5B` respondem, código 0;
  - o único nó aberto é o `/dev/i2c-0`;
  - os `I2C_SLAVE` só têm os argumentos `0x5a` e `0x5b`;
  - só há `I2C_SMBUS` de leitura.

  Registrar os valores de papel, estilo e velocidade em `docs/hardware.md`. Se nada responder, parar a change aqui e rever o plano com o usuário

## 4. Escritas mínimas

- [x] 4.1 No core, acrescentar `RamCommand` (`Off`, `On`, `Default`), o plano como valor e o executor sobre um `Smbus`. O plano tem:
  - a leitura do `0x0B` de todos os pentes primeiro;
  - `0x53` do maior slot para o menor e 50 ms;
  - as escritas de cada pente, na ordem da spec;
  - `0x44` do menor slot para o maior e 50 ms.

  O executor faz até 30 tentativas por escrita, com 5 ms entre elas e 5 ms depois de cada escrita, e para na primeira falha definitiva, com o endereço e o registrador. Verificar com um `Smbus` falso em `cargo test -p halo-core ram`:
  - `off` com papéis `0x00` e `0x01` dá a sequência exata do cenário da spec;
  - `on` só escreve `0x0B` e `0x20` = `0x50`;
  - `default` escreve as nove escritas na ordem;
  - uma leitura de papel com `ENXIO` dá zero escritas;
  - `0x5A` `0x20` falhando 30 vezes não envia nada depois;
  - uma falha passageira (2 vezes) segue;
  - as pausas medidas são de pelo menos 5 ms e 50 ms.
- [x] 4.2 Na CLI, acrescentar `halo led ram <off|on|default> [--dry-run]`. O `--dry-run` imprime o plano com `0x5A`/`0x5B` e o papel como "lido na hora", sem ler o sysfs nem abrir o nó. A falha na moldura diz o endereço, o registrador e que repetir o comando a refaz. Verificar com testes em `crates/halo-cli/tests/led.rs`:
  - `led ram off --dry-run` sem sysfs: o plano e código 0;
  - `led ram purple`: código 2;
  - sem pente suportado: código 3, sem abrir o nó;
  - nó com modo `000`: código 4 e `sudo`;
  - `led ram --help` lista os três comandos.
- [x] 4.3 Acrescentar o `halo led ram` ao `README.md` e ao `docs/roadmap.md` (Fase 5, item 2: as escritas mínimas da API v2 existem; os outros efeitos, as cores e o daemon ficam pendentes). Verificar que os comandos citados aparecem no `halo led --help`

## 5. Correções depois do primeiro ensaio de escrita

O ensaio de 2026-11-01 mostrou que o `0x0B` é o índice de posição do pente e muda sozinho uns 30 s depois do boot, e que leituras isoladas devolvem lixo (`docs/hardware.md`, "Primeiras escritas" e "Estudo do `0x0B`"). Este grupo só começa depois do teste de controle da 5.1. As tarefas 4.1 e 4.2 foram feitas com o plano antigo (papel relido e escrito de volta); este grupo as corrige.

- [x] 5.1 Teste de controle, no aparelho, sem nenhum comando do halo: desligar com corte de energia, ligar e observar os pentes por 5 minutos. Registrar em `docs/hardware.md` se eles ficam em rainbow ou passam sozinhos para outro efeito (como o azul-claro estático visto depois das leituras). Se só mudarem quando o halo lê, parar a change e rever o probe com o usuário antes do 5.2
- [x] 5.2 No core, leituras com até 5 tentativas (pausa de 10 ms × tentativa), refazendo as que falham ou têm byte alto `0xFF`, com "leitura inválida (`0xFF`)" como erro próprio. Verificar com o `FakeBus` em `cargo test -p halo-core ram`: lixo isolado é refeito e o valor bom é usado; 5 × `0xFF` dá leitura inválida, que não é "nada respondeu"; 5 × `ENXIO` dá "nada respondeu"; `ENXIO` seguido de sucesso segue
- [x] 5.3 No core, o índice calculado (slots em ordem decrescente: maior slot 0, seguinte 1…) no lugar do papel lido: o plano não tem mais leituras (`Step::ReadRole` e `Value::Role` saem), e o `0x0B` de cada pente é o índice. Um `ENXIO` nas 30 tentativas da primeira escrita vira um erro próprio ("nada respondeu, nada foi escrito"). Verificar em `cargo test -p halo-core ram`: o `off` escreve `0x5A` `0x0B`=`0x01` e `0x5B` `0x0B`=`0x00`, mesmo com o `0x0B` do `FakeBus` em `0x0E`; o `on` e o `default` também; o `FakeBus` não registra nenhuma leitura; o primeiro `0x53` com `ENXIO` dá o erro próprio e mais nada é escrito
- [x] 5.4 Na CLI, o probe mostra "Índice (0x0B)" e o aviso para não rodar em sequência (na saída e na ajuda), o `led ram` sai com 3 no erro de "nada respondeu" da primeira escrita, e o `--dry-run` mostra só a moldura, com os índices `0x01`/`0x00`. Verificar com os testes de `crates/halo-cli/tests/led.rs` e `probe.rs` atualizados, e com `halo led ram off --dry-run`

## 6. Verificação

- [x] 6.1 Rodar `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` e `openspec validate ram-hyperx-default-modes --strict`, todos passando
- [x] 6.2 No terminal do usuário, com o aval dele, depois de um desligamento com corte de energia:
  - `sudo target/debug/halo probe ram-controller` uma única vez, logo depois de ligar: índices `0x01`/`0x00`, estilo `0x01`;
  - `sudo target/debug/halo led ram off`: os dois pentes apagam, código 0 (sem probe entre os comandos);
  - `sudo target/debug/halo led ram on`: o rainbow volta sincronizado (comparar o brilho);
  - se não voltar igual, `sudo target/debug/halo led ram default`, e registrar o resultado;
  - `sudo strace -f -e trace=openat,ioctl target/debug/halo led ram off`: só `0x5a`/`0x5b` nos `I2C_SLAVE` e só o `/dev/i2c-0` aberto;
  - se o estado sobrevive a um reboot e a um desligamento com corte de energia.

  Registrar tudo em `docs/hardware.md`

## Workflow follow-up

- `/cross-review` da branch antes do sync e do archive.
- Sync da spec `ram-led`, archive e commit, só a pedido.
