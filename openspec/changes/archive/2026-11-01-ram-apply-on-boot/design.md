# Design

## Context

- O `halo led ram` já aplica `off`, `on`, `default` e `static` sem ler os pentes (`ram::controller`: `plan()`/`apply()`, com o `Plan` opaco), mas não guarda nada, e o pente volta ao rainbow a cada corte de energia.
- Nesta máquina:
  - o `i2c_piix4` sobe aos 9,5 s do boot e o `ee1004` publica o `0-0052` e o `0-0053` aos 9,81 s, os dois carregados pelo udev (o `i2c-dev` é embutido no kernel);
  - o `basic.target` chega aos 7,7 s, antes dos pentes;
  - a suspensão é S3 (`mem_sleep` = `[deep]`), e numa suspensão de 27 s (2026-11-01, 16:27:55–16:28:22) os pentes ficaram acesos em branco e continuaram brancos ao acordar.
- O `halo` instalado fica em `~/.local/bin/halo`, um arquivo do usuário. O nó `/dev/i2c-0` é do root de propósito (regra 72; `docs/permissions.md`, "SMBus da RAM").
- O FURY CTRL 2.0.66.0 (`FURYController_Service.cs`):
  - na partida (`OnStart`), roda a detecção e o `KeepDramLEDEffectFeature`, que reaplica o perfil salvo;
  - no `OnPowerEvent`, ao suspender (`QuerySuspend`/`Suspend`, L1657–1672), chama o `setDRAMDDR5_AllOff`;
  - ao retomar (`ResumeSuspend`/`ResumeAutomatic`, L1678–1696), refaz a detecção e reaplica com o `applyDRAMLEDmode(bResume: true)` (L1594).

  Toda aplicação vira o perfil salvo.

## Goals / Non-Goals

**Goals:**
- Os pentes voltam ao último estado aplicado no boot e ao acordar, e apagam ao suspender, sem nenhum comando do usuário.
- Nada que rode como root executa ou confia num arquivo que o usuário (ou qualquer processo dele) possa alterar.
- Nenhuma escrita nova no barramento: o `restore` reusa os planos existentes, e o `suspend` é o plano do `off`.

**Non-Goals:**
- Nenhuma leitura do pente para saber o estado atual. O halo confia no estado salvo, como o FURY CTRL confia no perfil.
- O `halod` e o painel não entram. O acesso sem root continua para uma change própria.

## Decisions

- **Unidades de sistema que chamam a CLI existente**, em vez de um daemon ou de um helper. O `halo led ram` já tem as validações, o lock do nó e os planos testados, e um oneshot do root é a menor peça que roda no boot e na suspensão.
  - *Alternativa:* um helper sem root e o `halod` reaplicando. Descartada nesta change porque exige um protocolo novo entre processos. A persistência sai antes dele.
- **Cópia do root em `/usr/local/libexec/halo/halo`.** As unidades executam só ela. O `install.sh` nunca usa `sudo` e só mostra os comandos (`sudo install -D -m 755 …`), como já faz com a regra udev.
  - *Alternativa:* `ExecStart=~/.local/bin/halo`. Descartada: um processo qualquer do usuário trocaria o binário e ganharia root no próximo boot.
- **Estado em `/var/lib/halo/ram-led`**, texto `chave=valor` versionado (`spec ram-persistence`, "Formato do estado"):
  - **Leitura:** o parser é estrito e monta o `RamCommand` pela mesma validação da CLI. A checagem de combinação de `--color`/`--brightness` e do brilho de 0 a 100, que hoje está no `ram_command` da CLI, passa para o core, num construtor do `RamCommand` a partir do nome, da cor e do brilho. A CLI e o estado usam esse construtor.
  - **Dono e permissões:** comparados com o usuário efetivo (`geteuid`), e não com "uid 0". Nas unidades, o efetivo é o root, e nos testes é o usuário que roda o teste. Assim a mesma regra vale nos dois lugares, sem caminho especial.
  - **Gravação:** `ram-led.tmp` no mesmo diretório, `fsync`, `rename` e `fsync` do diretório, com o arquivo em `0644` e o diretório em `0755`.
  - *Alternativa:* `/etc/halo/ram.conf`. Descartada porque o arquivo é escrito pelo programa a cada comando, e isso é estado (`/var/lib`), não configuração editada à mão.
- **Efeito separado de ligado/desligado.** O `on` só escreve índice e brilho (`plan(On)`), sem estilo nem cor, e depois de um corte de energia o pente está no rainbow de fábrica. Guardar só o último comando faria `static` → `off` → `on` voltar em rainbow. Por isso o estado guarda o efeito (`none`, `default` ou `static` com cor e brilho) e o `power`; `off` e `on` mantêm o efeito, e o `on` sobre `static` grava brilho 80, que é o que ele escreve. O `restore` aplica o efeito e, se `power=off`, o `off` em seguida, para que um `on` posterior mostre a cor certa. Achado do juiz externo em 2026-11-01.
  - *Alternativa:* fazer o `on` reaplicar o efeito salvo. Descartada: muda o requisito "Reacender" da `ram-led`, que já foi testado no aparelho.
- **Todo comando que der certo grava**, como o FURY CTRL. A exceção é o `suspend`, que nunca grava; senão, ao acordar, o `restore` reaplicaria o `off`. Assim o estado reaplicado é sempre o último que o usuário viu antes de suspender.
  - *Alternativa:* salvar só com `--save`. Descartada com o usuário (explore de 2026-11-01): ao acordar, voltaria uma cor antiga.
- **Sem estado, não fazer nada.** O `restore` e o `suspend` saem com 0 sem tocar no barramento, e as unidades também têm `ConditionPathExists=/var/lib/halo/ram-led`. Até o primeiro comando, o comportamento é o de hoje. Sem isso, o `suspend` apagaria pentes que nada reacenderia.
- **Espera pelos pentes dentro do `restore`**, e não por uma regra udev:
  - o `restore` refaz o `locate()` a cada 500 ms, por até 30 s, até o conjunto de instâncias ser igual ao `spd` salvo, sem segurar o lock do nó enquanto espera;
  - os erros do `locate()` durante a espera (sem adaptador, sem `ee1004`) contam como "ainda não";
  - o prazo vem de um `--wait-ms` oculto (padrão 30 000), só para os testes.
  - *Alternativa:* `ENV{SYSTEMD_WANTS}` no bind do `ee1004`. Descartada: são dois eventos, a 2 ms um do outro, e o primeiro dispararia o `restore` com um pente só.
- **Conjunto igual, não "contém".** Um pente a mais ou a menos que o salvo faz o `restore` falhar sem escrever: o `0x27` (número de pentes) e os índices dependem do conjunto. Um novo comando do usuário grava o conjunto atual.
- **`halo-ram-sleep.service` com o padrão `StopWhenUnneeded` + `ExecStop`**, que no systemd é a forma de rodar uma ação antes e outra depois do `sleep.target`. O `sleep.target` cobre suspensão, hibernação e híbrido.
  - *Alternativa:* um script em `/usr/lib/systemd/system-sleep/`. Descartada: a documentação do systemd desaconselha esses hooks, e eles ficam fora do `systemctl`.
- **Achados do juiz externo na revisão da branch (2026-11-01):**
  - **O lock do nó vai até o fim da gravação do estado.** O `apply_commands` devolve o `I2cDev`, e o `led_ram` só o solta depois do `save`. Sem isso, dois comandos simultâneos podiam gravar fora de ordem e dividir o `ram-led.tmp`.
  - **`ExecStart=-…suspend`:** o systemd não roda o `ExecStop` de uma unidade que falhou ao subir. Sem o `-`, um `suspend` que falhasse no meio deixaria pentes apagados depois de acordar.
  - **O `save` separa a falha antes do `rename` (`NotSaved`, o antigo fica inteiro) da falha no `fsync` do diretório (`NotDurable`, o novo já está no lugar)**, para a mensagem não dizer que vale o estado anterior quando o novo já foi trocado.
- **Endurecimento das unidades:** `NoNewPrivileges=yes`, `PrivateNetwork=yes`, `ProtectHome=yes` e `ProtectSystem=strict`, porque o `restore` e o `suspend` só leem o estado. O `/dev` e o `/sys` continuam visíveis, porque o halo precisa do `/dev/i2c-0` e do sysfs.
- **Bytes enviados:** nenhum novo.
  - O `restore` usa o `plan()` do efeito salvo e, conforme o `power`, o `plan(Off)` ou o `plan(On)`, com as fontes de "Apagar", "Reacender", "Padrão do FURY CTRL" e "Cor estática" na spec `ram-led`.
  - O `suspend` usa o `plan(Off)`: `0x08`=`0x53`, `0x0B`=índice, `0x20`=`0x00`, `0x08`=`0x44`. São o `setDRAMDDR5_AllOff` do serviço e o `SetKingstonFuryDDR5CmdStart@0x100187b0`, o `MasterSlaveRole@0x100192f0`, o `Brightness@0x1001a2a0` e o `CmdDone@0x100188c0` do `FuryCTRL.dll`, já em `docs/hardware.md`.
- **CLI:** `restore` e `suspend` entram em `RamCommand::NAMES`, mas não são `RamCommand`: são ações que leem o estado e então montam um. Os dois aceitam `--dry-run`; o `--color` e o `--brightness` dão código 2. O `--state-dir` é oculto, como o `--sys-root` e o `--dev-root`.

## Risks / Trade-offs

- [O controlador do pente pode não aceitar escritas logo depois do boot (aos ~10 s) ou do retorno] → O ensaio confere os dois casos. Se falhar, o journal da unidade mostra a escrita que falhou e as 30 tentativas. A correção vira uma pausa medida, numa tarefa nova.
- [Uma suspensão que falha ou é cancelada depois do `suspend`] → O `sleep.target` para do mesmo jeito, o `ExecStop` roda e o `restore` reacende os pentes.
- [O `restore` ao acordar falha (por exemplo, `ENXIO`)] → Os pentes ficam apagados até o próximo comando ou boot, e o erro fica no journal da `halo-ram-sleep.service`. É o custo de apagar ao suspender, que o usuário escolheu, igual ao do FURY CTRL.
- [A cópia do root fica desatualizada em relação ao `~/.local/bin/halo`] → O `install.sh` compara as duas e mostra o comando. O formato do estado é versionado, e uma versão que a cópia não conhece é recusada sem escrever.
- [O FURY CTRL no Windows muda a cor] → No boot seguinte no Linux, o halo reaplica o estado salvo aqui. Esse é o comportamento esperado, e fica documentado.

## Migration Plan

- Até o primeiro `halo led ram` depois desta change, não existe estado, e as unidades não fazem nada.
- Para reverter: `sudo systemctl disable --now halo-ram-restore.service halo-ram-sleep.service`, remover as três peças e, se quiser, o `/var/lib/halo`.
