# Proposal

## Why

O painel aprovado (spec `gnome-panel`) mostra no carrossel só o que o `halod` lista, com o nome e o que cada aparelho sabe fazer, e não guarda catálogo de hardware. Hoje o `Manager.Devices()` devolve só `(id, presente, devnode)` para o IT5702 e o Waterforce: sem nome, sem dizer o que é controlável e sem o mouse. Sem isso, a `gnome-shell-extension` não consegue desenhar o carrossel. Esta change é o pré-requisito dela.

## What Changes

- **BREAKING** `Manager.Devices()` e o sinal `DevicesChanged` passam de `a(sbs)` para `aa{sv}`: um dicionário por aparelho com `id`, `name`, `present`, `devnode` e `capabilities`. Os aparelhos ausentes continuam na lista, com `present` falso. Hoje não há cliente além dos testes e do exemplo de `busctl` do README; um campo novo no futuro é só uma chave a mais.
- **Capacidades:** a lista do que o daemon expõe para o aparelho pelo D-Bus. O Waterforce tem `led`, `speed` e `telemetry`. A placa-mãe e o mouse começam com a lista vazia. As changes de aparelho acrescentam itens (a `it5702-static-color` acrescenta `led` à placa), sem mudar a interface.
- **Nomes sem abrir o aparelho:**
  - Placa-mãe: o nome DMI da placa (`/sys/class/dmi/id/board_name`, `B550M AORUS ELITE` neste PC).
  - Waterforce: `AORUS Waterforce X` até o daemon conhecer o modelo, e depois o modelo completo (`AORUS Waterforce X 240`), aproveitando o `DE 00` que toda aplicação de LED e toda leitura de velocidade já enviam. O daemon não abre o aparelho só para isso.
  - Mouse: `Logitech G502 SE HERO`, nome fixo.
- **G502 detectado:** o daemon passa a encontrar o Logitech G502 SE HERO (`046d:c08b`) pela usage page de fabricante `0xFF00`, a interface que o driver Logitech do OpenRGB usa. Só pelo sysfs: nenhum hidraw é aberto e nada é enviado ao mouse.
- `DevicesChanged` passa a sair também quando o nome de um aparelho muda, não só a presença ou o caminho.
- A spec `gnome-panel` troca "controlável" por "com a capacidade `led`". O comportamento do painel não muda.
- Fora do escopo: o probe do G502 (`g502-read-probe`), o controle de LED da placa (`it5702-static-color`), a extensão e a persistência do modelo entre execuções do daemon.

## Capabilities

### New Capabilities

Nenhuma.

### Modified Capabilities
- `daemon-service`: a lista de aparelhos ganha nome, capacidades e o G502, com o formato de dicionário por aparelho; o sinal de mudança passa a cobrir a troca de nome; o nome do Waterforce vem do modelo já lido pelas operações do worker.
- `gnome-panel`: o carrossel e o aviso de aparelho sem controle passam a falar de capacidades (`led`), em vez de status "controlável" ou "detectado".

## Impact

- `crates/halo-daemon`: `bus.rs` (`Manager`, o tipo do aparelho e os testes), `ticks.rs` (a varredura com o G502, o nome DMI e o sinal quando o nome muda) e `worker.rs` (guardar o modelo lido pelas operações de LED e de velocidade).
- `crates/halo-core`: a constante de busca do G502 (`046d:c08b`, usage page `0xFF00`), sem nenhum comando. O modelo já vem nos resultados de hoje (`led::Applied.model` e `Snapshot.model` nas operações de velocidade).
- `README.md` (exemplo de `busctl` e a saída nova), `docs/architecture.md` (a interface `Manager`) e `docs/hardware.md` (o G502 em `hidraw1` com a usage page `0xFF00`).
- Hardware: nenhum comando novo. A placa e o mouse continuam sem ser abertos pelo daemon.
- Regra udev: não muda. Detectar a presença pelo sysfs não precisa de permissão no hidraw.
