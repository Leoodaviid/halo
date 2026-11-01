# Proposal

## Why

O pente não guarda o que o halo escreve: depois de um desligamento com corte de energia, ele volta ao rainbow de fábrica (`docs/hardware.md`, "Escritas sem leitura: `off` e `on`"). Numa suspensão S3 ele mantém a cor e fica aceso com o PC dormindo, como o usuário confirmou em 2026-11-01. O FURY CTRL resolve as duas coisas: reaplica o perfil na partida e ao retomar, e apaga os pentes antes de suspender (`FURYController_Service.cs`: `OnStart` → `KeepDramLEDEffectFeature`; `OnPowerEvent` → `setDRAMDDR5_AllOff` ao suspender e `applyDRAMLEDmode(bResume: true)` ao retomar). Esta change traz o mesmo comportamento para o halo, sem depender do `halod` nem de acesso sem root.

## What Changes

- **Estado salvo.** Todo `halo led ram off|on|default|static` que der certo grava o estado em `/var/lib/halo/ram-led`, como o FURY CTRL grava o perfil. O estado separa o efeito (`effect`: `none`, `default` ou `static`, com `color` e `brightness`) de ligado/desligado (`power`), para que `static` → `off` → `on` ainda volte com a cor depois de um corte de energia, e guarda as instâncias de SPD dos pentes (`spd`). `off` e `on` mantêm o efeito anterior. A gravação é atômica. O `--dry-run` não grava. Se a gravação falhar depois das escritas, o comando avisa e sai com código 5.
- **`halo led ram restore`**: reaplica o efeito salvo com as molduras do `static` ou do `default` e, se o estado for desligado, as do `off` em seguida (com `effect=none` e ligado, as do `on`).
  - Sem estado, não faz nada e sai com 0.
  - O arquivo é recusado, sem nenhum acesso ao barramento, se não for do usuário efetivo (o root, nas unidades), se o grupo ou outros puderem gravar nele ou se o conteúdo não passar na mesma validação da CLI.
  - Espera até 30 s que apareçam os SPDs salvos e falha sem escrever se o conjunto de pentes não for o mesmo.
  - Nunca grava o estado.
- **`halo led ram suspend`**: as escritas do `off` (`setDRAMDDR5_AllOff`, o que o FURY CTRL faz ao suspender), só se houver estado salvo, sem gravar.
- **Duas unidades de sistema**, que executam uma cópia do `halo` do root em `/usr/local/libexec/halo/halo`, nunca o `~/.local/bin/halo`:
  - `halo-ram-restore.service`, no boot (`multi-user.target`);
  - `halo-ram-sleep.service`, presa ao `sleep.target`: `suspend` ao entrar na suspensão ou na hibernação e `restore` ao acordar.
- **`packaging/install.sh`** continua sem `sudo`. Ele confere a cópia do root e as duas unidades contra o repositório e, quando faltarem ou estiverem diferentes, mostra os comandos com `sudo` (instalar, `daemon-reload`, `enable`). O `--uninstall` mostra os comandos para desabilitar e remover. O estado em `/var/lib/halo` fica.
- Fora de escopo:
  - o acesso sem root (helper) e a RAM no `halod` e no painel;
  - perfis com a RAM;
  - o modo independente e outros efeitos;
  - perceber mudanças feitas pelo FURY CTRL no Windows (o halo reaplica o estado salvo aqui).

## Capabilities

### New Capabilities
- `ram-persistence`: o estado salvo da RAM, o `restore`, o `suspend` e as unidades de sistema que reaplicam o estado no boot e ao acordar e apagam os pentes ao suspender.

### Modified Capabilities
- `ram-led`: "Entrada recusada antes de qualquer acesso" passa a cobrir o `restore` e o `suspend`, e o "Simulação" passa a cobrir os dois.
- `user-service`: o script de instalação confere as peças de sistema da RAM e mostra os comandos com `sudo`, e a desinstalação mostra os comandos para removê-las.

## Impact

- `crates/halo-core` (`ram`): o estado (formato, gravação atômica, leitura estrita, checagem de dono e de permissão), com testes.
- `crates/halo-cli`:
  - `led ram restore|suspend`;
  - a gravação depois de cada aplicação;
  - o `--state-dir` oculto para testes (padrão `/var/lib/halo`);
  - a espera pelos SPDs;
  - testes em `tests/led.rs`.
- `packaging/systemd/system/` (novo): `halo-ram-restore.service` e `halo-ram-sleep.service`, verificadas com `systemd-analyze verify`.
- `packaging/install.sh` e `packaging/tests/install-test.sh`: a conferência e os comandos com `sudo`.
- Docs: `docs/permissions.md` (por que a cópia do root), `docs/hardware.md` (a suspensão S3 mantém o estado; os ensaios de boot e de suspensão), `README.md` e `docs/roadmap.md` (Fase 5).
