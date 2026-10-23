# Proposal

## Why

O `halod` já está instalado e sobe com a sessão, mas não há nada na tela: o halo só se usa pelo terminal ou pelo `busctl`. O painel aprovado (spec `gnome-panel`, protótipo `docs/ui/panel-prototype.html`) é uma extensão do GNOME Shell, e ela roda dentro do processo da sessão do usuário, o que é novo neste projeto. Esta change é a primeira de três: ela põe a extensão no ar com o mínimo útil (a temperatura na barra e o atalho para o daemon) e prova a base em que as outras duas (`gnome-panel-led` e `gnome-panel-extras`) vão construir: o ciclo de vida dentro do Shell, o cliente D-Bus, a instalação e os testes.

## What Changes

- **Extensão GNOME Shell** em `extension/`, JavaScript puro (ESM, GNOME Shell 50), UUID `halo@leoodaviid.github.io`:
  - indicador na barra com o ícone do halo e a temperatura do líquido em graus inteiros, `--` quando o daemon a marca como inválida e só o ícone sem leitura;
  - o ícone esmaecido quando o `halod` não está no D-Bus;
  - um painel mínimo ao clicar: com o daemon rodando, o estado e quantos aparelhos estão presentes; com o daemon fora do ar, o aviso e as ações "Iniciar halod" e "Ver registro" (as últimas linhas do `journalctl --user -u halod.service`), desabilitadas quando a unidade não está instalada;
  - a extensão **não sobe o daemon sozinha**: ela só observa o nome no D-Bus, e iniciar o daemon é sempre um gesto do usuário. Um `systemctl --user stop halod.service` fica respeitado;
  - segue o estilo claro ou escuro e o destaque do sistema.
- **Lógica pura separada do St** (`extension/lib/`), testada com `gjs -m` fora do Shell, e um teste de fumaça que carrega a extensão num GNOME Shell headless isolado.
- **`packaging/install.sh`** passa a instalar a extensão em `~/.local/share/gnome-shell/extensions/` e a habilitá-la pelo `gsettings` (o `gnome-extensions enable` não enxerga uma extensão copiada com o Shell rodando), avisando que é preciso sair e entrar na sessão só quando a extensão mudou. O `--uninstall` a desabilita e remove.
- **`scripts/dev/nested-shell.sh`**: abre um GNOME Shell aninhado (`--devkit`, do `mutter-dev-bin`) com a extensão do repositório e `HOME`, `XDG_*` e `XDG_RUNTIME_DIR` próprios, para ver o painel sem tocar na sessão do usuário.
- Fora do escopo, nas próximas changes: o carrossel, o desenho dos aparelhos, os controles de LED, perfis, detalhes, erros de aplicação, linha de status, preferências e a fonte CPU na barra (que também depende da `waterforce-cpu-feed`).

## Capabilities

### New Capabilities

Nenhuma. O comportamento do painel já está na spec `gnome-panel`. Esta change implementa os requisitos da barra e do daemon fora do ar, e o painel mínimo é provisório até a `gnome-panel-led`.

### Modified Capabilities
- `gnome-panel`: novo requisito de que o painel não inicia o daemon sozinho, só observa o nome e deixa iniciar o daemon para o gesto do usuário.
- `user-service`: o script de instalação passa a instalar, habilitar, atualizar e remover a extensão.

## Impact

- Novos: `extension/` (`metadata.json`, `extension.js`, `lib/`, `tests/`, `stylesheet.css` e o ícone) e `scripts/dev/nested-shell.sh`.
- `packaging/install.sh` e `packaging/tests/install-test.sh` (com um `gsettings` falso).
- `README.md`, `docs/architecture.md` e `docs/roadmap.md`.
- A extensão roda dentro do `gnome-shell` do usuário: um erro dela aparece no `journalctl --user` do Shell, e um erro em `enable()` desativa só a extensão. A conferência final exige sair e entrar na sessão.
- Hardware: nenhum comando novo. A extensão só lê propriedades e a lista de aparelhos do `halod`, que só leem o sysfs.
- `crates/`: nada muda.
