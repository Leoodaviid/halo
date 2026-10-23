# Design

## Context

Ver `proposal.md` (Why) e as specs `gnome-panel` e `user-service`. O que restringe o desenho:

- GNOME Shell 50.1 no Wayland, gjs 1.88. Extensões são módulos ESM (`export default class extends Extension`) carregados pelo próprio processo do Shell. No Wayland não há como recarregar o código de uma extensão: só sair e entrar na sessão.
- `halod` (spec `daemon-service`): `io.github.leoodaviid.Halo1` no D-Bus da sessão; `Manager.Devices()` (`aa{sv}`) e o sinal `DevicesChanged`; propriedades de `Waterforce.Telemetry` (`Available`, `CoolantMilliCelsius`, `CoolantValid` e outras) com `PropertiesChanged`. Com a `systemd-user-service`, ele sobe com a sessão e também por ativação do D-Bus.
- systemd do usuário em `org.freedesktop.systemd1` (`/org/freedesktop/systemd1`): `LoadUnit`, `StartUnit`, `ResetFailedUnit`; o objeto da unidade tem `LoadState`.
- Verificado neste PC (2026-10-22, num Shell headless isolado):
  - `gnome-extensions enable <uuid>` termina com "does not exist" (código 2) quando a extensão foi copiada com o Shell já rodando. A chave `org.gnome.shell enabled-extensions`, gravada pelo `gsettings`, funciona sem o Shell.
  - Um Shell headless com `HOME`, `XDG_*` e `XDG_RUNTIME_DIR` próprios carrega extensões da pasta temporária, escreve os `console.log` delas no stderr e responde `gnome-extensions info` com o estado (`ACTIVE`), inclusive depois de desabilitar e habilitar.
  - Um Shell de teste que divide o `XDG_RUNTIME_DIR` com a sessão real cria `/run/user/1000/gnome-shell-disable-extensions`. Esse marcador faz a sessão real desligar todas as extensões no próximo início. Aconteceu uma vez e foi removido.
  - `mutter-dev-bin` 50.1 instalado (`/usr/libexec/mutter-devkit`). O `--devkit` com runtime próprio subiu, mas a janela não foi confirmada, e ele se registra no GDM e no logind do sistema.
- O ícone do protótipo é um SVG de traço com `currentColor`; o Shell recolore ícones simbólicos (`*-symbolic.svg`) com a cor do texto.

## Goals / Non-Goals

**Goals:**
- Toda decisão de exibição em funções puras, testáveis sem o Shell, e uma camada de St fina que só liga sinais e desenha.
- `disable()` desfaz tudo o que `enable()` fez: nenhum sinal, proxy, timer ou subprocesso sobra.
- Nenhum teste toca na sessão do usuário.

**Non-Goals:**
- O desenho final do painel (carrossel, blocos, blur), que vem com a `gnome-panel-led` a partir do protótipo.
- Preferências e GSettings próprios da extensão (`gnome-panel-extras`).
- Publicar no extensions.gnome.org.

## Decisions

### 1. Estrutura

```text
extension/
  metadata.json            uuid halo@leoodaviid.github.io, shell-version ["50"]
  extension.js             Extension: enable() cria o Indicator; disable() o destrói
  indicator.js             PanelMenu.Button: ícone, rótulo, menu mínimo (St, PopupMenu)
  daemon.js                proxies D-Bus do halod e do systemd, sinais -> estado
  lib/
    temperature.js         barText(telemetry) -> "38°" | "--" | ""
    view.js                panelView(state) -> o que o painel mostra
    journal.js             parse/limite das linhas do registro
  icons/halo-symbolic.svg
  stylesheet.css
  tests/
    assert.js              asserções mínimas, sem dependências
    *.test.js              gjs -m extension/tests/<arquivo>
    run.sh                 roda todos e falha se um falhar
```

`lib/` não importa `gi://St`, `resource:///org/gnome/shell/...` nem nada do Shell: só JavaScript e, quando precisar, `GLib`. É o que permite testar com `gjs -m` puro. `tests/` não é instalado.

### 2. Estado único e funções puras

`daemon.js` mantém um objeto de estado e chama um callback a cada mudança:

```text
state = {
  owner: false,                 // o nome tem dono?
  telemetry: { available, coolantMilliCelsius, coolantValid },
  devicesPresent: 0,
  unit: 'unknown' | 'loaded' | 'not-found',
  journal: null | [linhas],
}
```

- `barText(telemetry)`: sem `available` → `""` (só o ícone); `coolantValid` falso → `"--"`; senão `Math.round(mC / 1000) + "°"` (38 400 → `38°`, 38 500 → `39°`).
- `panelView(state)`: `{ kind: 'running', devices: n }`, ou `{ kind: 'down', canStart, canJournal, reason }`, com `canStart`/`canJournal` falsos e o motivo "serviço não instalado" quando `unit` é `not-found`.
- O indicador fica esmaecido (classe CSS) quando `owner` é falso.

Alternativa: espalhar as condições pelo código do St. Descartada: o comportamento da spec ficaria testável só no Shell.

### 3. D-Bus sem ativação

Os proxies do halod (`Manager` e `Waterforce.Telemetry`) são criados com `Gio.DBusProxyFlags.DO_NOT_AUTO_START`, que também cobre a construção (o Gio não ativa o nome ao criar o proxy com essa flag). O dono do nome vem da propriedade `g-name-owner` do proxy, com `notify::g-name-owner`:

```text
g-name-owner vira não nulo --> owner = true, lê as propriedades em cache, chama Devices()
g-name-owner vira nulo     --> owner = false, carrega o estado da unidade (LoadUnit)
g-properties-changed       --> atualiza telemetry
DevicesChanged             --> recalcula devicesPresent
```

Nenhuma leitura periódica: o halod já emite os sinais quando algo muda. As chamadas usam `Gio.Cancellable`, cancelado no `disable()`.

Alternativa: `Gio.bus_watch_name` à parte dos proxies. Descartada: o `g-name-owner` do próprio proxy dá a mesma informação sem um segundo observador.

### 4. Iniciar o serviço e ver o registro

- **Unidade instalada?** `LoadUnit("halod.service")` e a propriedade `LoadState` do objeto devolvido: `not-found` desabilita as duas ações. Consultado quando o dono some e quando o menu abre com o daemon fora do ar.
- **Iniciar:** `ResetFailedUnit("halod.service")` (ignorando o erro de unidade que não está em falha) e depois `StartUnit("halod.service", "replace")`. O painel se atualiza quando o `g-name-owner` aparece, sem tratar o retorno do `StartUnit` como "pronto".
- **Registro:** `Gio.Subprocess` com `journalctl --user -u halod.service -n 20 --no-pager -o short-iso`, lido com `communicate_utf8_async` e um `Cancellable`. O painel mostra as linhas num rótulo só de leitura, com "Voltar". Sem linhas, mostra "Sem registro".

### 5. Ícone, estilo e tema

- `icons/halo-symbolic.svg`: o anel aberto e o ponto do protótipo, convertidos em caminhos preenchidos, que é o que o Shell recolore.
- `stylesheet.css` só com classes da extensão (`.halo-indicator`, `.halo-dimmed`, `.halo-panel`, `.halo-journal`). Cores vêm do tema do Shell e de `-st-accent-color`, então o claro, o escuro e o destaque seguem o sistema sem código.

### 6. Testes em camadas

| Camada | Como | O que prova |
|---|---|---|
| Lógica | `sh extension/tests/run.sh` (`gjs -m`) | Os cenários de barra e de painel da spec, sem o Shell |
| Fumaça | `sh scripts/dev/headless-smoke.sh` | A extensão carrega (`ACTIVE`) num Shell 50 headless, sobrevive a `disable`/`enable` e não deixa `JS ERROR` no log, com e sem um `halod` (`target/debug/halod` com `--sys-root` falso) no barramento do teste |
| Visual | `scripts/dev/nested-shell.sh` (`--devkit`) | O usuário vê e clica na barra e no painel |
| Final | `packaging/install.sh` + sair e entrar | Na sessão real |

Os dois scripts de Shell usam `dbus-run-session` e um diretório temporário com `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME`, `XDG_CACHE_HOME` e `XDG_RUNTIME_DIR` próprios (o `--devkit` recebe `WAYLAND_DISPLAY` com o caminho absoluto do socket da sessão, para abrir a janela). Eles conferem, ao terminar, que `/run/user/$UID/gnome-shell-disable-extensions` não foi criado por eles.

No barramento de teste não há systemd, então o arquivo de ativação de lá não é usado: o `halod` de teste é iniciado pelo próprio script, com `--address` e `--sys-root`.

### 7. Instalação pelo `install.sh`

```text
7. extensão
   staging: copia extension/ sem tests/ para <dest>.new
   mudou?   diff -r <dest>.new <dest>  (ausente conta como mudança)
   troca:   rm -rf <dest>; mv <dest>.new <dest>     (só quando mudou)
   habilita: gsettings get org.gnome.shell enabled-extensions
             -> acrescenta 'halo@leoodaviid.github.io' se faltar -> gsettings set
             falha -> aviso com o comando para habilitar depois
   aviso:   "saia e entre na sessão para carregar a extensão" só quando mudou
uninstall: tira o UUID da lista (mesmo caminho, sem mexer nas outras) e rm -rf <dest>
```

- `<dest>` = `$XDG_DATA_HOME/gnome-shell/extensions/halo@leoodaviid.github.io`.
- A edição da lista é feita sobre o texto do `gsettings get` (formato GVariant `['a', 'b']` ou `@as []`), com `sed`, e o teste cobre lista vazia, lista com outras extensões e UUID já presente.
- Se `org.gnome.shell disable-user-extensions` estiver verdadeiro, o script avisa que as extensões do usuário estão desligadas.
- O teste do script ganha um `gsettings` falso, que guarda a lista num arquivo.

Alternativa: `gnome-extensions enable`. Descartada pelo teste do Context: falha com a extensão recém-copiada, que é o caso normal do script.

### 8. Investigação do `--devkit`

`scripts/dev/nested-shell.sh` abre o Shell aninhado com a extensão do repositório copiada para o temporário e, opcionalmente, um `halod` de teste. Antes de ser usado nas conferências, ele precisa de uma rodada com o usuário: a janela aparece, a extensão está na barra, e a sessão real não perde nada (o marcador de crash não aparece, as extensões do usuário continuam ativas, nenhuma sessão nova fica no `loginctl`). Se o registro no GDM causar efeito na sessão real, o script fica só com o headless e a conferência visual passa a ser a final.

Resultado da investigação (2026-10-22): a sessão real ficou intacta (sem o marcador de crash, sem sessão nova no `loginctl`), mas a janela não abriu. No GNOME 50, o visualizador `mutter-devkit` recebe a imagem do Shell aninhado pelo PipeWire, cujo socket fica no runtime dir, e com o `XDG_RUNTIME_DIR` isolado ele falhava com `Failed to connect pipewire context` e fechava. O script passa `PIPEWIRE_RUNTIME_DIR` com o runtime da sessão real: o PipeWire não guarda estado da sessão, então o isolamento do dconf, das extensões e do marcador continua.

## Risks / Trade-offs

- [Erro da extensão dentro do Shell do usuário] → Lógica testada fora do Shell, fumaça headless antes de instalar, `disable()` testado no ciclo da fumaça. Um erro em `enable()` só desativa a extensão.
- [Atualizar exige sair e entrar] → O script só pede isso quando a extensão mudou. Durante o desenvolvimento, as conferências visuais são no `--devkit`.
- [Script de teste mexendo na sessão real] → `XDG_RUNTIME_DIR` próprio em todo Shell de teste e a verificação do marcador ao terminar.
- [O `--devkit` falar com o GDM e o logind do sistema] → Rodada de investigação com o usuário antes de depender dele (decisão 8).
- [Lista de extensões editada como texto] → O teste cobre os formatos que o `gsettings` devolve; qualquer formato inesperado faz o script avisar e não gravar.
- [Ícone de traço não recolorido] → Convertido para preenchimento na decisão 5.

## Migration Plan

1. `packaging/install.sh`, sair e entrar na sessão.
2. Voltar atrás: `packaging/install.sh --uninstall` (remove também a extensão) ou `gnome-extensions disable halo@leoodaviid.github.io`.
3. Nada muda no `halod` nem nos perfis.
