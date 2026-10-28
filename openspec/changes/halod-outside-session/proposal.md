# Proposal

## Why

O `halod` para em todo logout, porque a unidade do usuário tem `PartOf=graphical-session.target`. O acesso ao Castor3 também some sem sessão ativa, porque a regra udev só usa `uaccess`. Fora da sessão, ninguém envia a CPU ao Waterforce, e o aparelho fica no último `E0`: os 80 °C da proteção. As medições de 2026-10-27 ("Bloqueio e logout medidos" em `docs/hardware.md`) mostram a fan em duty 100 durante toda a tela de login, com a CPU entre 34 e 46 °C, e o LCD mostrando 80 °C. A tela bloqueada não tem esse problema. No Windows, o `AorusLcdService` é um serviço do sistema e segue a CPU real antes e depois do login. O usuário já tem o linger ligado e está no grupo `plugdev`, então dá para chegar ao mesmo resultado sem nenhum processo root.

## What Changes

- **BREAKING (comportamento da unidade):** `halod.service` deixa de seguir a sessão gráfica (sem `PartOf=`/`After=graphical-session.target`, `WantedBy=default.target`). Com o linger, ela sobe no boot e não para no logout. Sem o linger, o gerenciador do usuário termina no logout e a unidade para com a proteção, como hoje.
- A regra udev do Castor3 (`1044:7a4d`) ganha `GROUP="plugdev", MODE="0660"`, além do `TAG+="uaccess"`. O IT5702 continua só com `uaccess`.
- Uma unidade nova, `halod-session.service`, presa ao `graphical-session.target`, serve de **barreira** no logout. O `ExecStop` dela chama `Session.End()` no `halod` e espera a resposta. Quando o acesso ao Castor3 depende só da ACL da sessão (linger com a regra antiga), o `halod` envia o `E0` de 80 °C e pausa o feed antes de responder, e a sessão gráfica só termina depois disso. Com o acesso pelo grupo, ele responde sem pausar. A classificação lê a ACL efetiva do nó, e não só os bits do `st_mode`, e qualquer caso não comprovado conta como "só ACL".
- O `packaging/install.sh` migra a habilitação (o `halod.service` sai de `graphical-session.target.wants` e vai para `default.target.wants`; a `halod-session.service` entra em `graphical-session.target.wants`) e passa a conferir o linger e o grupo `plugdev`. Quando faltar algum, ele mostra os comandos, sem executá-los, como já faz com a regra udev.
- A proteção de 80 °C continua em falha, travamento, parada, desligamento e suspensão. Nenhum byte novo vai ao aparelho: os mesmos `E1`/`E0` de hoje (`docs/protocols/castor3.md`).

## Capabilities

### New Capabilities

Nenhuma.

### Modified Capabilities

- `user-service`: "Daemon sobe com a sessão gráfica" sai, e entra "Daemon independente da sessão gráfica" (boot com linger, logout sem parada, degradação sem linger). "Proteção na unidade de usuário" deixa de citar a parada por `PartOf`. "Watchdog ligado ao progresso" passa a aceitar o scan com o feed pausado por sessão. Entram "Guarda da sessão gráfica" e "Conferência do linger e do grupo" (`plugdev`); a "Conferência da regra udev" não muda, só compara com a regra nova. "Atualização" ganha a migração da habilitação.
- `device-permissions`: "Dispositivos cobertos" dá ao Castor3 o grupo `plugdev` com modo `0660`, além do `uaccess`.
- `daemon-waterforce`: novo requisito "Proteção ao perder o acesso da sessão".

## Impact

- `packaging/systemd/halod.service`, a nova `packaging/systemd/halod-session.service`, `packaging/udev/70-halo.rules`, `packaging/install.sh`, `packaging/tests/install-test.sh` e `packaging/tests/systemd-harness.sh` (o alvo de teste no lugar do `graphical-session.target`).
- `crates/halo-daemon`: a interface `io.github.leoodaviid.Halo1.Session` (`Begin`, `End`), o modo `halod --session begin|end`, a classificação do acesso pela ACL efetiva, a leitura do estado inicial da guarda pelo `org.freedesktop.systemd1` e a pausa do feed.
- `docs/permissions.md`, `docs/architecture.md`, `README.md` e `docs/roadmap.md`.
- Instalação: trocar a regra udev exige `sudo`, rodado pelo usuário a partir dos comandos que o script mostra, como hoje. O `install.sh` continua sem sudo.
- Segurança: com a regra nova, qualquer processo do grupo `plugdev` abre o Castor3, mesmo fora da sessão. Num PC de um usuário, isso equivale ao acesso de hoje.
