# Design

## Context

Ver `proposal.md` (Why) e, na spec `gnome-panel`, os requisitos "Controles de iluminação do Waterforce", "Controles com o LED desligado", "Escolha de cor", "Aplicação só ao concluir o gesto", "Estado mostrado vem do daemon", "Liga e desliga", "Efeitos de paleta só para leitura" e a cor do LED em "Tema do sistema". O delta desta change acrescenta dois cenários (falha com mudança pendente e religar com perfil desligado). O que restringe o desenho:

- **Depois da `gnome-panel-devices`** (contrato entre as duas changes): `extension/lib/carousel.js`, `extension/lib/art.js` (`deviceSvg(device, led)`, `ledPaint(lastApplied)`, com `led = { color, off, rainbow }`), `extension/carousel.js` (St, com o contêiner `body` do aparelho atual e o sinal/callback `device-changed`), o `daemon.js` com `state.devices` (lista completa, com `capabilities`) e `state.lastApplied` (`{effect, colors, brightness, speed}` ou `null`, de um proxy de `Waterforce.Led` com `DO_NOT_AUTO_START`), e o liga/desliga desabilitado no cabeçalho. No `body` do Waterforce, um aviso provisório. O que a implementação fixou (arquivada em `openspec/changes/archive/2026-10-23-gnome-panel-devices`):
  - O painel tem largura fixa: 304 px de conteúdo no `menu.box` (320 px com o padding e a borda do menu do Shell). O `body` fica dentro do `HaloCarousel`, abaixo do bloco do aparelho, com 8 px entre os blocos, e tudo o que entra nele precisa caber nessa largura: um texto com quebra de linha sem largura limitada faz o painel crescer.
  - Os blocos do protótipo (`.tile`) são `.halo-tile`. As medidas ficam no `extension/stylesheet.css`, e as cores de cada tema em `stylesheet-light.css` e `stylesheet-dark.css`, que fazem `@import` do primeiro; o Shell carrega a variante do estilo atual e a troca sozinho.
  - O carrossel já tem `setLed(led)`, que recebe `ledPaint(...)`.
  - O indicador ignora `update()` depois do sinal `destroy`: o Shell o destrói ao sair sem chamar `disable()`.
- **`Waterforce.Led` do `halod`** (`crates/halo-daemon/src/bus.rs`):
  - `Apply(s effect, as colors, y brightness, y speed)`. Brilho e velocidade 0 valem o padrão do efeito (10 e 5, `Brightness::default` e `Speed::default` em `led.rs`). O `off` exige brilho 0 (`LedSetting::new` recusa brilho no `off`). Cores em `RRGGBB`; um efeito sem cor recusa cores (`ColorNotAccepted`), um de cor única aceita uma, uma paleta curta é completada até 8 repetindo a última.
  - `LastApplied` (`a{sv}`): vazio antes da primeira aplicação do processo; só muda com sucesso, e o `PropertiesChanged` sai **antes** da resposta do `Apply` (`apply_led` emite e só depois devolve). Guarda o `off` com brilho 0, as cores como o daemon as normalizou (minúsculas, paleta com 8) e a velocidade padrão quando foi pedida 0. Fica só em memória.
  - Erros (`error.rs`): `io.github.leoodaviid.Halo1.Error.{InvalidArgument, NotFound, PermissionDenied, Busy, Device}`.
- **`Profiles`**: `ActiveProfile` (vazio sem perfil ativo) e `Get(name) -> a{sv}`, em que a seção `waterforce.led` é um `a{sv}` com `effect` e, opcionais, `colors`, `brightness` e `speed` (`profiles.rs`, `to_dbus`). Os perfis ficam em `$XDG_CONFIG_HOME/halo/profiles/<nome>.toml` e o ativo em `$XDG_STATE_HOME/halo/active-profile`.
- **O `halod` não registra os `Apply`**: os únicos `eprintln!` são o início, os erros dos ticks e a mensagem de fim (`main.rs`, `ticks.rs`). O journal não serve para contar escritas.
- **`Slider` do Shell 50** (`resource:///org/gnome/shell/ui/slider.js`, lido do `libshell-18.so` desta máquina): valor contínuo de 0 a 1; `drag-begin` e `drag-end` vêm do `Clutter.PanGesture`, e um clique sem arraste também emite os dois. A roda do mouse (`vfunc_scroll_event`) e as setas (`vfunc_key_press_event`, passo de 0,1) mudam o valor **sem** `drag-begin`/`drag-end`.
- **Sysfs falso dos scripts de teste** (depois da `gnome-panel-devices`, decisão 9 dela):
  - `scripts/dev/fake-sysfs.sh` monta o Castor3 falso em `class/hidraw/hidraw99` e o hwmon, e recusa rodar se `/dev/hidraw99` existir. O `halod` traduz o nome em `/dev/<nome>` (`hid/discovery.rs`), então qualquer abertura falha com `NotFound`. A fumaça já o usa, e os testes do Rust usam `hidraw97` a `hidraw99`. Antes, a fumaça usava `hidraw3`, que nesta máquina é o teclado Redragon (`258a:013b`).
  - `scripts/dev/nested-shell.sh --daemon` ainda sobe o `halod` lendo o `/sys` real, em que o Castor3 é o `hidraw2`, com `uaccess` pela regra do halo. Com os controles desta change, um clique no Shell aninhado escreveria no Waterforce; por isso a task 1.3 o passa ao sysfs falso.
- Monitorar o D-Bus da sessão não exige root: `busctl --user monitor io.github.leoodaviid.Halo1` funcionou nesta máquina como usuário comum. O `dbus-monitor` está instalado.

## Goals / Non-Goals

**Goals:**
- Um `B6` por gesto concluído, no máximo, e nenhum sem gesto. Toda regra que decide se um pedido sai fica numa função pura testada.
- Nenhum teste automático ou Shell de teste capaz de abrir um hidraw real.
- A camada St só liga sinais, mostra o que `ledState.js` decide e chama `daemon.apply`.

**Non-Goals:**
- Mudar o `halod` ou o protocolo. Os pacotes são os que o `halod` já manda.
- As mensagens por tipo de erro, o "Tentar de novo", a linha de status, os perfis no cabeçalho e os detalhes (`gnome-panel-extras`).
- Um dispositivo com `led` que não seja o Waterforce: não existe hoje (`ticks.rs`, `DEVICES`).

## Decisions

### 1. Ordem e contrato com a `gnome-panel-devices`

Esta change só começa a ser implementada depois de a `gnome-panel-devices` estar implementada e conferida. Ela usa os nomes do contrato sem renomear: os controles entram no `body` que o `carousel.js` expõe quando o aparelho atual tem a capacidade `led` e o id `waterforce` (o único caminho D-Bus de LED que existe); o liga/desliga criado desabilitado passa a ser habilitado e ligado aqui; o `daemon.js` ganha `apply(setting)` e `activeProfileLed()`.

Alternativa: uma change só, com o carrossel e os controles. Descartada com o usuário: separar deixa a primeira metade sem nenhuma escrita no hardware e conferida antes de a segunda escrever.

### 2. Estrutura

```text
extension/
  lib/ledState.js        puro: catálogo, controles, fila, igualdade, religar, cor
  ledControls.js         St: efeito, cor, brilho, velocidade, aviso de estado desconhecido, linha de erro mínima
  daemon.js              + apply(setting), activeProfileLed(), lastOn zerado quando o dono muda
  indicator.js           liga/desliga habilitado; troca o aviso provisório do body pelos controles
  stylesheet.css         + medidas das classes .halo-led-*
  stylesheet-light.css   + cores das classes .halo-led-* no tema claro
  stylesheet-dark.css    + cores das classes .halo-led-* no tema escuro
  tests/ledState.test.js
scripts/dev/
  fake-sysfs.sh          já existe (gnome-panel-devices); passa a servir também ao nested (decisão 12)
  watch-led-applies.sh   monitor só de leitura dos Apply (decisão 12)
```

`lib/ledState.js` segue a regra da `lib/`: nada de `gi://St` nem de `resource:///org/gnome/shell/...`, só JavaScript, para rodar com `gjs -m`.

### 3. Modelo puro

Um ajuste é `{effect, colors, brightness, speed}`, com `colors` em `rrggbb` minúsculo. O estado do LED:

```text
led = {
  applied,     // o último ajuste confirmado (LastApplied, ou o que acabou de dar certo); null = desconhecido
  draft,       // o que a interface mostra; null = desconhecido
  lastOn,      // o último ajuste ligado visto nesta instância do daemon
  inFlight,    // o ajuste do Apply em andamento, ou null
  pending,     // só o último ajuste esperando, ou null
  powering,    // lendo o perfil ativo para religar
  error,       // texto da última falha, ou null
}
```

Constantes, iguais ao protótipo e aos nomes de `Effect::NAMES` (`led.rs`): `EFFECTS` (os nove de uma cor, com os nomes em português), `PALETTE_EFFECTS` (os seis de paleta), `HAS_COLOR` (`static`, `pulse`, `flash`, `double-flash`, `gradient`), `HAS_SPEED` (`pulse`, `flash`, `double-flash`, `gradient`, `cycle`, `wave`, `rainbow`), `usesSpeed(e) = HAS_SPEED ∪ paleta`, `SWATCHES` (as seis cores prontas, com o laranja `ff6600`).

Os padrões são os do GCC, como no `halod`: brilho 10, velocidade 5 e cor `ff6600` (`Effect::default_colors`). O `DEFAULT_LED` do protótipo (pulso, brilho 8, velocidade 2) era só um cenário de demonstração.

Funções (todas puras, todas testadas):
- `fromLastApplied(rec)`: `{}` ou `null` → `null`; senão o ajuste, com brilho 0 (o `off`) lido como 10 para exibição.
- `controlsView(led)`: o efeito mostrado, se a cor, o brilho e a velocidade aparecem, se estão esmaecidos e travados, o texto da velocidade (`n de 5`, "não se aplica" ou `—`), a paleta só de leitura, o aviso de estado desconhecido e o estado do switch (decisão 7).
- `effectChoice(led, effect)`, `withColor`, `withBrightness`, `withSpeed`: o novo rascunho.
- `request(setting)` e `sameEffective(a, b)` (decisão 4).
- `commit(led, setting)`, `finished(led, ok, error)`: a fila (decisão 5), cada uma devolvendo `{led, send}`.
- `powerTarget(lastOn, profileLed)` (decisão 8) e `profileLed(dict)`, que lê o `a{sv}` do `Profiles.Get` já desempacotado.
- `parseHex`, `hueOf`, `hueToHex`, `paletteShown(colors)` (a paleta sem a repetição final que o daemon acrescenta).

Alternativa: o estado dentro de `ledControls.js`, como no protótipo. Descartada: a fila e a igualdade decidem quantas vezes o aparelho grava, e precisam de teste sem o Shell.

### 4. O que vai no pedido e quando dois ajustes são iguais

`request(setting)` manda só o que o efeito usa, como a CLI:

| Efeito | `colors` | `brightness` | `speed` |
|---|---|---|---|
| `off` | `[]` | `0` | `0` |
| com cor | a primeira | 1 a 10 | a do rascunho se `usesSpeed`, senão `0` |
| sem cor (`cycle`, `wave`, `rainbow`) | `[]` | 1 a 10 | 1 a 5 |
| paleta | as cores do `LastApplied`, sem mudar | 1 a 10 | 1 a 5 |

`sameEffective(a, b)` compara o que chega ao aparelho: o mesmo efeito; o mesmo brilho, salvo no `off`; a mesma velocidade só quando o efeito a usa; as mesmas cores só nos efeitos com cor, e nas paletas depois de completar até 8 repetindo a última. Sem isso, um estático pedido com velocidade 0 voltaria do `LastApplied` com 5 e pareceria diferente do rascunho, e soltar o brilho no mesmo valor geraria um `B6` à toa.

Na paleta, o painel reenvia as 8 cores que o `LastApplied` publica: o `halod` completaria a lista curta do mesmo jeito, então os pacotes são os mesmos que os do perfil original.

### 5. Fila: um em voo e só o último pendente

```text
commit(s):    inFlight? -> pending = s                      (substitui o anterior)
              sameEffective(s, applied)? -> nada
              senão -> inFlight = s; send(request(s))
finished ok:  applied = inFlight; inFlight = null; error = null
              p = pending; pending = null
              p e não sameEffective(p, applied)? -> inFlight = p; send(request(p))
finished erro: inFlight = null; pending = null; draft = applied; error = mensagem
dono some:    applied = draft = lastOn = inFlight = pending = null
```

No sucesso, `applied` passa a ser o ajuste enviado sem esperar o `PropertiesChanged`; ele chega antes da resposta (Context), e o `LastApplied` que chegar depois só confirma. Um `LastApplied` que mude sem pedido do painel (outro cliente do D-Bus, ou um perfil ativado) atualiza `applied` e `lastOn` e, sem nada em voo nem pendente, também o `draft`.

Com isso, um gesto gera no máximo um `Apply`, e uma rajada de gestos com um pedido em voo gera no máximo dois: o em voo e o último pendente. A falha descarta o pendente (cenário novo do delta): os controles voltam ao estado aplicado, e mandar a mudança que esperava seria escrever algo que o usuário já não vê.

Alternativa: um atraso fixo depois de cada gesto, sem fila. Descartada: não impede dois pedidos simultâneos, e o atraso pesa em todo clique.

### 6. Fim de gesto em cada controle

| Controle | Durante o gesto | Fim do gesto, que chama `commit` |
|---|---|---|
| Brilho e velocidade: subclasse do `Slider` do Shell | `notify::value` arredonda para o inteiro e muda só o `draft` | `drag-end` (arraste ou clique) |
| Setas do teclado nos sliders | o `vfunc_key_press_event` próprio anda um inteiro por tecla | 300 ms sem outra tecla |
| Roda do mouse nos sliders | desligada: `vfunc_scroll_event` devolve `PROPAGATE` | — |
| Barra de matiz: `St.DrawingArea` com gradiente Cairo e um `Clutter.PanGesture` | a cor do `draft` segue o ponteiro | o `end` do gesto |
| Setas na barra de matiz | 10° por tecla | 300 ms sem outra tecla |
| Cores prontas: `St.Button` | — | 300 ms sem outro clique, como o `commitColorSoon` do protótipo |
| Hex: `St.Entry` | o texto; `#` opcional | Enter (`activate`) ou perda do foco, se `parseHex` aceitar; inválido marca o campo e não envia; Esc desfaz |
| Efeito: `PopupMenu.PopupMenu` ancorado no botão | — | escolher um item diferente do atual |
| Liga/desliga | — | o clique (decisão 8) |

A roda fica desligada porque no Shell 50 ela muda o valor sem `drag-end`, e uma rolagem do painel por cima do slider viraria uma sequência de pedidos. As setas e os cliques seguidos são agrupados pelo mesmo intervalo de 300 ms, para que segurar uma tecla não grave uma vez por passo. Os temporizadores ficam no `ledControls.js` (`GLib.timeout_add`) e são removidos no `destroy()`.

Alternativa: aplicar no `notify::value` com um atraso. Descartada: o arraste geraria pedidos no meio do gesto, contra a spec.

### 7. Controles visíveis, desligado e estado desconhecido

`controlsView` segue o `renderDevice` e o `shownSetting` do protótipo:
- Com o LED ligado: o `draft`. Cor só em `HAS_COLOR`; velocidade habilitada em `usesSpeed`, senão "não se aplica"; paleta com o nome e as cores, sem edição, e o popup de efeito só com os nove de uma cor.
- Layout, como o protótipo: um bloco (`.halo-tile`, padding de 10 px e 12 px, 280 px úteis) com o efeito e a cor, e outro com o brilho e a velocidade, abaixo do bloco do aparelho. Nada no `body` pode pedir mais que a largura do painel.
- Com o LED desligado: cor, brilho e velocidade com os valores de `lastOn` (ou os padrões, sem `lastOn`), esmaecidos pela opacidade do ator (0,4, como o switch desabilitado; o CSS do St não tem `opacity`) e com `reactive` e `can_focus` falsos. O efeito e o switch continuam ativos.
- Estado desconhecido (`draft` nulo): o aviso de que o estado do LED não é conhecido, o botão de efeito como "Escolher efeito", cor, brilho e velocidade travados como no desligado. Nada é enviado até um gesto.
- O switch fica ligado quando o `draft` existe e não é `off`. Ele fica insensível sem dono, com algo em voo ou durante a leitura do perfil (`powering`), como no protótipo. No estado desconhecido, ele aparece desligado e ativo: clicar é religar (decisão 8), que é um gesto do usuário.

### 8. Liga e desliga

- **Desligar**: `commit({...draft, effect: 'off', colors: []})`. O `request` manda `off`, `[]`, 0, 0.
- **`lastOn`**: o último `applied` diferente de `off` desde que o nome ganhou o dono atual. Ele zera quando o dono some ou muda, porque o `LastApplied` de um `halod` novo nasce vazio (cenário "Religar sem estado conhecido").
- **Religar**: `powerTarget(lastOn, profileLed)`, nesta ordem: `lastOn`; o LED do perfil ativo, se ele existir e não for `off` (cenário novo do delta); senão `{effect: 'cycle', colors: [], brightness: 10, speed: 5}`. O perfil só é lido quando falta `lastOn`: `daemon.activeProfileLed()` lê `ActiveProfile` e, se não for vazio, chama `Profiles.Get(nome)`, os dois pelo proxy de `Profiles` com `DO_NOT_AUTO_START`. Erro no `Get` (`ProfileNotFound`, por exemplo) conta como sem perfil. Durante a leitura, `powering` deixa o switch insensível.

`Profiles.Activate` não é usado: ativar o perfil mudaria a velocidade da fan e da bomba e o perfil ativo gravado, e religar é só o LED. O painel pede ao `Led.Apply` o mesmo LED que o perfil tem.

Alternativa: religar sempre com `cycle`. Descartada: a spec pede o último estado ligado e o perfil antes do `cycle`.

### 9. Falha: o mínimo antes da `gnome-panel-extras`

O requisito "Erros de aplicação" (mensagens por tipo e "Tentar de novo") é da `gnome-panel-extras`. Esta change faz só o que "Estado mostrado vem do daemon" exige e o necessário para o usuário notar a falha:
- os controles voltam a `applied` e o pendente é descartado (decisão 5);
- uma linha curta abaixo dos controles: "Não foi possível aplicar" seguido da mensagem do `halod` (o texto do erro D-Bus, sem o nome). Ela some no gesto seguinte ou num sucesso;
- `console.warn` com o nome do erro, que aparece no `journalctl --user` do Shell.

Um `Apply` cancelado no `disable()` não mostra nada. O `halod` termina o pedido mesmo assim, então esse pedido ainda pode gerar um `B6`.

Alternativa: não mostrar nada até a `gnome-panel-extras`. Descartada: os controles voltariam ao estado anterior sem explicação, e a conferência no Shell aninhado precisa ver o `NotFound`.

### 10. O desenho segue o rascunho

O anel do desenho usa o `draft`, e não o `applied`: durante o arraste da matiz e antes de o pedido terminar, ele já mostra a cor escolhida. Com a falha, o `draft` volta ao `applied` e o anel também. A `ledPaint` da `gnome-panel-devices` recebe um objeto com o mesmo formato do `LastApplied` (`{effect, colors, brightness, speed}`), então ela é chamada com o `draft`, e `null` continua sendo o anel neutro. A cor do LED aparece só no desenho e no seletor de cor; o switch, a seleção e os menus continuam com o destaque do sistema (`-st-accent-color`).

Alternativa: o anel só com o `LastApplied`, como na `gnome-panel-devices`. Descartada: durante o arraste a cor mudaria só no seletor, e a spec diz que a mudança aparece na interface.

### 11. D-Bus

- O proxy de `Waterforce.Led` da `gnome-panel-devices` ganha o método `Apply` no XML. `daemon.apply(setting)` chama `ApplyAsync(effect, colors, brightness, speed)` com o `Cancellable` do `HaloDaemon` e devolve `{ok}` ou `{error: {name, message}}`, com o nome tirado por `Gio.DBusError.get_remote_error`. Com `DO_NOT_AUTO_START`, a chamada também não ativa o daemon.
- Proxy novo de `io.github.leoodaviid.Halo1.Profiles` (`ActiveProfile` e `Get`), criado como os outros, com `DO_NOT_AUTO_START` e o mesmo `Cancellable`.
- A resposta de um `Apply` que chegue depois do `destroy()` não desenha nada: o `Cancellable` do `HaloDaemon` é cancelado no `disable()`, e o indicador ignora `update()` depois do `destroy` (o Shell o destrói ao sair sem chamar `disable()`).
- O `ledState` vive no `daemon.js` junto do resto do estado, para que a mudança de dono zere a fila e o `lastOn` no mesmo lugar em que o `lastApplied` já é zerado.
- Tempo limite do `Apply`: o padrão do D-Bus. O `halod` responde em até ~2 s no pior caso do `DE 00` (`RESPONSE_TIMEOUT`), mais a espera do `flock`.

### 12. Testes sem o aparelho

| Camada | O que prova |
|---|---|
| `sh extension/tests/run.sh` (`gjs -m`) | Os cenários da spec e do delta em `ledState.js`: efeitos e controles, desligado, desconhecido, fila (um em voo, só o último pendente, falha descarta o pendente), igualdade, religar, hex, matiz e paleta |
| `sh scripts/dev/headless-smoke.sh` | A extensão carrega com os módulos novos, sobrevive a `disable`/`enable` sem `JS ERROR`, e **não chama `Apply` nenhuma vez** sem gesto. Antes do Shell, um `Apply` mandado pelo script ao `halod` de teste volta `NotFound`, o que prova que o sysfs falso não leva a nenhum nó real |
| `sh scripts/dev/nested-shell.sh --daemon` | O usuário clica nos controles; todo `Apply` falha com `NotFound`; o script registra os `Apply` do barramento de teste, com os argumentos, e os mostra ao fechar |
| Conferência final | Decisão 13 |

**Sysfs falso seguro** (`scripts/dev/fake-sysfs.sh`, criado pela `gnome-panel-devices` e usado pelos dois scripts; esta change acrescenta o `Apply` de prova na fumaça e passa o nested a ele): o Castor3 vira `class/hidraw/hidraw99` (mesmo `uevent` e descritor de hoje) e o hwmon continua igual. O script recusa rodar se `/dev/hidraw99` existir. Assim o `halod` acha o aparelho (o carrossel mostra o Waterforce com `led`), mas `Hidraw::open` recebe `ENOENT` e devolve `NotFound` (`OpenError::from_io`) antes de qualquer escrita. O `--daemon` do nested deixa de ler o `/sys` real; não sobra nenhum modo de teste que suba o `halod` com o `/sys` real.

**Perfil no Shell aninhado**: `--profile` grava, no `$XDG_CONFIG_HOME` temporário, um perfil `teste` com `[waterforce.led]` tri-color e marca `$XDG_STATE_HOME/halo/active-profile`. Com ele, religar sem `lastOn` pede o tri-color; sem ele, o `cycle`. Os dois aparecem no registro de `Apply` do script.

**Registro de `Apply` no barramento de teste**: `dbus-monitor --address "$DBUS_SESSION_BUS_ADDRESS"` (o barramento do `dbus-run-session`, não o da sessão real) com as regras da decisão 13, gravado num arquivo do temporário.

Alternativa: um `halod` com um aparelho falso que aceite as escritas (`uhid`). Descartada: o `/dev/uhid` exige root, e o caminho de sucesso já está coberto pelos testes puros e pela conferência final.

### 13. Conferência final e contagem de `B6`

Com o aval do usuário, `packaging/install.sh`, sair e entrar na sessão. Num terminal, o monitor só de leitura do D-Bus da sessão real (`scripts/dev/watch-led-applies.sh`, que embrulha o `dbus-monitor --session` e não chama nenhum método):

```text
type='method_call',interface='io.github.leoodaviid.Halo1.Waterforce.Led',member='Apply'
type='signal',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged',
     path='/io/github/leoodaviid/Halo1/waterforce',arg0='io.github.leoodaviid.Halo1.Waterforce.Led'
type='error',sender=<dono de io.github.leoodaviid.Halo1>
```

Ele imprime uma linha por `Apply` pedido (com os argumentos), uma por sucesso e uma por erro, e o total de `B6`. A contagem vale porque:
- `LedSetting::packets()` sempre termina com um único `99 B6` (teste em `led.rs`), e `apply_to` não manda mais nada depois;
- o `PropertiesChanged` do `LastApplied` sai só com sucesso, um por `Apply` (e um por `Profiles.Activate` com LED, que o painel não chama);
- um `Apply` com erro de `NotFound`, `PermissionDenied`, `Busy`, sem resposta ao `DE 00` ou com modelo desconhecido não mandou nenhum pacote de LED, logo nenhum `B6`. Só um `Device` com erro de escrita no meio da sequência deixa a dúvida, e a conferência o registra como tal.

Ninguém mais pode falar com o `halod` durante a conferência (nada de `busctl` ou outro cliente), e a CLI `halo led` fica sem uso, porque abre o hidraw direto e não aparece no D-Bus. Captura de USB (`usbmon`) exigiria root e não é usada.

Critérios de aceitação, contados no monitor:
- arrastar o brilho de ponta a ponta e soltar: **um** `Apply` e **um** `B6`;
- soltar o brilho no valor já aplicado: nenhum `Apply`;
- mudar a cor duas vezes com um pedido em voo: **no máximo dois** `Apply`. O `Apply` real leva poucos milissegundos, então provocar o pedido em voo à mão é improvável; a fila é provada pelos testes puros, e aqui vale o limite.
- desligar e religar: um `Apply` cada, com o `off` e depois o estado anterior.

Cada gesto da conferência é dito antes ao usuário, com o comando que vai e o efeito esperado no cooler. O estado final deixado no aparelho é combinado com ele (o `cycle` é o de fábrica, conforme `docs/hardware.md`).

Alternativa: contar pelo journal do `halod`. Descartada: ele não registra os `Apply` (Context), e mudar o daemon para isso está fora do escopo.

### 14. Bytes que cada `Apply` faz o `halod` mandar

O painel não monta bytes: ele chama `Led.Apply`, e o `halod` usa `LedSetting::packets()` e `apply_to` do `halo-core`, cujos bytes estão em `docs/protocols/castor3.md` e foram conferidos no aparelho na change `waterforce-led`. Fonte de cada um:

| Bytes | Fonte |
|---|---|
| `99 DE 00` (conferência do modelo antes de escrever) | `DeviceApi.GetDeviceModelName` do `ucCooler.dll` (ilspycmd, "Identificação e status") |
| `99 C9 <efeito> <brilho×10> <velocidade×2> <índice> <área>` | `FUN_180040fc0` da `GcLedLib.dll` (Ghidra 12.1.4 pelo REA), com o dobro da velocidade de `dllexp_GvAIOV2LedSet` e o `Led_Cfg` de `LedApi.LedSettingToConfig` |
| `99 CD R G B` | `FUN_180041470` da `GcLedLib.dll` (REA) |
| `99 B0`–`B3 <efeito> RGB RGB` | `FUN_1800410d0` da `GcLedLib.dll` (REA) |
| `99 B6` | `FUN_180040780` da `GcLedLib.dll` (REA) |
| A sequência por efeito, inclusive o segundo `C9 … 0A 01` do gradiente e o `off` como estático com brilho 0 e `CD 00 00 00` | `switch` de `FUN_1800402a0` da `GcLedLib.dll` (REA) e `On = false` do C# do GCC |

O OpenRGB (`GigabyteCastor3Controller.cpp`, `72262b8`) usa os mesmos comandos, mas troca o brilho e a velocidade no `C9`, e não é usado (`castor3.md`, "Comparação com o OpenRGB").

Já vistos no aparelho: `static`, `pulse`, `cycle`, `rainbow` e `tri-color`. Ainda não vistos: `flash`, `double-flash`, `gradient`, `wave` e `off` (e os de paleta que o painel não oferece). A conferência pode mostrá-los pela primeira vez, com os mesmos comandos, e o resultado vai para `docs/hardware.md`.

## Risks / Trade-offs

- [Desgaste por `B6` desconhecido] → Um `Apply` por gesto concluído, nenhum sem gesto, nenhum igual ao aplicado, roda do mouse desligada nos sliders, teclas e cliques seguidos agrupados, no máximo dois por rajada. Tudo isso testado em `ledState.js` e contado na conferência.
- [Teste abrindo um hidraw real] → Sysfs falso com `hidraw99`, recusado se `/dev/hidraw99` existir, e a fumaça confirmando `NotFound` antes de subir o Shell. Nenhum modo de teste com o `/sys` real.
- [`disable()` com um `Apply` em voo] → O resultado é ignorado, mas o `halod` termina o pedido: no máximo um `B6` a mais, que o monitor da conferência mostra.
- [Outro cliente aplicando ao mesmo tempo] → O painel segue o `LastApplied` quando não tem nada em voo nem pendente; com algo em voo, a resposta do próprio pedido vence, e o `LastApplied` seguinte corrige.
- [Equivalência errada entre pedido e `LastApplied`] → `sameEffective` testada com o que o `halod` devolve de verdade (velocidade 5 no estático pedido com 0, paleta com 8, `off` com brilho 0). Um erro aqui gera um `Apply` a mais, e a conferência o mostraria.
- [`ledPaint` da `gnome-panel-devices` com outro formato] → Conferido na implementação de lá: `ledPaint` recebe o formato do `LastApplied`, e o carrossel tem `setLed`. A task 3.2 só chama `setLed(ledPaint(draft))`.
- [Atualizar a extensão exige sair e entrar] → Conferência visual no Shell aninhado antes de instalar.

## Migration Plan

1. Implementar depois da `gnome-panel-devices`; testes puros, fumaça e Shell aninhado sem o aparelho.
2. Com o aval do usuário: `packaging/install.sh`, sair e entrar, conferência com o monitor.
3. Voltar atrás: reinstalar a versão anterior da extensão (o `install.sh` de um commit anterior) ou `gnome-extensions disable halo@leoodaviid.github.io`. O LED fica no último estado aplicado; `halo led waterforce cycle` volta ao modo de fábrica.
