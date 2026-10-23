# Arquitetura proposta

```text
Barra do GNOME:  [ícone halo]   extensão GNOME Shell (GJS)
                      │          painel: cores, zonas, brilho, efeitos, perfis
                      │ D-Bus (sessão)
                      ▼
               haloD (Rust)      daemon do usuário (systemd --user)
                      │          um módulo por dispositivo, perfis; sobe com a sessão
          ┌───────────┼──────────────┬──────────────┐
          ▼           ▼              ▼              ▼
     IT5702 (HID)  Castor3 (HID)  G502 (HID)   RAM (SMBus, depois)
```

## Por que uma extensão do GNOME

O estilo desejado é de ícones discretos na barra superior que abrem um painel pequeno ao clicar. No GNOME com Wayland, um app comum (Tauri, GTK) não consegue isso: o ícone de bandeja (AppIndicator) só abre um menu de texto simples, e o Wayland não deixa o app posicionar uma janela abaixo do ícone. A extensão GNOME Shell é o caminho nativo e herda o tema do usuário.

Se for preciso uma tela de edição maior (desenhar efeitos por LED, por exemplo), ela pode vir depois como um app GTK4/libadwaita que usa o mesmo D-Bus.

## Painel do GNOME Shell

O comportamento está na spec `gnome-panel`, e o visual no protótipo [`docs/ui/panel-prototype.html`](ui/panel-prototype.html) (ver [docs/ui/README.md](ui/README.md)), que segue o canvas "halo · Apple HIG" com o destaque do GNOME. O painel é só um cliente do `halod`: ele não abre hidraw nem lê o sysfs.

- **Barra:** o ícone e a temperatura, arredondada para o inteiro. A fonte é o líquido (`CoolantMilliCelsius`, `--` quando `CoolantValid` é falso) ou a CPU, escolhida nas preferências.
- **Aparelhos:** o carrossel mostra os aparelhos que o `Devices` lista como presentes, com o nome que o daemon informa, e mostra os controles de LED só para quem tem a capacidade `led`. A extensão não guarda catálogo de hardware.
- **Aplicação:** um `Led.Apply` por gesto concluído (soltar o slider, escolher cor ou efeito, Enter no hex), no máximo um em voo, e só o último valor pendente é enviado. Cada aplicação termina com `B6`, então nada é enviado durante um arraste.
- **Religar:** o último estado ligado desta instância do daemon, depois o LED do perfil ativo e, por fim, `cycle`. O `LastApplied` só existe em memória.
- **Daemon parado:** "Iniciar halod" pede `StartUnit("halod.service", "replace")` ao systemd do usuário (`org.freedesktop.systemd1`, D-Bus da sessão), e "Ver registro" mostra as últimas 20 linhas de `journalctl --user -u halod.service`. As duas ficam desabilitadas quando a unidade não está instalada (`packaging/install.sh`).

Equivalência de cada elemento do protótipo no St (Shell 50):

| No protótipo | Na extensão |
|---|---|
| Indicador com ícone e `38°` numa pílula | `PanelMenu.Button` com um `St.BoxLayout` (ícone simbólico + `St.Label`) |
| Painel em material translúcido | O `menu` do `PanelMenu.Button` com CSS próprio e um `Shell.BlurEffect` em modo `BACKGROUND` (sem blur, se pesar: fundo opaco com a mesma cor) |
| Blocos agrupados | `St.BoxLayout` com fundo e raio no CSS |
| Carrossel com transição | `St.Widget` e `ease()` do Clutter; roda pelo `scroll-event` |
| Desenho do aparelho com o anel tingido | `St.DrawingArea` com Cairo, ou camadas de SVG em que só a camada do LED é simbólica e recebe a cor |
| Swatches | `St.Button` redondos com a cor no estilo inline |
| Barra de matiz | `St.DrawingArea` com gradiente Cairo e um marcador arrastável |
| Campo hex | `St.Entry` |
| Brilho e velocidade no estilo da Central de Controle | Subclasse do `Slider` de `resource:///org/gnome/shell/ui/slider.js` com o `vfunc_repaint` próprio, aplicando no `drag-end` |
| Popup de efeito e menu de perfis | Um `PopupMenu.PopupMenu` próprio ancorado no botão |
| Linha de status e itens de menu | `St.Label` e `PopupMenu.PopupMenuItem` |
| Iniciar halod e ver registro | Proxy de `org.freedesktop.systemd1` e `Gio.Subprocess` com `journalctl` |
| Liga/desliga | O switch do `PopupMenu` |
| Detalhes, Aparelhos e Registro | Troca de página dentro do mesmo menu |
| Copiar comando | `St.Clipboard` |
| Preferências | `Adw.PreferencesWindow` (prefs.js), com a fonte da temperatura em GSettings |

Pré-requisitos no `halod` para a extensão: o `Devices` com nome e capacidades (change `daemon-device-status`) e a temperatura da CPU (`waterforce-cpu-feed`).

## Daemon (`halod`, change `halo-daemon`)

- Crate `crates/halo-daemon`, binário `halod`. Fala com os aparelhos só pelo `halo-core`, sem lógica de protocolo própria, e encontra cada um pelo VID:PID e pela usage page a cada operação.
- D-Bus da sessão com o `zbus` 5 (executor próprio, sem tokio). Nome `io.github.leoodaviid.Halo1`, pedido sem `AllowReplacement` e sem `ReplaceExisting`: uma segunda instância recusa e sai, e não toma o nome da primeira.
- Não precisa de root. O Waterforce precisa da regra udev do halo, como na CLI.

```text
/io/github/leoodaviid/Halo1
  .Manager    prop Version; Devices() -> aa{sv}; signal DevicesChanged(aa{sv})
  .Profiles   List, Get, Save, Delete, Activate; prop ActiveProfile
/io/github/leoodaviid/Halo1/waterforce
  .Waterforce.Led        Apply(s, as, y, y); prop LastApplied
  .Waterforce.Speed      Status, SetPresets(s, s), SetFlatCurve(s, q), Restore(s)
  .Waterforce.Telemetry  props Available, FanRpm, PumpRpm, FanDuty, PumpDuty,
                         CoolantMilliCelsius, CoolantValid
```

- **Lista de aparelhos.** Um `a{sv}` por aparelho, na ordem `waterforce`, `it5702`, `g502`, com os ausentes na lista. As chaves são fixas e sempre têm o mesmo tipo: `id` (`s`), `name` (`s`), `present` (`b`), `devnode` (`s`, vazio quando ausente) e `capabilities` (`as`). Uma chave nova no futuro não quebra os clientes, que ignoram as que não conhecem.
  - **Capacidades** são as interfaces D-Bus que o daemon expõe para o aparelho (`Waterforce.Led` vira `led`), constantes por aparelho e independentes da presença. O Waterforce tem `led`, `speed` e `telemetry`; a placa-mãe e o G502 começam sem nenhuma, e cada change de aparelho acrescenta a sua.
  - **Nomes** sem abrir hidraw: a placa pelo `class/dmi/id/board_name` do sysfs (`Placa-mãe (IT5702)` sem ele), o G502 com o nome fixo `Logitech G502 SE HERO`, e o Waterforce pelo modelo que o worker já leu no `DE 00` de uma operação pedida. Até lá, e depois de cada restart do daemon, ele é `AORUS Waterforce X`. Quando o worker lê um modelo novo da tabela, ele o grava num `KnownModel` compartilhado e pede uma volta dos ticks pelo mesmo canal do resume; o tick monta o nome (`AORUS Waterforce X 240`) e emite o `DevicesChanged` no executor do zbus.
  - **G502** (`046d:c08b`): encontrado pela usage page de fabricante `0xFF00`, a interface do driver Logitech do OpenRGB. O daemon não abre o mouse.
- **Um worker por aparelho.** Uma thread é dona das chamadas bloqueantes ao core do Waterforce e atende um pedido de cada vez, na ordem de chegada. Os métodos D-Bus esperam a resposta por um canal, então o executor do zbus não bloqueia com as esperas de resposta do Castor3, e lista, perfis e telemetria continuam respondendo.
- **Primeira leitura.** Antes de publicar os objetos e pedir o nome, o daemon lê o sysfs uma vez (presença, nomes e hwmon), para que a chamada que o sobe pela ativação do D-Bus já receba a lista e a telemetria.
- **Ticks.** A cada 2 s, o daemon lê só o sysfs: a presença e o nome dos aparelhos (com `DevicesChanged` quando a presença, o caminho ou o nome muda) e a telemetria do hwmon (com `PropertiesChanged` só nas propriedades que mudaram). No fim de uma suspensão (`PrepareForSleep(false)` do logind, no D-Bus do sistema), roda os ticks na hora. Nenhum tick abre hidraw: o `waterforce-cpu-feed` vai acrescentar um que envia o `E0` pelo worker. O daemon não reaplica nada no login nem no resume (ver "Instalação e unidade").
- **Perfis.** TOML em `$XDG_CONFIG_HOME/halo/profiles/`, validados pelos tipos do core e gravados de forma atômica. A ativação da velocidade é um `DesiredState` do core: lê o estado e só escreve (com `B6`) se ele for diferente, com a mesma sequência do `set` e do `curve`.

## Instalação e unidade (change `systemd-user-service`)

`packaging/install.sh` instala tudo no usuário, sem sudo, e `--uninstall` desfaz. O teste dele é `sh packaging/tests/install-test.sh`, com `cargo` e `systemctl` falsos num diretório temporário.

```text
packaging/systemd/halod.service ---(@BINDIR@)--> $XDG_CONFIG_HOME/systemd/user/halod.service
packaging/dbus/...Halo1.service ---(@BINDIR@)--> $XDG_DATA_HOME/dbus-1/services/
target/release/{halo,halod} -------------------> $BINDIR (~/.local/bin)
packaging/udev/70-halo.rules ----(só cmp)------> /etc/udev/rules.d/  (sudo, à mão)

login do GNOME --> graphical-session.target --> halod.service (Type=dbus)
cliente D-Bus  --> dbus-daemon (SystemdService=halod.service) --^
```

- **Unidade:** `Type=dbus` com `BusName=io.github.leoodaviid.Halo1`, então o systemd só a considera pronta quando o nome aparece. `Restart=on-failure` com `RestartSec=2` e o limite padrão de inícios: com o nome tomado por um `halod` iniciado à mão, a unidade tenta algumas vezes e desiste. `PartOf`/`WantedBy=graphical-session.target`: o daemon sobe e desce com a sessão do GNOME, o único cliente.
- **Ativação pelo D-Bus:** com o daemon parado, uma chamada ao nome faz o `dbus-daemon` pedir a unidade ao systemd, mesmo com ela desabilitada.
- **Atualização:** o script compila antes de copiar, troca cada arquivo por um temporário e um `mv`, e faz `restart`, que também inicia o daemon na primeira instalação.
- **Sem reaplicar no login.** O Castor3 guarda a velocidade gravada com `B6` mesmo sem energia (confirmado no aparelho), e no Windows nada reenvia LED nem velocidade no boot: o `AorusLcdService` só manda os dados da tela, e o `ucCooler` só envia quando o usuário aplica. Se o anel perder o LED ao desligar (não testado), a reaplicação vira uma change própria.

## Acesso exclusivo ao hidraw

O Castor3 responde por pedido e resposta: o halo envia `99 <cmd>` e espera um input report que começa com o mesmo comando. Se dois processos conversarem com ele ao mesmo tempo, um pode pegar a resposta do outro. Por isso, `Hidraw::open` (`halo-core`) toma um `flock(LOCK_EX)` no próprio nó `/dev/hidrawN` e espera até 5 s por ele.

- O lock fica no nó do aparelho, e não num arquivo à parte: dois processos que abrem o mesmo `/dev/hidrawN` usam o mesmo inode, então não é preciso um nome de lock por aparelho, e o número do hidraw pode mudar entre boots sem problema, porque o lock é tomado depois da descoberta por VID:PID.
- Ficar em `Hidraw::open` cobre todo comando que abre o hidraw (probes, LED e velocidade), na CLI e no daemon.
- O lock é solto quando o descritor fecha, inclusive se o processo morrer.
- É um lock advisory: só vale entre processos do halo. O driver `gigabyte_waterforce` do kernel não usa o hidraw.

## Regras de segurança

- Só enviar comandos que já estejam documentados (OpenRGB ou análise com o REA). Nada de testar bytes desconhecidos no hardware.
- Começar todo dispositivo novo com uma ferramenta que só lê.
- RAM/SMBus fica fora até haver uma decisão explícita sobre `acpi_enforce_resources=lax`.
