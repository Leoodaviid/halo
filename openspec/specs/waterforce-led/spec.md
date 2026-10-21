# waterforce-led Specification

## Purpose

Aplicar cor e efeito ao anel e às fans RGB do AORUS Waterforce X (`1044:7a4d`) pelo hidraw, usando só os comandos e o layout de bytes confirmados no GIGABYTE Control Center.

## Requirements

### Requirement: Comandos permitidos
O comando de LED SHALL enviar ao Castor3 apenas `DE 00` (leitura do modelo), `C9`, `CD`, `B0`, `B1`, `B2`, `B3` e `B6`, cada um como output report `0x99` completado com zeros até 6144 bytes (fonte: `GcLedLib.dll` do GCC 26.09.10.01, ver `docs/protocols/castor3.md`). Ele MUST NOT enviar comandos de LCD, de fan, de bomba nem `C8`.

#### Scenario: Cor estática
- **WHEN** o usuário aplica o efeito estático com a cor `ff6600`, brilho 10
- **THEN** o sistema envia, nesta ordem, `99 DE 00`, `99 C9 01 64 0A 02 01`, `99 CD FF 66 00` e `99 B6`, e nada mais

### Requirement: Modelo conferido antes de escrever
O sistema SHALL enviar `DE 00` e só prosseguir se a resposta chegar em 2 segundos com `r[2]` igual a 0, 1 ou 2 (Waterforce X 240, 280 ou 360). Caso contrário, MUST NOT enviar nenhum comando de LED.

#### Scenario: Modelo reconhecido
- **WHEN** a resposta ao `DE 00` é `99 DE 00 ...`
- **THEN** o sistema envia a sequência do efeito

#### Scenario: Modelo não reconhecido
- **WHEN** a resposta ao `DE 00` tem `r[2]` igual a 5
- **THEN** o sistema não envia `C9`, `CD`, `B0`–`B3` nem `B6`, informa o valor lido e termina com código diferente de zero

#### Scenario: Sem resposta ao DE
- **WHEN** nenhuma resposta `99 DE` chega em 2 segundos
- **THEN** o sistema não envia nenhum comando de LED e termina com código diferente de zero

### Requirement: Layout do C9
O pacote de efeito SHALL ser `99 C9 <efeito> <brilho × 10> <velocidade × 2> <índice> <área>`, com o brilho no byte 2 e a velocidade no byte 3, como na `GcLedLib.dll`, e não como no OpenRGB. O índice é `02` e a área é `01`, exceto onde a sequência do efeito indicar outro valor.

#### Scenario: Brilho e velocidade separados
- **WHEN** o usuário aplica o efeito pulso com brilho 3 e velocidade 2
- **THEN** o pacote de efeito é `99 C9 02 1E 04 02 01`

### Requirement: Efeitos e sequências
O sistema SHALL oferecer os efeitos da interface do GCC para o X 240 e enviar, para cada um, a sequência da `GcLedLib.dll`, sempre terminando com um único `B6`.

#### Scenario: Efeitos de cor única
- **WHEN** o efeito é estático (01), pulso (02), flash (04) ou flash duplo (05)
- **THEN** a sequência é `C9`, `CD <cor>`, `B6`

#### Scenario: Efeitos sem cor
- **WHEN** o efeito é ciclo (03), onda (08) ou arco-íris (0A)
- **THEN** a sequência é `C9`, `B6`, e o sistema recusa a opção de cor para esses efeitos

#### Scenario: Gradiente
- **WHEN** o efeito é gradiente (06)
- **THEN** a sequência é `C9` com índice `02`, `CD <cor>`, `C9` com índice `0A`, `B6`

#### Scenario: Transição de cores
- **WHEN** o efeito é transição (07)
- **THEN** a sequência é `C9` com índice `08` e área `02`, `B0`–`B3` com a paleta, `B6`

#### Scenario: Efeitos de paleta
- **WHEN** o efeito é três cores (0B), giro (0C), alternância (0D), lego (0E) ou aurora (0F)
- **THEN** a sequência é `C9`, `B0`–`B3` com a paleta, `B6`

### Requirement: Paleta
Os pacotes de paleta SHALL ser `99 Bn <efeito> R G B R G B` para n de 0 a 3, com as cores 1 e 2 no `B0`, 3 e 4 no `B1`, 5 e 6 no `B2`, 7 e 8 no `B3`. O sistema SHALL aceitar de 1 a 8 cores e completar as que faltarem repetindo a última. Sem cores informadas, usa as cores padrão do GCC para o efeito.

#### Scenario: Três cores
- **WHEN** o efeito três cores recebe `ff0000`, `00ff00`, `0000ff`
- **THEN** o `B0` é `99 B0 0B FF 00 00 00 FF 00`, e o `B1`, o `B2` e o `B3` são `99 Bn 0B 00 00 FF 00 00 FF`

#### Scenario: Paleta grande demais
- **WHEN** o usuário informa 9 cores
- **THEN** o sistema recusa a entrada e não abre o aparelho

### Requirement: Faixas de brilho e velocidade
O sistema SHALL aceitar brilho de 1 a 10 e velocidade de 1 a 5, como a interface do GCC, com padrão 10 e 5. Um valor fora da faixa MUST ser recusado antes de abrir o aparelho.

#### Scenario: Valores padrão
- **WHEN** o usuário não informa brilho nem velocidade
- **THEN** o `C9` leva `64` (brilho 10) e `0A` (velocidade 5)

#### Scenario: Velocidade inválida
- **WHEN** o usuário informa velocidade 6
- **THEN** o sistema informa a faixa válida e não envia nada

### Requirement: Desligar
O sistema SHALL oferecer um efeito de desligar que envia `C9 01 00 0A 02 01`, `CD 00 00 00` e `B6`, como o GCC faz com o LED desligado (estático com brilho 0).

#### Scenario: Desligar o anel
- **WHEN** o usuário aplica o efeito de desligar
- **THEN** o sistema envia `99 DE 00`, `99 C9 01 00 0A 02 01`, `99 CD 00 00 00` e `99 B6`

### Requirement: Aplicação única
Cada execução do comando SHALL enviar uma única sequência de efeito, com um único `B6` no fim, e terminar. O sistema MUST NOT oferecer repetição, animação feita pelo software ou reenvio periódico.

#### Scenario: Uma execução
- **WHEN** o usuário aplica qualquer efeito
- **THEN** o aparelho recebe exatamente um `B6`, e o processo termina

### Requirement: Simulação sem aparelho
O sistema SHALL oferecer uma opção de simulação que mostra em hexadecimal os pacotes da sequência, sem o `DE 00`, e não abre nem procura o hidraw.

#### Scenario: Simulação
- **WHEN** o usuário aplica o efeito estático `ff6600` com a opção de simulação
- **THEN** a saída mostra `99 C9 01 64 0A 02 01`, `99 CD FF 66 00` e `99 B6`, e nenhum dispositivo é aberto

### Requirement: Aparelho ausente ou sem permissão
O sistema SHALL informar quando o Waterforce não for encontrado, e, quando o hidraw não abrir por falta de permissão, indicar a regra udev do halo. Nos dois casos termina com código diferente de zero sem enviar nada.

#### Scenario: Sem permissão
- **WHEN** o hidraw do `1044:7a4d` existe, mas o usuário não pode abri-lo
- **THEN** o sistema informa a falta de permissão, o caminho e onde está a regra udev do halo
