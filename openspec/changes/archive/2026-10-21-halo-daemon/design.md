# Design

## Context

O `halo-core` já tem tudo o que o daemon precisa fazer no hardware, como funções síncronas e completas: `led::apply`, `speed::{status, set, curve, restore}` e `telemetry::read`. Cada uma procura o aparelho pelo sysfs, abre o hidraw com `Hidraw::open`, conversa e fecha. Todas recebem `sys_root`, o que já permite testar com um sysfs falso. Por baixo, as variantes `*_to` aceitam qualquer `InputReports + OutputReports`, e o `FakeDevice` de `led::tests` grava os pacotes enviados.

A velocidade já tem o caminho que a ativação de perfil precisa. `write_to` lê o estado, grava o snapshot e envia um `Target`, e `Snapshot::target()` monta a sequência da restauração (`E5` fan, `E5` bomba, `E6` de cada lado em Customized, `B6`), que foi confirmada no aparelho em 2026-10-20.

A motivação e o escopo estão no `proposal.md`, e os requisitos nas specs `device-exclusive-access`, `daemon-service`, `daemon-waterforce` e `profiles`.

## Goals / Non-Goals

**Goals:**
- Um daemon pequeno que só traduz D-Bus para o `halo-core`, sem lógica de protocolo própria.
- A CLI continuar independente e útil sem o daemon, para depurar o hardware.
- Deixar o laço periódico e o resume prontos para o `waterforce-cpu-feed` e o `systemd-user-service` só acrescentarem um produtor ou uma política.

**Non-Goals:**
- Async no `halo-core`. O core continua síncrono, e o daemon isola as chamadas bloqueantes.
- Estabilidade da interface D-Bus para terceiros. O `1` no nome marca a primeira versão, e ela pode mudar até a extensão GNOME existir.
- Detecção de hotplug por eventos do udev. Verificar o sysfs a cada 2 s basta para a lista de aparelhos.

## Decisions

### 1. `flock` no próprio nó do hidraw, dentro de `Hidraw::open`

`Hidraw::open` passa a chamar `flock(fd, LOCK_EX | LOCK_NB)` logo depois de abrir, e tenta de novo a cada 50 ms até 5 s. Se não conseguir, devolve um novo `OpenError::Busy(PathBuf)`, no estilo das variantes existentes. O lock é solto quando o `File` fecha, inclusive se o processo morrer. Os erros `speed::SpeedError::Open`, `led::ApplyError::Open` e os do probe já carregam o `OpenError`, então o `Busy` chega à CLI e ao daemon sem tipos novos em cada módulo.

- **Por que no nó do aparelho:** dois processos que abrem o mesmo `/dev/hidrawN` usam o mesmo inode, então o lock vale entre eles sem arquivo extra e sem depender de `$XDG_RUNTIME_DIR`. Um `hidrawN` que muda de número entre boots não importa, porque o lock é tomado depois da descoberta.
- **Por que em `Hidraw::open`:** é o único ponto de abertura do core. Probe, LED, speed e o probe do IT5702 ficam cobertos sem mudar cada um.
- **Por que 5 s:** o pior caso de uma escrita de velocidade é de 4 leituras com tempo limite de 2 s cada, mas na prática ela leva menos de 1 s. Com 5 s, a CLI espera uma operação normal do daemon e não fica presa atrás de um aparelho travado.
- **Alternativas:** um arquivo de lock em `$XDG_RUNTIME_DIR/halo/` (precisa de um nome por aparelho e não protege contra uma CLI de outro usuário); a CLI passar sempre pelo D-Bus (a CLI passaria a depender do daemon, e isso pode vir depois sem mudar nada aqui); não fazer nada (o problema do `proposal.md`).
- O lock é advisory: só protege entre processos do halo. Isso basta, porque o kernel (`gigabyte_waterforce`) não usa o hidraw e o OpenRGB não roda nesta máquina.

### 2. zbus 5 com o executor próprio dele, sem tokio

O `halod` usa o `zbus` 5 com a feature padrão (`async-io`), o `#[interface]` para as interfaces, `#[derive(DBusError)]` com prefixo `io.github.leoodaviid.Halo1.Error` para os erros, e o `zbus::block_on` no `main`. O nome é pedido com `DoNotQueue` e sem `AllowReplacement`. Se a resposta não for `PrimaryOwner`, o processo sai com código 1 (spec "Instância única").

- **Alternativas:** `dbus-rs` (precisa da libdbus em C e tem API mais baixa); `zbus` com tokio (traz um runtime grande só para um timer e alguns canais).
- O nome segue o domínio reverso do repositório (`github.com/Leoodaviid/halo` → `io.github.leoodaviid`), em minúsculas porque nomes de bus aceitam maiúsculas, mas a convenção do GNOME para `io.github.*` é minúscula no usuário.

### 3. Um worker por aparelho, com fila

Cada aparelho que abre hidraw (nesta change só o Waterforce) tem uma thread `std` dona das chamadas ao core. Os métodos D-Bus mandam um `Job` por um canal e esperam a resposta num `async_channel::bounded(1)`. Assim:
- as chamadas ao mesmo aparelho ficam em série e na ordem de chegada (spec "Operações de um aparelho em série"), sem depender do `flock` dentro do próprio processo;
- o executor do zbus nunca bloqueia com os `sleep` de `PACKET_INTERVAL` nem com os 2 s de espera de resposta, e perfis, lista e telemetria continuam respondendo.

O worker fala com o aparelho por um trait `WaterforceBackend` com as operações do core (`apply_led`, `speed_status`, `set_presets`, `set_curve`, `restore`, `apply_speed_state`). A implementação real chama o `halo-core` com o `sys_root` e o `SnapshotStore`. Os testes usam um backend falso que grava as chamadas. Os bytes continuam testados no core, com o `FakeDevice`.

- **Alternativa:** `blocking::unblock` com um `Mutex` por aparelho. Funciona, mas a ordem de chegada não fica garantida e a fila some do desenho, que é justamente onde o `waterforce-cpu-feed` vai precisar intercalar o `E0` com as escritas do usuário.

### 4. Ativação da velocidade para um estado desejado

O perfil vira um `DesiredState` validado: para cada lado, o preset vira o modo, e o RPM vira o modo Customized (`01`) com os pontos de `Curve::flat`. Um novo `speed::apply_state_to(device, store, desired)` faz assim:

1. `read_state` (`DE 00`, `DD`, `D9 01`, `D9 02`), com o modelo conferido.
2. Compara com a regra da spec "Ativação da velocidade só quando difere". Se for igual, devolve "sem mudança", sem snapshot e sem escrita.
3. Se o lado ausente estiver num modo que não pode ser repetido (`!restorable_mode`), recusa com `CurrentModeUnknown`, como o `set`.
4. Senão, grava o snapshot e envia um `Target` com os dois `E5` (o lado ausente repete o modo lido), um `E6` só para cada lado em curva plana e um `B6`. O envio é o mesmo do `write_to`, separado numa função comum.

O lado ausente não reenvia o `E6`, nem quando está em Customized, como no `set` (`resolve_modes`). A sequência é a do `FanControl.OnClickApply` do `ucCooler.dll` do GCC 26.09.10.01, com `DeviceApi.SetSpeedMode`, `DeviceApi.SetSpeedCurve` e `DeviceApi.Save`, já usada pelo `set`, pelo `curve` e pela restauração e testada no aparelho. A diferença está só na origem do estado (um perfil) e na comparação antes de gravar.

- **Por que comparar:** cada `B6` grava na flash do Castor3. Ativar o mesmo perfil várias vezes, e mais tarde o `systemd-user-service` ativando no login, não pode gravar a flash a cada vez.
- **Alternativa:** um `set` seguido de um `curve`, como a CLI faria em dois comandos. Isso dá dois `B6` e um estado intermediário no aparelho.
- O LED não tem leitura, então sempre é aplicado (spec "Ativação do LED"). Gravar o LED só quando o perfil muda, no lugar de sempre, fica para o `systemd-user-service`, que é quem vai ativar sem um pedido explícito do usuário.

### 5. Perfis em TOML com `serde`, validados pelos tipos do core

Formato do arquivo:

```toml
[waterforce.led]
effect = "static"
colors = ["ff6600"]
brightness = 10
speed = 5

[waterforce.speed]
fan = "quiet"    # ou um RPM: fan = 1500
pump = "turbo"
```

O `serde` desserializa para structs "cruas" (`String`, `u8`, um enum `untagged` preset-ou-RPM), e a validação converte para os tipos do core (`Effect::from_name`, `Rgb::from_str`, `Brightness::new`, `Speed::new`, `FanPreset`/`PumpPreset::from_name`, `Curve::flat`) e monta o `LedSetting` com `LedSetting::new`. Assim, as regras são as mesmas da CLI sem duplicar faixas. Campos desconhecidos são recusados (`deny_unknown_fields`), para que um erro de digitação não seja ignorado sem aviso.

Gravação atômica: escrever em `<nome>.toml.tmp` no mesmo diretório, `fsync` e `rename`. O `active-profile` segue o mesmo caminho.

No D-Bus, o perfil vai como `a{sv}` com as chaves `waterforce.led` e `waterforce.speed`, cada uma um `a{sv}` com os mesmos campos do TOML (`fan`/`pump` como `s` para preset ou `q` para RPM). O `a{sv}` é o formato mais fácil de usar no GJS, que converte para objetos JS com `deepUnpack()`.

- **Alternativa:** passar o TOML como string pelo D-Bus. É mais simples no daemon, mas obriga a extensão a ter um parser TOML.

### 6. Interface D-Bus

```
io.github.leoodaviid.Halo1                (nome do bus)

/io/github/leoodaviid/Halo1
  .Manager    prop Version s
              Devices() -> a(sbs)          id, presente, devnode ("" se ausente)
              signal DevicesChanged(a(sbs))
  .Profiles   List() -> as
              Get(s name) -> a{sv}
              Save(s name, a{sv} profile)
              Delete(s name)
              Activate(s name) -> as       secoes aplicadas
              prop ActiveProfile s         "" quando nenhum

/io/github/leoodaviid/Halo1/waterforce
  .Waterforce.Led        Apply(s effect, as colors, y brightness, y speed)
                         prop LastApplied a{sv}   {} antes da 1a aplicacao
  .Waterforce.Speed      Status() -> a{sv}
                         SetPresets(s fan, s pump)  "" = lado nao informado
                         SetFlatCurve(s side, q rpm)
                         Restore(s which)          "previous" | "original"
  .Waterforce.Telemetry  props Available b, FanRpm u, PumpRpm u,
                         FanDuty y, PumpDuty y,
                         CoolantMilliCelsius i, CoolantValid b
```

As propriedades emitem `org.freedesktop.DBus.Properties.PropertiesChanged`, que é o que o GJS acompanha com `connect('g-properties-changed', …)`.

Mapa dos erros do core para os da spec "Erros D-Bus": validação → `InvalidArgument`; `NotFound` → `NotFound`; `OpenError::PermissionDenied` → `PermissionDenied`; `OpenError::Busy` → `Busy`; `NoResponse`, `UnknownModel`, `Io` → `Device`; `SnapshotError` e erros de arquivo → `Storage`.

### 7. Laço periódico e resume

Uma tarefa assíncrona com `async_io::Timer::interval(2 s)` roda, a cada ciclo, uma lista de "ticks". Nesta change são dois, e os dois só leem o sysfs:
- presença: `find_devices` do IT5702 e do Waterforce e comparação com a lista anterior, com `DevicesChanged` quando muda;
- telemetria: `telemetry::read` e comparação com a leitura anterior, com `PropertiesChanged` só nas propriedades que mudaram.

O resume assina `org.freedesktop.login1.Manager.PrepareForSleep` no D-Bus do sistema, numa segunda conexão. Com `false` (acordou), roda os ticks na hora. Se a conexão com o sistema falhar, o daemon registra um aviso e segue (spec "Resume").

Um tick é um ponto de extensão interno: o `waterforce-cpu-feed` vai somar um que envia o `E0` pelo worker do Waterforce, e o `systemd-user-service` vai decidir se o resume ativa o perfil. Nenhum tick desta change abre hidraw (spec "Nada é enviado ao hardware sem pedido").

### 8. Layout e flags do binário

`crates/halo-daemon/src/`: `main.rs` (argumentos, conexão e laço), `bus.rs` (interfaces e erros), `worker.rs` (fila e backend), `profiles.rs` (formato, validação e armazenamento) e `ticks.rs` (presença, telemetria e resume). O binário aceita `--sys-root` (oculto, para testes, como na CLI) e `--address <endereço D-Bus>` (oculto, para os testes usarem um `dbus-daemon` privado). O log vai para o stderr, que o journald recolhe quando houver a unidade systemd.

## Risks / Trade-offs

- [O `flock` num nó de caractere pode não funcionar em algum kernel] → O `flock` vale para qualquer arquivo aberto no Linux, inclusive nós em devtmpfs. A task do lock testa com um arquivo comum e confere uma vez no `/dev/hidrawN` real (só `open` e `flock`, sem enviar nada).
- [O lock do Waterforce fica preso por uma CLI travada] → O lock acompanha o descritor e some quando o processo morre. A espera é limitada a 5 s, com a mensagem de ocupado.
- [`LastApplied` engana o usuário quando a CLI muda o LED por fora] → A spec proíbe apresentar a propriedade como estado do aparelho. A extensão deve mostrar "último aplicado pelo halo". A CLI notificar o daemon fica fora desta change.
- [Ativar perfis com LED grava a flash a cada vez] → Só acontece com pedido explícito. A política para a ativação automática fica no `systemd-user-service` (Decisão 4).
- [A interface D-Bus muda quando a extensão for desenhada] → O `gnome-panel-prototype` vem antes da extensão. O `Halo1` permite um `Halo2` sem quebrar ninguém, e até a extensão não existe cliente além dos testes.
- [Dependências novas (`zbus` e as transitivas)] → São crates puros em Rust, sem biblioteca C. O `Cargo.lock` fixa as versões.
- [Teste no aparelho] → As sequências são as da CLI, já testadas no aparelho. Uma única ativação real de perfil (task 6.2) confere o caminho de ponta a ponta, com o aval do usuário.

## Migration Plan

Não há estado anterior a migrar. Os snapshots continuam no mesmo lugar e no mesmo formato, compartilhados entre a CLI e o daemon. Para reverter, basta não iniciar o `halod`: a CLI continua igual, só que agora com o lock, que não muda nada quando há um único processo.

## Open Questions

- Intervalo da telemetria configurável (por exemplo, mais lento com o painel fechado): pode virar uma propriedade depois, sem mudar o desenho.
- A CLI usar o daemon quando ele estiver rodando: o lock já resolve a concorrência. Rotear pela D-Bus só vale a pena se o `LastApplied` precisar refletir as mudanças feitas pela CLI.
