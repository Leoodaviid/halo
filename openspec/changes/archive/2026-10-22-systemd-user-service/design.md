# Design

## Context

Ver `proposal.md` (Why) e a spec `user-service`. O que já existe e restringe o desenho:

- `halod` (`crates/halo-daemon`): pede `io.github.leoodaviid.Halo1` sem fila e sem permitir que outro o tome (`bus::claim_name`). Com o nome em uso, termina com código 1 e a mensagem "o halo já está rodando". Lê `XDG_CONFIG_HOME`/`XDG_STATE_HOME` ou `HOME`, escreve o log no stderr e não envia nada ao hardware na inicialização (spec `daemon-service`).
- Binários: `halo` (crate `halo-cli`) e `halod` (crate `halo-daemon`), num workspace com `Cargo.lock`.
- Neste PC (Ubuntu, GNOME, systemd 259), o D-Bus da sessão é o `dbus.service` do systemd do usuário, e o `graphical-session.target` fica ativo durante a sessão do GNOME.
- A regra udev está em `packaging/udev/70-halo.rules` e é instalada à mão com os comandos do `docs/permissions.md`.
- As sessões do Claude rodam com `HOME` e `XDG_*` do perfil (`/home/bumblebee/.claude-profiles/ld`). Rodar o script daqui instalaria no perfil, e não no usuário real.
- Não há `shellcheck` instalado.

## Goals / Non-Goals

**Goals:**
- Um comando para instalar ou atualizar, e um para remover, que só escrevem no `$HOME`.
- Um script que dá para testar inteiro num diretório temporário, sem systemd nem cargo de verdade.

**Non-Goals:**
- Endurecer a unidade com sandbox do systemd (`PrivateDevices`, `ProtectHome` e afins). O daemon precisa dos hidraw e do `$HOME`, e o ganho não compensa o risco de quebrar o acesso agora.
- Pacotes `.deb`, `cargo install` ou instalação para todos os usuários.

## Decisions

### 1. Shell POSIX em `packaging/install.sh`

Um script `sh` (`set -eu`), sem bash nem dependências além de `cargo`, `systemctl`, `install`, `cmp` e `sed`. Ele é executado a partir de qualquer diretório e acha a raiz do repositório pelo próprio caminho.

```text
install.sh [--uninstall] [--help]

instalar                                  --uninstall
--------                                  -----------
1. cargo build --release --locked         1. systemctl --user disable --now halod.service
   -p halo-cli -p halo-daemon                (ignora se a unidade não existe)
2. gera a unidade e a ativação com o      2. remove binários, unidade e ativação
   caminho real do BINDIR                 3. systemctl --user daemon-reload
3. instala binários, unidade, ativação    4. informa onde ficaram perfis e snapshots
4. systemctl --user daemon-reload
5. systemctl --user enable halod.service
6. systemctl --user restart halod.service
7. confere a regra udev
```

Alternativas: `cargo xtask install` (Rust para copiar três arquivos, e a extensão não é Rust) e uma change própria para o script (a unidade seria testada com comandos à mão e reescrita depois). As duas foram descartadas no explore.

### 2. Caminhos configuráveis, só para os testes

| Variável | Padrão | Uso |
|---|---|---|
| `BINDIR` | `$HOME/.local/bin` | binários |
| `XDG_CONFIG_HOME` | `$HOME/.config` | `systemd/user/halod.service` |
| `XDG_DATA_HOME` | `$HOME/.local/share` | `dbus-1/services/io.github.leoodaviid.Halo1.service` |
| `CARGO_TARGET_DIR` | `<repo>/target` | onde o cargo deixa os binários |
| `HALO_UDEV_RULES_DIR` | `/etc/udev/rules.d` | só leitura, para a conferência |

O systemd do usuário procura unidades em `$XDG_CONFIG_HOME/systemd/user`, e o `dbus-daemon` procura ativações em `$XDG_DATA_HOME/dbus-1/services`. Então os padrões respeitam a especificação XDG, e um teste aponta tudo para um diretório temporário. Os perfis e os snapshots não são do script: ele só cita `$XDG_CONFIG_HOME/halo` e `$XDG_STATE_HOME/halo` na mensagem final da desinstalação.

### 3. Unidade e ativação geradas a partir de modelos

`packaging/systemd/halod.service` e `packaging/dbus/io.github.leoodaviid.Halo1.service` ficam no repositório com `@BINDIR@` no lugar do caminho, e o script troca com `sed`. Usar `%h/.local/bin` direto na unidade quebraria um `BINDIR` diferente, e o arquivo de ativação do D-Bus não tem especificador de home.

```ini
# halod.service
[Unit]
Description=halo: daemon de RGB, velocidade e telemetria
Documentation=https://github.com/Leoodaviid/halo
PartOf=graphical-session.target
After=graphical-session.target

[Service]
Type=dbus
BusName=io.github.leoodaviid.Halo1
ExecStart=@BINDIR@/halod
Restart=on-failure
RestartSec=2

[Install]
WantedBy=graphical-session.target
```

```ini
# io.github.leoodaviid.Halo1.service
[D-BUS Service]
Name=io.github.leoodaviid.Halo1
Exec=@BINDIR@/halod
SystemdService=halod.service
```

- `Type=dbus` com `BusName`: o systemd só considera a unidade pronta quando o nome aparece, o que o `halod` faz depois de publicar todos os objetos.
- `Restart=on-failure` e `RestartSec=2`, com o limite padrão do systemd (5 inícios em 10 s): com o nome tomado por um `halod` iniciado à mão, a unidade tenta algumas vezes e desiste, sem laço.
- `PartOf`/`WantedBy=graphical-session.target`: o daemon sobe e desce com a sessão do GNOME, o único cliente. Num login só por SSH, ele não sobe, mas a ativação pelo D-Bus ainda o sobe se alguém chamar.
- `SystemdService=`: o `dbus-daemon` pede a unidade ao systemd em vez de executar o `Exec=`. O `Exec=` só é obrigatório pelo formato do arquivo.

Alternativa: `WantedBy=default.target`, que subiria o daemon em qualquer login. Foi descartada no explore.

### 4. `restart` no lugar de `try-restart`

No explore ficou `try-restart` para trocar o binário em uso numa atualização. O `restart` faz o mesmo e também inicia o daemon na primeira instalação, que é o que o usuário espera de "instalar". Iniciar o daemon não envia nada ao hardware (`daemon-service`).

### 5. Compilar antes de tocar no que está instalado

O `cargo build` roda primeiro. Se falhar, o `set -e` encerra o script antes de qualquer cópia, e a instalação anterior continua inteira. Os binários são copiados com `install -m 755`, que cria um arquivo novo no lugar: o `halod` em execução continua com o binário antigo até o `restart`.

### 6. Conferência da regra udev só por leitura

`cmp -s` entre `packaging/udev/70-halo.rules` e `$HALO_UDEV_RULES_DIR/70-halo.rules`. Três saídas: igual (nada a mostrar), ausente ou diferente (aviso com os três comandos do `docs/permissions.md`). O script nunca chama `sudo` nem `udevadm`, e a instalação do usuário termina com sucesso nos três casos.

### 7. Teste com `cargo` e `systemctl` falsos

`packaging/tests/install-test.sh` monta um diretório temporário, põe no `PATH` um `cargo` falso (que cria `halo` e `halod` de mentira em `CARGO_TARGET_DIR/release`) e um `systemctl` falso (que só registra os argumentos), aponta as variáveis da decisão 2 para o temporário e confere:

- os arquivos instalados, com o `@BINDIR@` trocado;
- a ordem das chamadas ao `systemctl`;
- a reinstalação, que substitui os arquivos;
- a desinstalação, que remove só o que foi instalado e mantém um `halo/` de perfis e snapshots;
- a falha de compilação, que não troca nada;
- as três saídas da regra udev;
- que nada foi escrito fora do temporário.

A unidade gerada também passa por `systemd-analyze --user verify`, que só lê o arquivo.

O teste é um shell à parte, e não um teste do cargo, porque o script não é Rust e o teste não precisa compilar nada. Ele roda com `sh packaging/tests/install-test.sh`.

### 8. Primeira leitura do sysfs antes de pegar o nome

Achado na conferência no PC: com o daemon parado, a chamada a `Manager.Devices` que o subiu pela ativação recebeu `aa{sv} 0`. O `halod` pegava o nome no D-Bus antes da primeira volta dos ticks, e o `dbus-daemon` entrega a chamada guardada assim que o nome aparece. A corrida já existia, mas a ativação a torna sistemática, e a telemetria tinha o mesmo problema (`Available` falso até o primeiro tick).

O `halod` passa a ler o sysfs uma vez (presença, nomes e hwmon, as mesmas leituras do tick) antes de publicar os objetos e pedir o nome, e o `Manager` e o `Telemetry` nascem com esse estado. O primeiro tick lê o mesmo e não emite sinal. Nenhum hidraw é aberto.

Alternativas: só a lista (a telemetria continuaria começando indisponível) e deixar o cliente esperar o `DevicesChanged` (o painel mostraria "nenhum aparelho" a cada subida). O usuário escolheu a lista e a telemetria.

## Risks / Trade-offs

- [Instalar pela sessão do Claude cai no perfil, e não no usuário real] → A task de conferência no PC é do usuário, no terminal dele, ou com `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME` e `XDG_STATE_HOME` apontando para `/home/bumblebee` e o aval dele. O script mostra no início para onde vai instalar.
- [`~/.local/bin` fora do `PATH`] → A unidade usa o caminho absoluto. O script avisa se o `BINDIR` não estiver no `PATH`, para o `halo` da linha de comando.
- [`halod` iniciado à mão ao mesmo tempo que a unidade] → A unidade falha e desiste (cenário "Outra instância com o nome"). O README explica parar a unidade antes de rodar o daemon à mão.
- [`PartOf=graphical-session.target` para o daemon no logout] → Esperado: sem sessão gráfica, não há painel. Uma chamada D-Bus depois disso o sobe de novo.
- [O anel pode perder o LED ao desligar, e nada reaplica] → É um risco aceito no explore. O teste é rápido (um `static` com `B6`, corte de energia, olhar o anel) e, se mostrar perda, a reaplicação vira uma change própria.
- [Sem `shellcheck`] → O teste cobre o comportamento. Instalar o `shellcheck` fica a critério do usuário.

## Migration Plan

1. Instalar: `packaging/install.sh` no terminal do usuário. Se houver um `halod` rodando à mão, pará-lo antes.
2. Voltar atrás: `packaging/install.sh --uninstall`. Os perfis e os snapshots ficam.
3. Nada muda no formato dos perfis, no D-Bus nem na regra udev.
