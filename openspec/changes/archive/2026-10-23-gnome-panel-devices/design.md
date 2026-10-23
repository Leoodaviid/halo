# Design

## Context

Ver `proposal.md` (Why) e a spec `gnome-panel` (requisitos "Aparelhos vindos do daemon", "Navegação do carrossel", "Aparelho sem controle", "Imagens locais" e "Tema do sistema"). O que restringe o desenho:

- A extensão da `gnome-shell-extension`: `daemon.js` com um estado único e um callback, proxies com `DO_NOT_AUTO_START`, o dono do nome pelo `g-name-owner`, `lib/` puro testado com `gjs -m` (`sh extension/tests/run.sh`), `indicator.js` só desenhando. O painel hoje é um `PopupMenu` com o título "halo" e a contagem de aparelhos presentes.
- `halod` (`crates/halo-daemon/src/bus.rs`):
  - `Manager.Devices()` devolve `aa{sv}` com `id`, `name`, `present`, `devnode` e `capabilities` (`as`), e o sinal `DevicesChanged` traz a mesma lista. Os ids hoje são `waterforce` (`led`, `speed`, `telemetry`), `it5702` (sem capacidades) e `g502`.
  - `Waterforce.Led` em `/io/github/leoodaviid/Halo1/waterforce` tem a propriedade `LastApplied` (`a{sv}`: `effect` `s`, `colors` `as` em `rrggbb` minúsculo, `brightness` `y`, `speed` `y`), vazia antes da primeira aplicação do processo, com `PropertiesChanged`. O desligado vem com brilho 0.
- O protótipo (`docs/ui/panel-prototype.html`): `drawWaterforce`, `drawBoard`, `drawGeneric` e `drawDevice` geram os SVGs (`viewBox="0 0 300 156"`, mostrados em 280×128). As partes do LED, só no Waterforce, têm as classes `.led-stroke` e `.led-fill`, pintadas pela variável `--led-paint`. `setLedPaint()` decide a pintura: sem estado ou com `off`, a classe `.led-off` (cinza `#4a4a50` com opacidade 0,6); efeitos sem cor e fora da paleta, `url(#ledRainbow)`; nos demais, a primeira cor. `syncDevices()` mantém a página pelo id e cai no primeiro; `select()` é circular; a roda acumula `deltaY` e dá um passo quando passa de 24 e já se passaram 330 ms do último passo (a soma zera depois de 350 ms parado).
- Fatos confirmados nesta máquina (GNOME Shell 50.1, Wayland, gjs 1.88):
  - O carregador de SVG reconhece o formato pelos primeiros bytes: o `<svg` vem logo depois da declaração XML, sem comentários antes. Um SVG gerado como texto e mostrado com `Gio.BytesIcon` num `St.Icon` passa pelo mesmo carregador.
  - O Shell recolore ícones simbólicos forçando `fill`. Os desenhos têm cores próprias, então nenhum nome termina em `-symbolic`.
  - `gjs -m` decodifica o SVG gerado com `GdkPixbuf.Pixbuf.new_from_stream_at_scale` e dá acesso aos pixels (conferido com um anel azul: o pixel do traço sai `0 0 255`).
  - Os dois Shells de teste (`headless-smoke.sh` e `nested-shell.sh`) já têm `HOME`, `XDG_*` e `XDG_RUNTIME_DIR` próprios. No Wayland, a extensão só recarrega saindo e entrando na sessão.

## Goals / Non-Goals

**Goals:**
- Toda decisão de exibição do carrossel e do desenho em funções puras de `lib/`, testadas com `gjs -m`, e o St só ligando sinais e desenhando.
- Deixar para a `gnome-panel-led` pontos de encaixe estáveis: o contêiner `body`, o sinal `device-changed`, o liga/desliga no cabeçalho e o `state.lastApplied`.
- Nenhuma escrita no hardware, nem indireta: a extensão não chama nenhum método do `halod` que mude um aparelho.

**Non-Goals:**
- Os controles do LED, o liga/desliga habilitado, a fila de `Apply` e o religar (`gnome-panel-led`).
- A legenda de cada aparelho abaixo do nome (no protótipo, "Anel e ventoinhas RGB", "IT5702 · USB HID"): é informação de catálogo e fica para os detalhes da `gnome-panel-extras`, se o daemon passar a fornecê-la.
- O fundo translúcido com blur e os blocos do protótipo: o carrossel entra no menu atual com CSS próprio.
- Toque (arrastar com o dedo), que o protótipo tem e a spec não pede.

## Decisions

### 1. Duas changes: primeiro os aparelhos, depois o LED

A `gnome-panel-led` planejada na `gnome-shell-extension` trazia o carrossel, os desenhos e os controles juntos. Ela foi dividida: esta change (`gnome-panel-devices`) traz o carrossel e os desenhos sem nenhuma escrita no hardware, e a `gnome-panel-led` traz os controles, que escrevem. A `gnome-panel-extras` continua depois.

Alternativa: uma change só. Descartada: o carrossel e o desenho são grandes e se conferem sem tocar no hardware, enquanto cada conferência dos controles grava no aparelho. Separar deixa a parte que escreve menor e conferida sobre uma base já aceita.

O contrato com a `gnome-panel-led` (nomes fixos):

- `extension/lib/carousel.js` e `extension/lib/art.js` (decisões 3 e 4);
- `extension/carousel.js` com o contêiner `body` e o sinal `device-changed` (decisão 5);
- `state.devices` e `state.lastApplied` no `daemon.js` (decisão 2);
- o liga/desliga desabilitado no cabeçalho (decisão 6).

A `gnome-panel-led` cria `lib/ledState.js` e `ledControls.js`, preenche o `body` do Waterforce no lugar do aviso provisório e acrescenta o `Apply` e a leitura do perfil ativo ao `daemon.js`. Nada disso entra aqui.

### 2. Estado: a lista completa e o `LastApplied`

```text
state = {
  ...o que já existe (owner, telemetry, devicesPresent, unit, journal),
  devices: [{ id, name, present, capabilities: ['led', ...] }],   // ordem do Devices()
  lastApplied: { effect, colors: ['rrggbb'], brightness, speed } | null,
}
```

- `devices` vem do `Devices()` ao aparecer o dono e de cada `DevicesChanged`; `devicesPresent` continua sendo `countPresent(devices)`, para o que já usa a contagem. A normalização do `aa{sv}` já desempacotado (`recursiveUnpack`) fica numa função pura de `lib/view.js` (`devicesFrom`), que tolera chaves ausentes (`present` falso, `capabilities` vazio, `name` igual ao `id`).
- `lastApplied` vem de um terceiro proxy, de `io.github.leoodaviid.Halo1.Waterforce.Led` em `/io/github/leoodaviid/Halo1/waterforce`, criado com `DO_NOT_AUTO_START` como os outros: só a propriedade `LastApplied` na interface do wrapper, sem o método `Apply`, que é da `gnome-panel-led`. É lido quando o dono aparece e a cada `g-properties-changed` desse proxy. `lastAppliedFrom` (também em `lib/view.js`) devolve `null` para o dicionário vazio ou sem `effect`.
- Quando o dono some, `devices` volta a `[]` e `lastApplied` a `null`: o cache de um proxy sem dono não vale.

Alternativa: um `Gio.DBusConnection.call` ao `Properties.Get` em vez do proxy. Descartada: o proxy já entrega o `PropertiesChanged` no cache, como no `Telemetry`, e mantém a regra única de `DO_NOT_AUTO_START`.

### 3. Lógica do carrossel em `lib/carousel.js`

Funções puras, sem St:

- `presentDevices(devices)`: só os presentes, na ordem da lista.
- `nextIndex(i, n)` e `prevIndex(i, n)`: circulares (`n` igual a 0 devolve 0).
- `selectAfterChange(prevId, devices)`: o índice do aparelho `prevId` entre os presentes; se ele não estiver mais, 0; `null` se não houver presentes. É o que cobre "Aparelho desconectado" e "Nome muda" (o id fica, o nome vem da lista nova).
- `statusText(device)`: "Disponível" com a capacidade `led`, "Detectado" sem.
- `wheelStep(gate, dy, now)`: a regra da roda do protótipo (passo quando a soma passa do limiar e o último passo foi há mais de 330 ms; soma zerada depois de 350 ms parado), devolvendo `{gate, step}` com `step` em −1, 0 ou 1. Pura e com o tempo como argumento, para o teste não depender de relógio. Os eventos discretos do Clutter (`UP`/`DOWN`) entram como um `dy` acima do limiar; os `SMOOTH`, pelo `get_scroll_delta()`.
- `lockedText(device)`: "O halod detectou <nome>, mas ainda não tem controle de iluminação para ele.", o texto do protótipo.

`wheelStep` e `lockedText` são acréscimos ao contrato, sem mudar os nomes combinados.

### 4. Desenhos em `lib/art.js`, gerados como texto

- `deviceSvg(device, led)`: o texto do SVG para `waterforce`, `it5702` e, para qualquer outro id, o genérico com o nome do daemon. `led = { color: 'rrggbb' | null, off: bool, rainbow: bool }`. Só o Waterforce tem partes de LED; os outros ignoram `led`.
- Os SVGs são os do protótipo, com três ajustes para o carregador de imagens, que não tem CSS:
  - as classes `.led-stroke` e `.led-fill` viram atributos `stroke` e `fill` com a pintura já resolvida;
  - o `<defs>` do Waterforce leva também o `linearGradient` `ledRainbow` do protótipo, usado como `url(#ledRainbow)`;
  - o texto do genérico troca `currentColor` e `font-family="inherit"` por uma cor fixa legível nos dois temas (`#8a9198`, a do desenho) e `sans-serif`, e o nome é escapado para XML (`&`, `<`, `>`, `"`, `'`).
  - Sem `filter` nem `class`: a sombra do protótipo é CSS e não entra.
- O texto começa com `<?xml version="1.0" encoding="UTF-8"?>` seguido direto do `<svg xmlns="http://www.w3.org/2000/svg" ...>`, sem comentário antes, e nenhum nome de arquivo ou recurso termina em `-symbolic`.
- O SVG é quadrado: `width="300" height="300" viewBox="0 -72 300 300"`, com a arte de 300×156 do protótipo no meio. O `St.Icon` desenha um quadrado e esticaria um SVG não quadrado (a saída prevista em "Riscos", aplicada já na task 1.2).
- `ledPaint(lastApplied)` decide o `led`, seguindo o `setLedPaint()` do protótipo:

| `lastApplied` | `led` | Anel |
|---|---|---|
| `null` | `{color: null, off: false, rainbow: false}` | neutro: cinza `#4a4a50`, opacidade cheia |
| `effect` = `off` | `{color: null, off: true, rainbow: false}` | esmaecido: cinza `#4a4a50` com opacidade 0,6 |
| `cycle`, `wave`, `rainbow` | `{color: null, off: false, rainbow: true}` | `url(#ledRainbow)` |
| `static`, `pulse`, `flash`, `double-flash`, `gradient` | `{color: colors[0], ...}` | a cor |
| paleta (`color-shift`, `tri-color`, `spin`, `switch`, `lego`, `aurora`) | `{color: colors[0], ...}` | a primeira cor da paleta |
| efeito desconhecido ou sem cor válida | como `null` | neutro |

  Uma cor só é aceita se for `^[0-9a-f]{6}$` (minúsculas, como o daemon publica), para nenhum texto do D-Bus entrar no SVG sem validação. No protótipo, sem estado e desligado têm o mesmo cinza esmaecido; aqui o neutro fica com opacidade cheia para distinguir "desconhecido" de "desligado".
- No St: `new St.Icon({gicon: new Gio.BytesIcon({bytes})})`, com os bytes do texto em UTF-8. O desenho é regerado quando o aparelho atual ou o `led` mudam, e só se o texto mudou.

Alternativa: redesenhar em Cairo (`St.DrawingArea`). Descartada: seria reescrever à mão os caminhos do protótipo, sem um jeito de testar o resultado fora do Shell. O SVG gerado é testado como texto e como imagem (decodificado com o `GdkPixbuf` no `gjs -m`, conferindo a cor de um pixel do anel), e a arte continua a mesma do protótipo aprovado. A alternativa da tabela do `docs/architecture.md` (camadas de SVG com o LED simbólico) também fica de fora: a recoloração simbólica força `fill`, o que não serve aos traços do anel, nem ao arco-íris.

### 5. O carrossel no St (`extension/carousel.js`)

`HaloCarousel`, um `St.BoxLayout` vertical registrado com `GObject.registerClass` e o sinal `device-changed` (parâmetro: o aparelho selecionado, como objeto JS, ou `null`):

```text
HaloCarousel (vertical, espaço de 8 px, como o .page)
  tile (bloco do aparelho, o .tile do protótipo)
    stage (St.Widget, 128 px, can_focus, reactive, track_hover)  <- scroll-event, key-press-event
      artClip (St.Widget 280x128, FixedLayout, clip_to_allocation)
        art (St.Icon com Gio.BytesIcon, icon-size 246, centrado por um Clutter.AlignConstraint)
      prev / next (St.Button redondos de 24 px, sobre o desenho)
    meta: nome (St.Label)  +  chip (ponto e texto, verdes em "Disponível")
    pager (St.BoxLayout de St.Button de 6 px)
  body (St.BoxLayout vertical, público)
  empty (bloco "Nenhum aparelho encontrado")
```

- `setDevices(devices)` aplica `presentDevices` e `selectAfterChange` com o id atual; `setLed(led)` regera o desenho. Quem chama é o `indicator.js`, a cada mudança de estado.
- Setas, indicador de página, roda e teclas chamam o mesmo `select(index, dir)`; com menos de dois aparelhos, setas e indicador ficam ocultos e a navegação não faz nada.
- Teclas: Esquerda e Direita, com o foco no `stage`. Cima e Baixo continuam com o menu, que as usa para andar entre os itens. O protótipo também aceita Cima e Baixo, mas lá não há um menu em volta.
- A troca de página tem a transição curta do protótipo (opacidade e deslocamento de 12 px com `ease()`, cerca de 140 ms), cancelada se outra troca começar antes, e nenhuma transição quando a lista muda.
- `device-changed` sai na troca de página e quando o aparelho selecionado muda por causa da lista (inclusive para `null`), não em toda atualização de estado.
- Tamanho, como o `.art svg` do protótipo (280×128 com a proporção mantida, a arte em 246×128): o `St.Icon` tem `icon-size` 246 e fica no tamanho natural, um quadrado de 246 px, centrado por um `Clutter.AlignConstraint` dentro do `artClip` de 280×128, que corta a sobra transparente de cima e de baixo. O ícone não pode ficar num `BinLayout` da altura do desenho: o layout limita a altura dele, e o `St.Icon` encolhe o quadrado inteiro para caber (a arte cairia para cerca de 146×76).
- As setas ficam com opacidade 0 e aparecem com `ease()` quando o ponteiro está sobre o `stage` (`hover`) ou o foco está nele ou num filho (`notify::key-focus` do `global.stage`), como o `.stage:hover .nav` e o `.stage:focus-within .nav` do protótipo.

Alternativa: um `St.ScrollView` paginado com todos os desenhos lado a lado. Descartada: o protótipo mostra um aparelho de cada vez, e um único `St.Icon` evita manter vários desenhos e seus textos.

### 6. Painel: cabeçalho, carrossel e corpo

- O painel tem a largura fixa do `.material` do protótipo: 304 px de conteúdo no `menu.box`, que com o padding de 1 px, os 6 px do `.popup-menu-content` e a borda de 1 px do Shell dão 320 px. Sem a largura fixa, um texto com quebra de linha pede a largura dele inteiro numa linha só, e o painel cresce ao trocar de aparelho. Os itens do carrossel e do cabeçalho têm o padding do Shell zerado, e o espaço vem do CSS do halo.
- O cabeçalho troca o item "halo" por uma linha com o título (15 px, negrito) e um `PopupMenu.Switch` desabilitado (`reactive` falso, o estilo `:insensitive` e opacidade 0,4, como o `.switch:disabled`) em todos os aparelhos. A `gnome-panel-led` o habilita.
- Com o daemon no ar, a contagem dá lugar ao carrossel. Com o daemon fora do ar, nada muda: o aviso, "Iniciar halod" e "Ver registro" da `gnome-shell-extension`, sem o carrossel.
- O `indicator.js` ouve `device-changed` e preenche o `body`:
  - sem `led`: o `.locked` do protótipo, um bloco centralizado com o cadeado (`changes-prevent-symbolic`), "Controle em desenvolvimento" e o `lockedText` (requisito "Aparelho sem controle");
  - com `led`: um aviso provisório num bloco, "Os controles do LED chegam na próxima versão do painel." Ele não é requisito e some com a `gnome-panel-led`.
- Cores: o `stylesheet.css` tem as medidas e cores neutras, e `stylesheet-light.css` e `stylesheet-dark.css` fazem `@import` dele e trazem os tokens do protótipo de cada tema (bloco, texto secundário, chip, pontos de página, setas). O Shell carrega a variante do estilo atual e a troca sozinho quando o estilo muda. Os itens de menu não clicáveis, que o Shell esmaece (`:insensitive`), recebem a cor de rótulo do protótipo.
- A cor do LED só entra nos bytes do desenho. O CSS não usa a cor do LED. O switch e o foco seguem o `-st-accent-color` do tema; o ponto da página atual tem a cor de rótulo, como o `.dot.on` do protótipo.
- O Shell destrói o indicador ao sair sem chamar `disable()`, e o `daemon.js` ainda pode mandar um estado (o dono do nome some junto). Depois do sinal `destroy`, `update()` não desenha nada.

### 7. Testes em camadas

| Camada | Como | O que prova |
|---|---|---|
| Lógica | `sh extension/tests/run.sh` | `carousel.test.js`: os cenários "Lista do daemon", "Um aparelho só", "Aparelho desconectado", "Nome muda", "Volta ao início" e "Roda do mouse" (o tempo é argumento). `art.test.js`: o início do texto, o escape do nome, a tabela da decisão 4, e cada SVG decodificado pelo `GdkPixbuf` em 300×300 (com e sem preservar a proporção, a mesma imagem), com o pixel do anel do Waterforce na cor pedida. `view.test.js`: `devicesFrom` e `lastAppliedFrom` |
| Fumaça | `sh scripts/dev/headless-smoke.sh` | A extensão continua `ACTIVE`, sem `JS ERROR` e sem avisos `halo:` no log, com o proxy do `Led` criado contra o `halod` de teste (`LastApplied` vazio); sem o daemon, a ativação do barramento continua sem uso com os três proxies |
| Visual | `scripts/dev/nested-shell.sh --daemon` | O usuário vê o carrossel com os aparelhos reais (só leitura), navega pelas setas, roda, teclas e indicador, e vê o anel neutro (o `halod` de teste nunca aplicou nada) |
| Final | `packaging/install.sh` + sair e entrar, com o aval do usuário | Na sessão real, com o `LastApplied` que o `halod` da sessão tiver |

O anel colorido não tem conferência no Shell sem escrever no aparelho: o `halod` de teste começa com o `LastApplied` vazio, e só um `Apply` o preenche. Por isso a cor é provada pelo teste de pixels, e a conferência visual da cor fica para a `gnome-panel-led`, quando o painel aplicar. Na conferência final, o anel mostra o que o `halod` da sessão já tiver aplicado, se tiver.

### 8. Nada vai ao hardware

Esta change não usa nenhum byte de protocolo: a extensão só chama `Devices()` e lê `LastApplied`, e o `halod` responde de memória (a lista vem do sysfs, que ele só lê). O wrapper do `Led` não declara o `Apply`, então não há como chamá-lo por engano.

### 9. Sysfs falso que não leva a nenhum nó real

Os testes montam um sysfs falso, e o `halod` traduz o nome do hidraw falso em `/dev/<nome>` (`hid/discovery.rs`). A fumaça de hoje chama o Castor3 falso de `hidraw3`, e um teste do `halod` (`primeira_chamada_ja_tem_aparelhos_e_telemetria`) também. Nesta máquina, o `/dev/hidraw3` real é o teclado Redragon (`258a:013b`), hoje `0600 root`. Nada abre esse nó ainda, porque a fumaça e esse teste só leem a lista e a telemetria, mas a `gnome-panel-led` vai pôr um `Apply` na fumaça, e o nome errado faria o `halod` tentar abrir o teclado.

`scripts/dev/fake-sysfs.sh` monta o Castor3 falso em `class/hidraw/hidraw99` e o hwmon, e recusa rodar se `/dev/hidraw99` existir. Com ele, o `halod` acha o Waterforce (o carrossel o mostra com `led`), mas qualquer abertura falha com `NotFound` antes de qualquer escrita. A fumaça passa a usá-lo nesta change, antes de existir qualquer `Apply` no painel, e os testes do Rust trocam os nomes de hidraw que existem nesta máquina (0 a 5) por `hidraw97` a `hidraw99`. O `nested-shell.sh --daemon` continua lendo o `/sys` real nesta change, para mostrar os aparelhos de verdade: o wrapper do `Led` não tem `Apply` (decisão 8). A `gnome-panel-led` passa o nested ao sysfs falso, junto com os controles.

Alternativa: deixar a troca para a `gnome-panel-led`, junto com o primeiro `Apply`. Descartada pelo usuário: nenhum teste deve apontar para um nó que existe, mesmo que hoje não o abra.

## Risks / Trade-offs

- [O `St.Icon` não preservar a proporção do SVG não quadrado] → Aplicado: o SVG é quadrado com a arte centrada (decisão 4), e o ícone fica no tamanho natural dentro de um contêiner que corta a sobra (decisão 5). Conferido no Shell aninhado e na sessão real.
- [Cada cor nova gera uma textura nova no cache do St] → O desenho só é regerado quando o texto muda, e as mudanças de `LastApplied` são raras (uma por aplicação). Se o uso de memória crescer na conferência, guardar o último `Gio.BytesIcon` por texto e reaproveitá-lo.
- [O carregador de SVG do Shell divergir do `GdkPixbuf` do teste] → O teste prova o texto e a cor; a fumaça e o Shell aninhado provam o carregamento no Shell, com os fatos já confirmados (declaração XML seguida de `<svg`, nome sem `-symbolic`).
- [A roda em touchpads mandar muitos eventos `SMOOTH`] → Um passo por sequência, com o limiar e os tempos do protótipo; ajustáveis numa constante se a conferência pedir.
- [Teclas de seta disputadas com o menu] → Só Esquerda e Direita no carrossel (decisão 5).
- [Texto do D-Bus dentro do SVG] → O nome é escapado e a cor validada por expressão regular antes de entrar no texto.
- [A `gnome-panel-led` mexer nos mesmos arquivos] → O contrato da decisão 1 fixa os nomes, e esta change só deixa os pontos de encaixe, sem lógica de LED.

## Migration Plan

1. `packaging/install.sh` (copia a extensão nova) e sair e entrar na sessão.
2. Voltar atrás: reinstalar a partir do commit anterior e sair e entrar, ou `gnome-extensions disable halo@leoodaviid.github.io`.
3. Nada muda no `halod`, nos perfis nem nos aparelhos.
