# Tasks

Esta change guarda o último `halo led ram` aplicado e o reaplica no boot e ao acordar, e apaga os pentes ao suspender, como o FURY CTRL (`FURYController_Service.cs`: `OnStart`/`KeepDramLEDEffectFeature`, `OnPowerEvent` com o `setDRAMDDR5_AllOff` e o `applyDRAMLEDmode(bResume: true)`). Ela não envia nenhuma escrita nova: o `restore` reusa o `plan()` do efeito salvo e o do `off` ou do `on`, e o `suspend` é o `plan(Off)`, o `setDRAMDDR5_AllOff` já documentado em `docs/hardware.md`. Os comandos com `sudo` e os ensaios no aparelho só rodam no terminal do usuário, com o aval dele.

## 1. Estado no core

- [x] 1.1 Em `crates/halo-core/src/ram/controller.rs`, criar o construtor do `RamCommand` a partir do nome, da cor e do brilho, com a regra que hoje está no `ram_command` da CLI:
  - `static` exige cor;
  - os outros recusam cor e brilho;
  - brilho de 0 a 100 e padrão 80.

  A CLI passa a usá-lo. Verificar com `cargo test -p halo-core ram` (as combinações válidas e as recusadas) e com os testes atuais de `cargo test -p halo-cli --test led ram`, que precisam passar sem mudança.
- [x] 1.2 Criar `crates/halo-core/src/ram/state.rs` com:
  - o `SavedState` (efeito `none`/`default`/`static` com cor e brilho, `power` e instâncias de SPD) e a transição do requisito "Estado depois de cada comando";
  - o `render` e o `parse` estrito do requisito "Formato do estado";
  - o `save(dir)` atômico (`ram-led.tmp`, `fsync`, `rename`, `fsync` do diretório, arquivo `0644`, diretório `0755` criado se faltar);
  - o `load(dir)`, que devolve `None` sem o arquivo e recusa arquivo que não é comum, que não é do `geteuid`, que o grupo ou outros podem gravar, ou cujo diretório falha nas mesmas regras.

  Verificar com testes em `cargo test -p halo-core ram`:
  - ida e volta dos três efeitos com `power=on` e `off`;
  - `static ff0000` brilho 20 → `off` → `on` dá `effect=static`, `ff0000`, brilho 80, `power=on`, e `off` sem estado anterior dá `effect=none`;
  - chave desconhecida (`command=`), repetida e faltando, `version=2`, cor com `effect=default`, brilho 150 e `spd` vazio ou repetido dão inválido;
  - modo `0666` no arquivo e `0777` no diretório dão recusado;
  - um `save` que falha (diretório só de leitura) deixa o arquivo antigo inteiro;
  - o arquivo gravado tem modo `0644`.

## 2. Comandos na CLI

- [x] 2.1 Em `crates/halo-cli/src/main.rs`, gravar o estado depois de cada `off`, `on`, `default` e `static` aplicado com sucesso, no `--state-dir` oculto (padrão `/var/lib/halo`), com as instâncias dos pentes do `Target`. O `--dry-run` não grava. Se a gravação falhar, avisar que os pentes mudaram mas o estado não foi salvo e sair com código 5. Verificar com testes em `crates/halo-cli/tests/led.rs`:
  - `static --color ffffff --dry-run` não cria o arquivo;
  - uma falha de escrita (nó falso que é um arquivo comum) não cria o arquivo.

  O caminho de sucesso, que exige o barramento, fica coberto pelo teste do `save` (1.2) e pelo ensaio 4.2.
- [x] 2.2 Acrescentar `halo led ram restore` e `halo led ram suspend`, com o `--wait-ms` oculto (padrão 30 000):
  - sem estado, os dois dizem isso e saem com 0 sem ler o sysfs;
  - estado recusado sai com 5 sem ler o sysfs;
  - o `restore` espera o conjunto de pentes ser igual ao `spd` salvo, consultando a cada 500 ms e sem segurar o lock, e depois do prazo sai com 3, citando os pentes que faltam ou sobram;
  - o `suspend` aplica o `plan(Off)` sem esperar;
  - nenhum dos dois grava o estado;
  - os dois aceitam `--dry-run` (o `restore` mostra o plano do estado salvo; o `suspend`, o `off`), e `--color`/`--brightness` dão 2 antes de ler o estado.

  Verificar com testes em `crates/halo-cli/tests/led.rs`:
  - sem estado, código 0 com o nó em modo `000`;
  - estado `0666`, código 5 com o nó em modo `000`;
  - `restore --dry-run` de um `static ffffff` ligado mostra `0x31 = 0xFF` nos dois pentes e nenhum `0x20 = 0x00`; de um `static ff0000` desligado, as molduras do estático seguidas da moldura do `off`;
  - com `--wait-ms 300` e só o `0-0052` no sysfs, código 3 citando o `0-0053`;
  - um `0-0053` criado por uma thread 200 ms depois do início faz o `restore` passar da espera (o erro passa a ser o do ioctl no nó falso, código 5, e não "faltam pentes");
  - `restore --color 0000ff`, código 2;
  - o `led ram --help` cita o `restore` e o `suspend`.

## 3. Unidades e instalação

- [x] 3.1 Criar `packaging/systemd/system/halo-ram-restore.service` e `halo-ram-sleep.service` como no requisito "Unidades de sistema", com `ConditionPathExists=/var/lib/halo/ram-led`, `ExecStart`/`ExecStop` só em `/usr/local/libexec/halo/halo` e `NoNewPrivileges`, `PrivateNetwork`, `ProtectHome` e `ProtectSystem=strict`, com comentários que citam o `OnStart` e o `OnPowerEvent` do FURY CTRL. Verificar com `systemd-analyze verify packaging/systemd/system/*.service` (sem erros, com um binário falso no lugar se o verify exigir o executável) e com um `grep` que confirma que nenhuma linha `Exec` aponta para fora de `/usr/local/libexec/halo/`
- [x] 3.2 Em `packaging/install.sh`:
  - a conferência das três peças (cópia do root contra o `halo` compilado; unidades contra o repositório), com `HALO_LIBEXEC_DIR` e `HALO_SYSTEM_UNIT_DIR` para os testes, mostrando os comandos com `sudo` sem executar;
  - no `--uninstall`, os comandos para desabilitar e remover, dizendo que o `/var/lib/halo` fica.

  Verificar com `sh packaging/tests/install-test.sh` estendido:
  - peças ausentes mostram `sudo install`, `sudo systemctl daemon-reload` e `sudo systemctl enable`;
  - binário diferente mostra só a troca dele;
  - peças iguais não mostram nenhum `sudo` da RAM;
  - a desinstalação mostra `sudo systemctl disable` e os `sudo rm`;
  - nenhuma chamada real a `sudo` (o `FAKE_LOG` já confere).
- [x] 3.3 Documentar: `docs/permissions.md` (por que a cópia do root e não o `~/.local/bin`; o estado do root e a recusa por dono e permissão), `README.md` (o estado salvo, o `restore` e o `suspend`, e como instalar as unidades) e `docs/roadmap.md` (Fase 5: a reaplicação feita; o helper sem root e o painel pendentes). Registrar em `docs/hardware.md`, na seção da RAM, que a suspensão S3 mantém o estado e deixa os pentes acesos (2026-11-01, 16:27:55–16:28:22) e o que o FURY CTRL faz no `OnPowerEvent`. Verificar que os comandos citados existem no `halo led ram --help` e na saída do `install.sh`

## 4. Ensaio no aparelho

- [x] 4.1 No terminal do usuário, com o aval dele, instalar as peças com os comandos que o `install.sh` mostrar e conferir:
  - `systemctl is-enabled halo-ram-restore.service halo-ram-sleep.service` dá `enabled` nos dois;
  - `ls -l /usr/local/libexec/halo/halo` mostra `root root`.
- [x] 4.2 No terminal do usuário: `sudo target/debug/halo led ram static --color ffffff`. Conferir que `sudo cat /var/lib/halo/ram-led` tem `effect=static`, `color=ffffff`, `brightness=80`, `power=on` e `spd=0-0052,0-0053`, e que `ls -ld /var/lib/halo /var/lib/halo/ram-led` dá `root`, `0755` e `0644`. O comando é o `static` da spec `ram-led` (`applyDRAMLEDmode` do FURY CTRL)
- [x] 4.3 Suspensão, no aparelho:
  - `systemctl suspend`: os pentes apagam (escritas do `off`, `setDRAMDDR5_AllOff`) e, ao acordar, voltam brancos (o `restore` do estado salvo);
  - `journalctl -b -u halo-ram-sleep.service` mostra o `suspend` e o `restore` com código 0;
  - se o usuário usa hibernação, repetir com `systemctl hibernate`.
- [x] 4.4 Boot, no aparelho: desligar com corte de energia e ligar. Os pentes ficam brancos sem nenhum comando, e `journalctl -b -u halo-ram-restore.service` mostra o `restore` com código 0 e quanto tempo depois do boot ele escreveu. Registrar os resultados da 4.2 à 4.4 em `docs/hardware.md`. Se alguma escrita falhar no boot ou ao acordar, parar e registrar sem acrescentar pausas no escuro

## 5. Verificação

- [x] 5.1 Rodar `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `sh packaging/tests/install-test.sh` e `openspec validate ram-apply-on-boot --strict`, todos passando

## Workflow follow-up

- `/cross-review` da branch antes do sync e do archive.
- Sync das specs `ram-persistence`, `ram-led` e `user-service`, archive e commit, só a pedido.
