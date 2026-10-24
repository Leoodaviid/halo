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

A telemetria lê o hwmon do driver `gigabyte_waterforce`, sem abrir o aparelho. Uma temperatura do líquido acima de 100 °C aparece como `Líquido:  inválido (o aparelho informou 224 °C)`, e uma nota no stderr explica o caso. O Castor3 já informou valores assim, e eles só voltaram ao normal depois de desligar o PC com corte de energia (ver "Bomba plana e temperatura errada no `DA`" em [docs/hardware.md](docs/hardware.md)). Fan e bomba continuam sendo mostradas, e o código de saída continua 0.

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

O protocolo não tem leitura do estado do LED, então o modo de fábrica não pode ser restaurado com exatidão. `cycle` e `rainbow` são os mais próximos. Códigos de saída: 2 para argumento inválido, 3 para aparelho não encontrado, 4 para falta de permissão (instale a regra udev) e 5 para falha de comunicação, modelo desconhecido ou aparelho ocupado.

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

### Daemon (`halod`)

O `halod` é o daemon do usuário: ele publica no D-Bus da sessão o LED, a velocidade e a telemetria do Waterforce e os perfis salvos, para a extensão do GNOME (ver "Extensão do GNOME"). Ele só envia comandos ao aparelho quando um cliente pede, e as sequências são as mesmas da CLI, com os mesmos snapshots. Sozinho, ele só lê o sysfs a cada 2 s (presença dos aparelhos e telemetria do hwmon). Ele não reaplica o perfil no login nem depois da suspensão: o Waterforce guarda a velocidade gravada com `B6` mesmo sem energia (ver [docs/hardware.md](docs/hardware.md)), e o GCC também não reenvia nada no boot. Reaplicar a cada login seria um `B6` a mais, sem ganho.

#### Instalação

```sh
packaging/install.sh                   # compila, instala e sobe o halod e a extensão; também atualiza
packaging/install.sh --uninstall       # remove; perfis e snapshots ficam
```

Tudo vai para o seu usuário, sem sudo:

| O quê | Onde |
|---|---|
| `halo` e `halod` | `~/.local/bin` (ou `$BINDIR`) |
| Unidade `halod.service` do systemd do usuário | `~/.config/systemd/user/` |
| Ativação pelo D-Bus de `io.github.leoodaviid.Halo1` | `~/.local/share/dbus-1/services/` |
| Extensão do GNOME Shell (`halo@leoodaviid.github.io`) | `~/.local/share/gnome-shell/extensions/halo@leoodaviid.github.io/` |

O script compila com `cargo build --release --locked` antes de tocar no que já está instalado: se a compilação falhar, nada muda. Depois ele habilita a unidade, que sobe com a sessão gráfica, e (re)inicia o daemon, para que uma atualização passe a valer na hora. Com a unidade parada ou desabilitada, qualquer chamada ao nome no D-Bus sobe o daemon de novo. A extensão é copiada sem a pasta `tests/` e habilitada pelo `gsettings` (`org.gnome.shell enabled-extensions`), e a pasta só é trocada quando o conteúdo mudou. Depois de instalar ou atualizar a extensão, **saia e entre na sessão**: no Wayland o Shell não recarrega o código de uma extensão. O script só pede isso quando a extensão mudou. A desinstalação desabilita e remove a extensão e mantém os perfis (`~/.config/halo`) e os snapshots de velocidade (`~/.local/state/halo`), inclusive o `waterforce-speed.original`.

A regra udev não é instalada pelo script, porque precisa de sudo: ele só confere se ela está em `/etc/udev/rules.d/` e igual à do repositório e, se não estiver, mostra os comandos (ver [docs/permissions.md](docs/permissions.md)).

```sh
systemctl --user status halod.service  # estado do daemon
journalctl --user -u halod.service     # registro
```

Para rodar o daemon à mão, a partir do código, pare a unidade antes: com o nome já tomado, um segundo `halod` recusa e sai.

```sh
systemctl --user stop halod.service
cargo run -q -p halo-daemon            # fica rodando; uma segunda instância recusa e sai
```

Exemplos com o `busctl` (o nome do bus é `io.github.leoodaviid.Halo1`):

```sh
N=io.github.leoodaviid.Halo1; R=/io/github/leoodaviid/Halo1; W=$R/waterforce

busctl --user call $N $R $N.Manager Devices                            # todos os aparelhos, presentes ou não
busctl --user call $N $W $N.Waterforce.Led Apply sasyy static 1 ff6600 10 0
busctl --user call $N $W $N.Waterforce.Led Apply sasyy cycle 0 0 0     # 0 = padrão do efeito
busctl --user call $N $W $N.Waterforce.Speed Status                    # só leitura
busctl --user call $N $W $N.Waterforce.Speed SetPresets ss quiet ""    # "" mantém a bomba
busctl --user call $N $W $N.Waterforce.Speed SetFlatCurve sq fan 1500
busctl --user call $N $W $N.Waterforce.Speed Restore s previous
busctl --user get-property $N $W $N.Waterforce.Telemetry FanRpm CoolantMilliCelsius Available
```

- O `Devices` devolve um dicionário por aparelho (`aa{sv}`), sempre na ordem `waterforce`, `it5702`, `g502`, com as chaves `id`, `name`, `present`, `devnode` (vazio quando ausente) e `capabilities` (as interfaces que o daemon expõe para o aparelho: hoje só o Waterforce tem `led`, `speed` e `telemetry`). A ordem das chaves na saída do `busctl` varia. Um trecho, com o Waterforce em `/dev/hidraw3`:

  ```text
  aa{sv} 3 5 "id" s "waterforce" "name" s "AORUS Waterforce X" "present" b true "devnode" s "/dev/hidraw3" "capabilities" as 3 "led" "speed" "telemetry" 5 "id" s "it5702" "name" s "B550M AORUS ELITE" ...
  ```

- A presença vem só do sysfs: o daemon não abre a placa-mãe nem o mouse. O nome da placa é o `board_name` do DMI, e o do Waterforce é `AORUS Waterforce X` até a primeira operação no aparelho (LED, velocidade ou perfil), que já lê o modelo com o `DE 00`; depois vira o modelo completo, como `AORUS Waterforce X 240`, com um `DevicesChanged`. O `DevicesChanged` sai quando a presença, o caminho ou o nome de um aparelho muda.
- O `Apply` recebe o efeito, as cores (de 0 a 8, `rrggbb`), o brilho e a velocidade. Brilho ou velocidade 0 valem o padrão do efeito, e o `off` só aceita brilho 0.
- As escritas pelo D-Bus sempre terminam com `B6`. O `--no-save` e o `--dry-run` existem só na CLI.
- A propriedade `LastApplied` da interface `Led` é o último LED aplicado **pelo daemon**, e não o estado do aparelho, que não tem leitura do LED.
- Os erros têm nome sob `io.github.leoodaviid.Halo1.Error`: `InvalidArgument`, `NotFound`, `PermissionDenied`, `Busy`, `Device`, `ProfileNotFound` e `Storage`.

#### Perfis

Cada perfil é um arquivo em `$XDG_CONFIG_HOME/halo/profiles/<nome>.toml` (sem a variável, `~/.config/halo/profiles/`), com seções opcionais:

```toml
[waterforce.led]
effect = "static"
colors = ["ff6600"]
brightness = 10      # opcional, de 1 a 10
speed = 5            # opcional, de 1 a 5

[waterforce.speed]
fan = "quiet"        # um preset ou um RPM de curva plana: fan = 1500
pump = "turbo"       # o lado ausente mantém o modo atual
```

O nome tem de 1 a 32 letras minúsculas, dígitos e hífen. As regras são as da CLI, e um perfil inválido é recusado sem alterar o arquivo. Pelo D-Bus:

```sh
busctl --user call $N $R $N.Profiles Save 'sa{sv}' noite 2 \
  waterforce.led 'a{sv}' 2 effect s static colors as 1 ff6600 \
  waterforce.speed 'a{sv}' 2 fan s quiet pump s turbo
busctl --user call $N $R $N.Profiles List
busctl --user call $N $R $N.Profiles Get s noite
busctl --user call $N $R $N.Profiles Activate s noite                  # devolve as seções aplicadas
busctl --user get-property $N $R $N.Profiles ActiveProfile
busctl --user call $N $R $N.Profiles Delete s noite
```

O `Activate` aplica a velocidade antes do LED. A velocidade é lida antes (`DE 00`, `DD`, `D9 01`, `D9 02`) e só é gravada, com `B6`, se for diferente do perfil, para não gravar a flash à toa. O LED é sempre aplicado, porque não tem leitura. Se uma seção falhar, as outras são tentadas mesmo assim, o erro diz quais foram aplicadas e o perfil ativo não muda. O nome do ativo fica em `$XDG_STATE_HOME/halo/active-profile`.

### Extensão do GNOME

A extensão (`extension/`, UUID `halo@leoodaviid.github.io`, GNOME Shell 50) é um cliente do `halod`: ela só fala com o D-Bus da sessão e não abre hidraw nem lê o sysfs. Esta versão faz o mínimo:

- Na barra, o ícone do halo e a temperatura do líquido em graus inteiros (`38°`). Com a temperatura inválida (`CoolantValid` falso), aparece `--`, e sem leitura (`Available` falso) aparece só o ícone.
- Com o `halod` fora do D-Bus, o ícone fica esmaecido.
- Ao clicar, o painel mostra um carrossel com os aparelhos que o `halod` encontrou: o desenho, o nome e o status ("Disponível" com controle de LED, "Detectado" sem), com setas, indicador de página, roda do mouse e as teclas Esquerda e Direita. O anel do Waterforce aparece na cor do último LED que o `halod` aplicou (cinza enquanto ele não aplicou nada). No Waterforce, o painel controla o LED: efeito (os nove de uma cor), cor (cores prontas, matiz e hex), brilho de 1 a 10 e velocidade de 1 a 5, e o liga/desliga do cabeçalho. Cada mudança vira um único `Led.Apply` quando o gesto termina (ao soltar o slider, escolher a cor ou o efeito, ou Enter no hex), nunca durante um arraste, porque cada aplicação grava no aparelho (`99 B6`). A roda do mouse não muda os sliders. Religar volta ao último estado ligado, depois ao LED do perfil ativo e, por fim, ao `cycle`. Os efeitos de paleta aplicados por um perfil aparecem só para leitura. Os aparelhos sem controle mostram o aviso de controle em desenvolvimento. Com o daemon fora do ar, mostra o aviso e as ações "Iniciar halod" e "Ver registro" (as últimas linhas do `journalctl --user -u halod.service`), desabilitadas se a unidade não estiver instalada.

A extensão não sobe o daemon sozinha: ela só observa o nome no D-Bus, e iniciar é sempre um gesto seu. Um `systemctl --user stop halod.service` fica respeitado. O resto do painel (perfis, detalhes, mensagens de erro, preferências) vem na próxima change (ver [docs/roadmap.md](docs/roadmap.md)).

Os erros da extensão aparecem no registro do Shell, que no Ubuntu fica no journal do usuário. Este comando devolve as linhas do `gnome-shell` (testado aqui, no GNOME Shell 50 com Wayland):

```sh
journalctl --user -b /usr/bin/gnome-shell --no-pager | grep -iE 'halo|JS ERROR'
```

O filtro `-u org.gnome.Shell@wayland.service` não serve: ele devolve `-- No entries --`, porque o Shell da sessão não roda nessa unidade. Um erro em `enable()` desativa só a extensão.

### Desenvolvimento da extensão

```sh
sh extension/tests/run.sh                 # lógica pura (barra e painel), com o gjs, sem o Shell
sh scripts/dev/headless-smoke.sh          # precisa de: cargo build -p halo-daemon
sh scripts/dev/nested-shell.sh [--daemon] [--profile]  # precisa do mutter-dev-bin
sh scripts/dev/watch-led-applies.sh [--address <endereço>]  # conta os Led.Apply e os B6
```

- `run.sh` roda os testes de `extension/lib/` com `gjs -m` e termina com código diferente de 0 se algum falhar.
- `headless-smoke.sh` carrega a extensão num GNOME Shell headless e confere que ela fica `ACTIVE`, que sobrevive a um ciclo `disable`/`enable` e que não deixa `JS ERROR` no log. Ele usa um `halod` de teste, com um sysfs falso, e confere que a extensão não ativa o `halod`.
- `nested-shell.sh` abre um Shell aninhado numa janela, com a extensão do repositório, para ver e clicar no painel. Sem `--daemon`, o painel mostra o daemon fora do ar. Com `--daemon`, sobe um `halod` de teste com o sysfs falso de `scripts/dev/fake-sysfs.sh`: o carrossel mostra só o Waterforce, e todo `Led.Apply` falha com `NotFound`, porque o nó (`hidraw99`) não existe. Nenhum modo de teste lê o `/sys` real. `--profile` grava um perfil `teste` com o LED tri-color e o marca como ativo, para testar o religar sem estado conhecido. Ao fechar a janela, o script mostra os `Led.Apply` que passaram pelo barramento do teste, com os argumentos. Um argumento inválido termina com código 2.

- `watch-led-applies.sh` só observa o D-Bus (com o `dbus-monitor`, sem root e sem chamar nenhum método do `halod`): mostra cada `Led.Apply` com os argumentos, cada sucesso e cada erro, e no Ctrl+C os totais. Cada `Apply` que dá certo muda o `LastApplied` e manda ao Waterforce um único `99 B6`, que grava no aparelho; um erro não manda nenhum, salvo um `Device` no meio da sequência, marcado como incerto. Sem `--address`, observa o D-Bus da sessão; o `nested-shell.sh` mostra o comando com o endereço do barramento do teste.

Os dois scripts de Shell rodam isolados, com `HOME`, `XDG_*` e `XDG_RUNTIME_DIR` próprios e um D-Bus próprio: um Shell de teste com o runtime real cria `/run/user/<uid>/gnome-shell-disable-extensions`, e a sessão desliga todas as extensões no próximo início. Ao terminar, eles conferem que esse marcador não apareceu.

### Um processo por aparelho

Só um processo do halo por vez conversa com cada aparelho. Ao abrir o hidraw, o halo toma um `flock` exclusivo no nó e espera até 5 s se outro processo do halo (a CLI ou o daemon) estiver com ele. Depois disso, desiste sem enviar nada e termina com código 5: `halo: /dev/hidraw2 ocupado por outro processo do halo`. A telemetria e o `--dry-run` não abrem o hidraw e não esperam.

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
