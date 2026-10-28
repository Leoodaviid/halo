# Proposal

## Why

Desde a `waterforce-cpu-feed`, o `halod` publica a temperatura da CPU no D-Bus (`CpuMilliCelsius`, `CpuValid`), mas a barra só mostra o líquido, que muda pouco. A spec `gnome-panel` já pede a escolha entre líquido e CPU nas preferências ("Fonte da temperatura configurável"), e esse requisito ficou na `gnome-panel-extras`, que junta seis frentes. Esta change separa a fonte CPU e as preferências da extensão, a menor parte de uso diário, e deixa pronta a base de preferências para as próximas.

## What Changes

- Schema GSettings da extensão (`org.gnome.shell.extensions.halo`) com a chave da fonte da temperatura, `coolant` por padrão.
- Janela de preferências (`prefs.js`, GTK4/libadwaita) no visual do protótipo (`docs/ui/panel-prototype.html`, "Preferências do halo"): duas opções, líquido e CPU. A CPU fica desabilitada, com a explicação, enquanto o `halod` não publicar a temperatura da CPU, a não ser que já seja a escolhida.
- A barra passa a seguir a fonte escolhida e troca na hora, sem reabrir o painel nem reiniciar o Shell. Com a CPU: graus inteiros com `CpuValid`, `--` sem ele (o daemon guarda o último valor na falha, que não pode aparecer como atual) e só o ícone quando o daemon não publica a CPU.
- Item "Preferências do halo…" no fim da página principal do painel, que abre a janela de preferências. O item "Detalhes do aparelho…" continua na `gnome-panel-extras`.
- O `packaging/install.sh` compila o schema na pasta da extensão instalada, e os scripts de teste do Shell (`headless-smoke.sh`, `nested-shell.sh`) fazem o mesmo na cópia deles.
- Nenhum comando novo ao hardware e nenhuma mudança no `halod`: a extensão só lê propriedades que já existem.

## Capabilities

### New Capabilities

Nenhuma.

### Modified Capabilities

- `gnome-panel`: "Fonte da temperatura configurável" ganha os cenários da CPU válida, inválida e da troca na hora, e o da opção já escolhida com a CPU indisponível; "Itens de menu do painel" passa a ter só o item de preferências nesta etapa, com os detalhes pendentes da `gnome-panel-extras`. "Temperatura inválida ou ausente" passa a valer também para a CPU.
- `user-service`: "Instalação da extensão do GNOME" passa a instalar o schema compilado e a falhar, sem tocar na extensão instalada, quando faltar o `glib-compile-schemas`.

## Impact

- `extension/`: novos `prefs.js` e `schemas/org.gnome.shell.extensions.halo.gschema.xml`; `metadata.json` com `settings-schema`; `lib/temperature.js` com a fonte; `daemon.js` lendo `CpuMilliCelsius`/`CpuValid` do proxy de telemetria; `indicator.js` com o item de menu e a reação à troca da chave.
- `packaging/install.sh` e `packaging/tests/install-test.sh`: compilação do schema e a ausência do `glib-compile-schemas`.
- `scripts/dev/headless-smoke.sh` e `scripts/dev/nested-shell.sh`: compilam o schema na cópia da extensão. `scripts/dev/fake-sysfs.sh` ganha um hwmon `k10temp` com `Tctl`, para ver a CPU na barra sem tocar no `/sys` real.
- Dependência de tempo de instalação: `glib-compile-schemas` (pacote `libglib2.0-bin`, já presente nesta máquina).
- `docs/roadmap.md`: a `gnome-panel-extras` perde a fonte CPU e as preferências.
