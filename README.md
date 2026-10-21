# halo

Controle de RGB nativo para Linux, feito para o hardware deste PC. A ideia é cobrir o que o RGB Fusion 2.0 e o GIGABYTE Control Center fazem no Windows, com uma interface própria e minimalista: um ícone na barra superior do GNOME que abre um painel com cores, zonas, efeitos e perfis.

Não é um clone da interface da Gigabyte nem uma camada sobre o OpenRGB. O halo fala direto com cada controlador. O código do OpenRGB serve só como referência de protocolo.

## Estado

Fase de análise concluída em 2026-10-20. Já existem ferramentas que só leem, sem alterar nada, e o controle de LED do Waterforce (ver abaixo):

```sh
cargo run -q -p halo-cli -- probe it5702 --raw          # placa-mãe
cargo run -q -p halo-cli -- telemetry waterforce         # fan, bomba e líquido
cargo run -q -p halo-cli -- probe waterforce --raw      # leituras do Waterforce
```

O probe do Waterforce precisa da regra udev do halo (ver [docs/permissions.md](docs/permissions.md)). Ele envia só os pedidos de leitura `D6`, `DE 00`, `DD`, `D9 01`, `D9 02`, `C7` e `E8` (ver [docs/protocols/castor3.md](docs/protocols/castor3.md)). Exemplo de saída, com valores ilustrativos:

```text
Dispositivo:        /dev/hidraw2
Nome HID:           ITE Tech. Castor3
Firmware (D6):       1.6
Modelo (DE):         Waterforce X 240 [0]
Modos (DD):          fan Default [2], bomba Balanced [3]
Curva da fan (D9):   0 °C 1200 RPM, 30 °C 1200 RPM, 50 °C 1400 RPM, 65 °C 1800 RPM
Curva da bomba (D9): sem resposta em 2 s
Fans RGB (C7):       2
LCD (E8):            modo 7
```

O número entre colchetes é o byte bruto. O modo 3 (FixedRPM) aparece como Balanced, como no GCC. Com `--raw`, a saída mostra também os primeiros 64 bytes de cada resposta, com o pedido no título (`Resposta D9 01:`). Se algum pedido ficar sem resposta, o comando termina com código 5.

### LED do Waterforce

`halo led waterforce <efeito>` aplica cor e efeito ao anel e às fans RGB do Waterforce, com os comandos da `GcLedLib.dll` do GIGABYTE Control Center (ver [docs/protocols/castor3.md](docs/protocols/castor3.md#anel-e-rgb-das-fans)). Antes de escrever, ele confere o modelo com `DE 00`. Cada execução manda uma sequência só, terminada por um único `B6`.

```sh
halo led waterforce static --color ff6600                    # cor estática
halo led waterforce cycle                                     # ciclo de cores
halo led waterforce tri-color --color ff0000 --color 00ff00 --color 0000ff
halo led waterforce pulse --color 00ffff --brightness 5 --speed 2
halo led waterforce off                                       # desliga o anel e as fans
halo led waterforce static --color ff6600 --dry-run           # só mostra os pacotes
```

- Efeitos: `static`, `pulse`, `flash` e `double-flash` (uma cor, padrão `ff6600`), `gradient` (uma cor, padrão `ff0000`), `cycle`, `wave` e `rainbow` (sem cor), `color-shift`, `tri-color`, `spin`, `switch`, `lego` e `aurora` (paleta de até 8 cores, completada com a última), e `off`.
- Sem `--color`, valem as cores padrão do GCC para o efeito.
- `--brightness` vai de 1 a 10 e `--speed` de 1 a 5, com padrão 10 e 5.
- `--dry-run` mostra os pacotes em hexadecimal sem procurar nem abrir o aparelho:

```text
99 C9 01 64 0A 02 01
99 CD FF 66 00
99 B6
```

O protocolo não tem leitura do estado do LED, então o modo de fábrica não pode ser restaurado com exatidão. `cycle` e `rainbow` são os mais próximos. Códigos de saída: 2 para argumento inválido, 3 para aparelho não encontrado, 4 para falta de permissão (instale a regra udev) e 5 para falha de comunicação ou modelo desconhecido.

### Velocidade do Waterforce

`halo speed waterforce` lê e escolhe o modo da fan e da bomba, com os presets do firmware ou uma curva Customized, só com os comandos do GIGABYTE Control Center (`E5`, `E6` e `B6`, ver [docs/protocols/castor3.md](docs/protocols/castor3.md)):

```sh
halo speed waterforce status                                  # só leitura: DE 00, DD, D9 01, D9 02
halo speed waterforce set --fan quiet                         # a bomba mantém o modo atual
halo speed waterforce set --fan performance --pump turbo
halo speed waterforce set --fan quiet --pump turbo --dry-run  # só mostra os pacotes
halo speed waterforce restore                                 # volta ao estado de antes da última escrita
halo speed waterforce restore --original                      # volta ao estado de antes da primeira escrita
```

A curva Customized é **plana**: o mesmo RPM em 0, 30, 50 e 80 °C. Ela vale para um lado por vez, e o outro lado mantém o modo atual:

```sh
halo speed waterforce curve fan 1500
halo speed waterforce curve pump 2600
halo speed waterforce curve fan 1500 --dry-run       # só mostra os pacotes
halo speed waterforce curve fan 1500 --no-save       # experimental, sem B6
halo speed waterforce restore                        # volta ao estado de antes da curva
```

- A curva é plana porque o firmware não segue a temperatura do líquido. Ele segue a temperatura da CPU, que no Windows o `AorusLcdService` envia ao aparelho e no Linux ninguém envia. Sem ela, uma curva com degraus ficaria presa num ponto que você não escolheu (no aparelho, a referência se comportou como se estivesse perto de 50 °C com o líquido a 26 °C; veja `docs/hardware.md`). Um RPM fixo não depende da referência.
- Fan de 900 a 2500 RPM, a faixa do GCC. Uma curva plana não sobe com a carga: um RPM baixo pode deixar a CPU mais quente em jogos ou compilações. Para uso geral, um preset (`set --fan balanced`) é mais seguro.
- Bomba de 2600 a 2800 RPM. Essa faixa é do halo: o GCC aplica à bomba a faixa da fan, o que a deixaria mais lenta do que qualquer preset. 2600 é o Balanced da bomba, e 2800 é o maior valor que o GCC grava na curva. Para a rotação máxima, use `set --pump turbo`.
- O RPM é validado antes de procurar o aparelho, também com `--dry-run`. Na simulação, o `E5` do outro lado aparece com `??`, porque o modo dele só é lido na gravação.
- Se o outro lado estiver num modo que o halo não repete (FixedRPM, por exemplo), nada é escrito: escolha antes um preset para ele com o `set`.
- `--no-save` (experimental) envia os `E5` e o `E6` sem o `B6`. A curva vale na hora, mas não se sabe se continua depois de desligar.
- O `restore` volta os modos e reenvia a curva só do lado que estava em Customized no snapshot. Se o lado estava num preset, o slot Customized dele fica com a curva gravada por último, sem estar ativo. Uma curva com degraus guardada num snapshot, como a de fábrica, também é restaurada.

- Fan: `default`, `quiet`, `balanced`, `performance` e `turbo`. Bomba: `balanced` e `turbo`. ZeroRPM, FixedRPM, Customized e Quiet na bomba são recusados.
- O `set` sempre envia os dois `E5`, fan e bomba, como o GCC. O lado não informado repete o modo lido no aparelho.
- O `status` mostra a curva do `D9` como o slot Customized guardado no aparelho. Ela só é a curva ativa quando o modo é Customized. Para os presets, ele mostra a tabela que a interface do GCC exibe, que não é lida do aparelho.
- `--no-save` (experimental) envia os `E5` sem o `B6`. Ainda não se sabe se o modo vale assim.
- `--dry-run` exige `--fan` e `--pump` e não procura o aparelho nem grava snapshot.

Antes de cada `set`, `curve` ou `restore`, o halo grava o estado atual em `$XDG_STATE_HOME/halo/` (sem a variável, `~/.local/state/halo/`): `waterforce-speed.previous` é substituído a cada escrita, e `waterforce-speed.original` é gravado só na primeira e nunca mais alterado. Para começar um original novo, apague esse arquivo. Se a gravação falhar, nada é enviado. Dois `restore` seguidos alternam entre os dois últimos estados. Os códigos de saída são os mesmos do LED. O 2 vale também para uma curva inválida e quando o modo atual do lado não informado é um que o halo não repete.

Precisa do Rust estável (rustup). Licença: GPL-2.0-or-later, porque a lógica de protocolo é portada do OpenRGB.

Veja:

- [docs/hardware.md](docs/hardware.md): inventário do hardware, protocolos e permissões.
- [docs/architecture.md](docs/architecture.md): arquitetura proposta.
- [docs/roadmap.md](docs/roadmap.md): fases e próximos passos.

## Hardware alvo

| Dispositivo | Controlador | Situação |
|---|---|---|
| Placa-mãe Gigabyte B550M AORUS ELITE | ITE IT5702 (`048d:5702`), USB HID | Protocolo conhecido, acesso liberado |
| AORUS Waterforce X 240, telemetria | Driver `gigabyte_waterforce` do kernel (hwmon) | Funciona, só leitura |
| AORUS Waterforce X 240, anel e fans RGB | Castor3 (`1044:7a4d`), USB HID | Funciona: `halo led waterforce`, protocolo do GCC confirmado no aparelho |
| AORUS Waterforce X 240, velocidade da fan e da bomba | Castor3 (`1044:7a4d`), USB HID | Funciona: `halo speed waterforce`, presets do firmware confirmados no aparelho; curva Customized pendente |
| AORUS Waterforce X 240, tela LCD | Castor3 (`1044:7a4d`), USB HID | Protocolo analisado no GCC, implementação pendente |
| Mouse Logitech G502 SE HERO | `046d:c08b`, USB HID | Protocolo conhecido |
| Teclado Redragon edição Horda (só LED vermelho) | Sino Wealth (`258a:013b`), USB HID | Protocolo desconhecido e de alto risco, investigar com REA |
| RAM Kingston Fury DDR4 (`KF3200C16D4/16GX`) | SMBus, endereço `0x58` | Adiado: SMBus bloqueado pelo ACPI |
