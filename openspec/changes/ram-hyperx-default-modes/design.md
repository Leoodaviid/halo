# Design

## Context

Ver proposal.md, "Why". O estado de que a abordagem depende:

- **Barramento.** O SMBus da porta 0 do FCH (`SMBus PIIX4 adapter port 0 at 0b00`, hoje `i2c-0`) tem:
  - os SPDs `0x52` e `0x53`;
  - os seletores de página `0x36`/`0x37`, presos pelo kernel como `dummy`;
  - o VRM (`0x60`) e os DACs `NCT3933` (`0x10` e `0x15`, este com a tensão da DRAM).

  O `0x27` não responde (`ENXIO`, 2026-11-01). O nó `/dev/i2c-0` é `root:i2c 660`, e o adaptador tem `i2c-dev/i2c-0` no sysfs.
- **Serviço do FURY CTRL** (`ilspy-FuryControllerService/`):
  - O `CheckDramSupport` percorre o SPD de 8 bits `0xA0` a `0xAE`, com `j_index` de 0 a 7, ou seja, o índice do SPD.
  - O `QueryDDRInformation` dá ao `KingstonFury_Beast_Refresh` `type = 1`, `api_ver = 2` e `address_offset = j_index`. O `master_slave` (na verdade o índice de posição, ver abaixo) ele lê uma vez com o `GetKingstonFuryDDR5MasterSlaveRole(88 + j_index)` e guarda.
  - Com `type = 1`, o `iDDRBaseAddress` é `88` (`0x58`), então o controlador do pente no SPD `0x50 + s` fica em `0x58 + s`.
  - O `CommandStart` envia `SetKingstonFuryDDR5CmdStart` aos slots usados, do último para o primeiro, e depois espera 50 ms. O `CommandDoneAndRun` envia `SetKingstonFuryDDR5CmdDone` do primeiro para o último e espera 50 ms.
  - O `setDRAMDDR5_AllOff` é `CommandStart` → por slot, `SetKingstonFuryDDR5MasterSlaveRole(índice guardado)` e `SetKingstonFuryDDR5Brightness(0)` → `CommandDoneAndRun`.
  - O `setDRAMDDR5_DefaultRainbow` é `CommandStart` → por slot, `Style(1)`, `MasterSlaveRole(índice guardado)`, `IRDelay(0)`, `Direction(1)`, `Speed(25)`, `setDDR5Brightness(80)`, `Width(0)`, `Hue(0)` e `HyperSpeed(0)` → `CommandDoneAndRun`. É o mesmo efeito do `reset_default_effect` (`rainbow_1`) na API 2.
  - O brilho padrão do serviço é `_brightness = 80`.
- **`FuryCTRL.dll` 2.0.66.0, no caminho comum** (sem o LPC Intel do X99/X299):
  - cada `SetKingstonFuryDDR5*` é um write byte data no endereço de 8 bits `param_1 * 2`, com até 30 tentativas (`0x1E`), seguido de `Sleep(5)`. Os registradores: CmdStart `0x08`=`0x53`, CmdDone `0x08`=`0x44`, Style `0x09`, MasterSlaveRole `0x0B`, Direction `0x0C`, IRDelay `0x0D`, Speed `0x0E`, Width `0x10`, Hue `0x11`, HyperSpeed `0x1E`, Brightness `0x20`;
  - cada `GetKingstonFuryDDR5*` (MasterSlaveRole `0x0B`, Style `0x09`, Speed `0x0E`) chama o `SMB_BlockRead2` do `SMBus_Engine.dll` (operação `4` do backend `ATISMBus`, não decompilada até o fim), guardando `valor >> 8`, seguido de `Sleep(5)`. Nenhuma escrita vem antes.
- **O `0x0B` é o índice de posição, não um papel** (2026-11-01, `docs/hardware.md`, "Estudo do `0x0B`"):
  - o OpenRGB o chama de `FURY_REG_INDEX` e diz que o software oficial escreve os índices em ordem decrescente, da borda para o slot da CPU, para até 4 pentes; o próprio OpenRGB calcula e escreve o índice, nunca o lê;
  - o FURY CTRL lê o `0x0B` uma vez, na detecção com o serviço recém-iniciado, e reusa o valor guardado em todas as escritas;
  - no aparelho, o `0x0B` lido logo depois de ligar é `0x01` (slot 2) e `0x00` (slot 3); uns 25–35 s depois, sem nenhuma escrita, passa a `0x0E` nos dois;
  - leituras isoladas devolvem lixo (`0xFF`, `0x5A`) ou `ENXIO` em cerca de 3% das vezes; o OpenRGB tenta cada leitura até 5 vezes e trata `0xFFFF` como erro.
- **OpenRGB `72262b8`** (`KingstonFuryDRAMController`): tem os mesmos registradores (`FURY_REG_APPLY` `0x08`, `MODE` `0x09`, `INDEX` `0x0B`, `BRIGHTNESS` `0x20`; `FURY_BEGIN_TRNSFER` `0x53`, `FURY_END_TRNSFER` `0x44`) e os mesmos estilos (0 estático, 1 rainbow, 3 respiração, 4 dinâmico). Os endereços são os mesmos (`0x58` + slot), mas a detecção do OpenRGB escreve antes de ler. O halo segue a do FURY CTRL, que só lê.
- **Estado atual dos pentes:** uma cor para a barra toda, trocando. Qual estilo é esse, o probe diz.

## Goals / Non-Goals

**Goals:**
- A trava de endereços e de registradores está nos tipos: não existe caminho no código que monte uma transação fora da spec.
- Todo o plano de um comando é um valor (a lista de transações), o mesmo usado pelo `--dry-run` e pela execução.

**Non-Goals:**
- Restaurar o efeito exato de antes do `off`. O brilho não tem leitura; o `on` usa o 80 do FURY CTRL.
- Os outros efeitos, mesmo os que só mudam o estilo.

## Decisions

- **O endereço é um tipo.** `Controller` só é construído a partir de um `SpdSlot` com SPD decodificado, `Verdict::Supported`, marca `F` e modelo 2, 3 ou 4 (Beast SE, Renegade Refresh, Beast Refresh: os DDR4 com `api_ver = 2` no `QueryDDRInformation`), com `0x58 + (endereço do SPD − 0x50)` e o slot de 0 a 3. Não há construtor público a partir de um número. Um `Verdict::Supported` da API v1 (HyperX, Renegade, Beast) ou sem nome não vira `Controller`: o `locate` devolve um erro próprio ("pente da API v1"), com código 3, sem abrir o nó.
  - *Por quê:* o `Verdict::Supported` vem do `FUN_1000af50`, que aprova as duas APIs; a escolha da API fica no serviço, pelo nome do modelo.
  - *Alternativa:* uma lista fixa `[0x5A, 0x5B]`. Descartada: amarra o código a este PC e escreveria num slot sem pente se a placa mudasse.
- **Um único nó aberto, com o `I2C_SLAVE` trocado por controlador.** O `flock` é por descrição de arquivo aberto, então abrir o nó duas vezes no mesmo processo travaria a si mesmo. O `I2cDev`:
  - abre o nó com `O_RDWR` e toma o `flock` (5 s, depois `Busy`);
  - faz `ioctl(I2C_SLAVE, addr)` sempre que o controlador da transação muda;
  - nunca usa o `I2C_SLAVE_FORCE`. Um `EBUSY` no `I2C_SLAVE` vira `AddressBusy`.

  O trait `Smbus` só tem `read_word_data(Controller, ReadReg)` e `write_byte_data(Controller, WriteReg, u8)`. `ReadReg` e `WriteReg` são enums com só os registradores da spec.
  - *Alternativa:* o crate `i2cdev`. Descartada: dependência nova para três ioctls, e a API genérica aceita qualquer endereço.
- **A escolha do adaptador vem do `ram::find`.** O `SpdSlot` ganha o número do adaptador. O nó é `<dev>/i2c-N`, e só existe se `<adaptador>/i2c-dev/i2c-N` existir no sysfs. O `--dev-root` é oculto, como o `--sys-root`, para os testes. A diferença entre "sem pente suportado" e "SPD ilegível" segue a spec (o `ENXIO` não conta).
- **Plano como valor.** `RamCommand` (`Off`, `On`, `Default`) com os papéis lidos gera `Vec<Op>`:
  1. `0x53` do maior slot para o menor;
  2. uma pausa de 50 ms;
  3. as escritas de cada pente, na ordem da spec;
  4. `0x44` do menor slot para o maior;
  5. outra pausa de 50 ms.

  O executor faz até 30 tentativas por escrita, com 5 ms entre elas, e 5 ms depois de cada escrita bem-sucedida. O espaço entre as tentativas não aparece no DLL (fica no motor do `SMBus_Engine`); 5 ms é uma escolha do halo.
- **Índice calculado, nunca relido.** O valor de `0x0B` nas escritas vem dos slots: os pentes controláveis em ordem decrescente de slot recebem 0, 1, 2… Nesta máquina, slot 3 → `0x00` e slot 2 → `0x01`, os mesmos valores lidos logo depois do boot, que é o que o FURY CTRL guardaria. O plano não tem passo que leve um valor lido a uma escrita.
  - *Alternativa:* não escrever o `0x0B` no `off`/`on`. Descartada: os dois fluxos do FURY CTRL escrevem o índice, e não se sabe se o controlador aceita a moldura sem ele.
  - *Alternativa:* guardar o `0x0B` lido no primeiro uso depois do boot. Descartada: o halo não sabe quanto tempo faz que o PC ligou, e o valor muda sozinho em segundos.
- **Leituras com tentativas, só no probe.** Cada leitura tem até 5 tentativas, só enquanto falhar, com pausa de 10 ms × número da tentativa entre elas (como o `3 × retries × 10 ms` do OpenRGB, mais curto porque são só três registradores) e 5 ms depois de cada transação. Uma tentativa com erro ou com byte alto `0xFF` é refeita. O valor é o byte alto do word.
- **O `led ram` não lê nada.** O teste de controle (2026-11-01, `docs/hardware.md`) mostrou que, sem o halo, o rainbow fica estável e o índice fica em `0x01`/`0x00` por 5 minutos; o `0x0E` e o azul-claro parado só apareceram durante as séries de leituras do halo, com a animação rodando. O FURY CTRL lê cada pente só na detecção e depois só escreve; o halo faz o mesmo nas escritas. A ausência de um pente aparece na primeira escrita: `ENXIO` nas 30 tentativas do primeiro `0x53` quer dizer que nada foi escrito (código 3).
  - *Custo:* se o primeiro pente aceitar o `0x53` e o segundo falhar, a moldura fica aberta só no primeiro; repetir o comando a refaz inteira.
  - *Alternativa:* ler dentro da moldura, como o OpenRGB. Fica de fora: não se sabe se a moldura protege o pente, e as escritas não precisam de leitura.
- **O probe lê pouco e avisa.** Ele lê `0x0B`, `0x09` e `0x0E`, nessa ordem, uma vez por comando (6 transações com dois pentes, se nenhuma falhar), e a saída e a ajuda pedem para não rodar em sequência.
- **Códigos de saída:**

  | Código | Quando |
  |---|---|
  | `2` | comando inválido (clap) |
  | `3` | sem adaptador, sem `ee1004`, sem pente suportado, só pentes da API v1, sem nó, ou todas as falhas de leitura do controlador foram `ENXIO` |
  | `4` | `EACCES` ao abrir o nó, ou SPD ilegível com `EACCES` |
  | `5` | nó ocupado, endereço preso, falha de leitura que não é `ENXIO`, SPD ilegível sem `EACCES`, ou escrita que falhou nas 30 tentativas |

  São os `EXIT_*` que já existem.
- **Mensagens:**
  - sem permissão, a mensagem diz "precisa de root: rode com sudo", explica por que o nó é restrito e não cita o grupo `i2c`;
  - na falha da moldura, ela diz o endereço e o registrador, que a moldura ficou incompleta e que repetir o comando a refaz.
- **`sudo`:** os comandos não leem nem gravam nada em `$HOME` ou `$XDG_*`.

## Risks / Trade-offs

- [O slot do controlador pode não ser o índice do SPD] → O `j_index` é o índice do laço do `CheckDramSupport` sobre o SPD, e o probe só lê. Se `0x5A`/`0x5B` não responderem, a change para antes de qualquer escrita.
- [Outro aparelho responde em `0x58`–`0x5B`] → Essa faixa não aparece na `GSAT` do firmware (VRM e DACs), e o halo só endereça os slots com SPD Kingston suportado. O probe mostra índice, estilo e velocidade. Valores fora do esperado são registrados antes de qualquer escrita.
- [Moldura incompleta (processo morto ou falha depois do `0x53`)] → O efeito é desconhecido. A mensagem manda repetir o comando, que refaz a moldura inteira. Não se envia o `0x44` depois de uma falha, porque o FURY CTRL também não faz isso.
- [Firmware ou kernel usando o barramento ao mesmo tempo] → Pelo estudo do SMM e do AML (`docs/hardware.md`), o firmware só usa o SMBus em Sx, reset e botão de energia. O kernel serializa com o `ee1004` pelo lock do adaptador.
- [O `on` (brilho 80) não ser o brilho de hoje] → O ensaio compara visualmente. Se ficar diferente, o `default` é o caminho documentado de volta, e o brilho certo fica para outra change.
- [O modo pode não persistir depois de desligar] → O ensaio registra o reboot e o corte de energia.
- [Rodar o binário inteiro como root] → Aceito para o ensaio. O uso diário pede um helper mínimo, numa change própria.

## Migration Plan

Não há migração: são comandos novos, sem estado. Para desfazer o efeito nos pentes, `sudo halo led ram on` ou `sudo halo led ram default`. Para desfazer o código, basta tirar os subcomandos.

## Open Questions

- Por que o `0x0B` passa a `0x0E` uns 30 s depois do boot, e se isso tem relação com o efeito ter ficado azul-claro estático depois das leituras. O teste de controle (ligar com corte de energia e não rodar nada do halo) responde a segunda parte; se as leituras do halo forem a causa, a change para de novo antes de qualquer código.
- O significado da velocidade (`0x0E`), mostrada em hexadecimal.
- Se o efeito de hoje é o estilo 1 (rainbow) ou o 4 (dinâmico). O probe responde, e isso não muda esta change.
