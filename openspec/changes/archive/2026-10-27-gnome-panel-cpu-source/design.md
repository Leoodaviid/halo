# Design

## Context

- O `halod` já publica em `io.github.leoodaviid.Halo1.Waterforce.Telemetry` as propriedades `CpuMilliCelsius` (i), `CpuValid` (b) e `CpuFeedActive` (b), conferidas com `busctl --user introspect` em 2026-10-27 (31875 m°C, válida). Pela spec `daemon-waterforce` ("Temperatura CPU no D-Bus"), a CPU vem do `k10temp` e independe do Waterforce: sem o cooler, `CpuValid` continua verdadeira. Na falha, `CpuMilliCelsius` mantém o último valor com `CpuValid=false`.
- A extensão (`extension/`) não tem preferências: nem `prefs.js`, nem `schemas/`, nem `settings-schema` no `metadata.json`. O `TelemetryProxy` do `daemon.js` só declara `Available`, `CoolantMilliCelsius` e `CoolantValid`. O `lib/temperature.js` (`barText`) só conhece o líquido.
- O protótipo aprovado (`docs/ui/panel-prototype.html`) define a janela "Preferências do halo": seção "Temperatura na barra", duas opções em rádio com os textos "Líquido do Waterforce / Padrão. Muda pouco: segue o líquido do cooler." e "CPU", cujo texto auxiliar é "Temperatura da CPU publicada pelo halod." ou "Indisponível: o halod ainda não fornece a temperatura da CPU.". No protótipo, a CPU fica desabilitada sem publicação, menos quando já é a escolhida (`renderPrefs`). O item "Preferências do halo…" fica no rodapé da página principal, abaixo de uma linha.
- GNOME Shell 50.1 nesta máquina. O Shell não compila schemas ao carregar uma extensão: `Extension.getSettings()` procura `schemas/gschemas.compiled` na pasta dela. O `packaging/install.sh` copia a pasta `extension/` (sem `tests/`) para um staging e compara com a instalada.
- Nenhum byte de protocolo entra nesta change: a extensão só lê propriedades D-Bus que já existem, e o `halod` não muda.

## Goals / Non-Goals

**Goals:**

- Uma base de preferências (schema, `prefs.js`, item no painel) que a `gnome-panel-extras` possa ampliar sem refazer nada.
- A fonte da barra testável como função pura, como o `barText` atual.

**Non-Goals:**

- Detalhes do aparelho, perfis, erros de aplicação e linha de status (`gnome-panel-extras`).
- Mostrar `CpuFeedActive` ou qualquer estado do envio de `E0` na barra ou nas preferências.
- Mudar o `halod` ou o D-Bus.

## Decisions

### 1. Schema GSettings com enum

`extension/schemas/org.gnome.shell.extensions.halo.gschema.xml`, caminho `/org/gnome/shell/extensions/halo/`, chave `temperature-source` do enum `org.gnome.shell.extensions.halo.TemperatureSource` (`coolant`, `cpu`), padrão `coolant`. O `metadata.json` ganha `"settings-schema": "org.gnome.shell.extensions.halo"`, e assim `this.getSettings()` funciona sem argumento na extensão e no `prefs.js`.

Alternativa: chave `s` livre. Descartada: o enum faz o `gsettings` recusar valores errados, e o código não precisa tratar fontes desconhecidas. A escolha fica no dconf do usuário, o que atende ao cenário "Escolha guardada".

### 2. A fonte entra no `barText`

`barText(telemetry, source)` em `lib/temperature.js`. O estado `telemetry` ganha `cpuPublished`, `cpuMilliCelsius` e `cpuValid`:

- `coolant`: o comportamento atual, sem mudança.
- `cpu`: `''` sem `cpuPublished`; `--` com `cpuValid` falso; senão, os graus inteiros arredondados como no líquido. Para a CPU, `available` (que é do Waterforce) não conta, pelo cenário "CPU sem Waterforce".

`cpuPublished` é "o proxy tem `CpuMilliCelsius` em cache". Um `halod` antigo sem a propriedade deixa o cache nulo, e o daemon fora do ar já zera tudo pelo `owner`. Esse é o mesmo critério do protótipo (`cpuMilliCelsius != null`).

Alternativa: usar `CpuFeedActive` como critério de "publicado". Descartada: ele fala do envio ao cooler, não da leitura, e fica falso sem supervisão ou sem Waterforce, quando a temperatura ainda é válida.

### 3. A extensão reage à chave

Em `enable()`, `this._settings = this.getSettings()`, e o indicador recebe a fonte atual. Um `changed::temperature-source` chama de novo o desenho da barra com o último estado, o que atende ao cenário "Troca para a CPU" sem reabrir o painel. Em `disable()`, o sinal é desconectado e `_settings` volta a `null`, como o resto do que o `enable()` cria.

### 4. `prefs.js` com libadwaita e um proxy próprio

`extension/prefs.js`, `ExtensionPreferences.fillPreferencesWindow(window)`: uma `Adw.PreferencesPage` com o grupo "Temperatura na barra" e duas `Adw.ActionRow` com `Gtk.CheckButton` agrupados (rádio). Título e subtítulo vêm do protótipo. A janela segue o tema do sistema (Adw), não as medidas do painel.

A janela de preferências roda em outro processo, sem o estado do `daemon.js`. Por isso o `prefs.js` cria o próprio `Gio.DBusProxy` de `Waterforce.Telemetry`, com `DO_NOT_AUTO_START`, e acompanha `notify::g-name-owner` e `g-properties-changed`. Assim a opção CPU se habilita sem reabrir a janela (cenário "Daemon passa a publicar com as preferências abertas"). O proxy é cancelado no `close-request` da janela.

A regra de sensibilidade e o texto ficam numa função pura, `cpuOption(published, source)` em `lib/prefs.js`, que devolve `{sensitive, hint}` e é testada pelo `gjs`. O `prefs.js` só a aplica. O `lib/` não importa nada do Shell, então o processo das preferências pode carregá-lo.

Alternativa: perguntar ao Shell pelo estado. Descartada: o processo das preferências não tem um canal simples com a extensão, e o D-Bus do `halod` já é a fonte da verdade.

### 5. Item de menu no fim do painel

Um `PopupSeparatorMenuItem` e um `PopupMenuItem` "Preferências do halo…" depois da seção principal. Ele aparece com o daemon no ar e fora do ar, e fica escondido só na página do registro, como o resto da página principal. Ao ativar, o menu fecha e a extensão chama `this.openPreferences()`. As medidas (padding da linha, linha separadora) seguem o `.menu-row` e o `.hr` do protótipo, inline como as outras, porque o tema MacTahoe vence o `stylesheet.css`.

### 6. Schema compilado na instalação e nos scripts de teste

O `install.sh` roda `glib-compile-schemas --strict "$staging/schemas"` antes do `diff` com a instalada. Se o comando não existir, o script para antes de trocar a pasta instalada e diz qual pacote instalar (`libglib2.0-bin`). A saída do compilador depende só do XML, então reinstalar sem mudança continua sem o aviso de sair e entrar. Isso é verificado no `install-test.sh`, com o compilador real.

O `headless-smoke.sh` e o `nested-shell.sh` compilam o schema na cópia deles. O `gschemas.compiled` não vai para o git (`extension/schemas/.gitignore`). Como o `enable()` chama `getSettings()`, a extensão em `ACTIVE` na fumaça prova que o schema compilado foi achado.

### 7. CPU no sysfs falso

O `scripts/dev/fake-sysfs.sh` ganha um hwmon `k10temp` em outro índice, com `temp1_label=Tctl` e `temp1_input=45000`. Com o `nested-shell.sh --daemon`, a escolha da CPU nas preferências passa a barra de `31°` a `45°` sem tocar no `/sys` real. O `halod` de teste roda sem supervisão: publica a CPU e não envia `E0` (spec `daemon-waterforce`, "Sem supervisão"). De qualquer forma, o hidraw falso não leva a nenhum nó real.

## Risks / Trade-offs

- [O `openPreferences()` depende do serviço `org.gnome.Shell.Extensions`, que talvez não suba no barramento isolado do Shell aninhado] → Conferir no Shell aninhado. Se não subir, registrar no design e conferir a janela na sessão real depois da instalação.
- [Com a CPU escolhida, um `halod` antigo deixa a barra só com o ícone sem nenhum aviso] → É o que a spec pede ("sem trocar a fonte sozinha"), e as preferências explicam o motivo.
- [A desinstalação deixa a chave no dconf] → É inofensivo. A spec `user-service` já mantém as configurações do usuário na desinstalação.
- [A barra com a CPU oscila mais que com o líquido] → É a escolha do usuário, e o padrão continua sendo o líquido.

## Migration Plan

Nada a migrar: sem a chave gravada, a fonte é `coolant`, e a barra fica como hoje. A instalação é a de sempre (`packaging/install.sh`, sair e entrar na sessão). Para voltar atrás, basta reinstalar a versão anterior da extensão. A chave órfã no dconf não afeta nada.
