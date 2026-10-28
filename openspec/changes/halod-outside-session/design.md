# Design

## Context

- A unidade `packaging/systemd/halod.service` tem `PartOf=graphical-session.target`, `After=graphical-session.target` e `WantedBy=graphical-session.target`. O journal de 2026-10-27 mostra paradas na instalação, no logout (22:39 e 23:31) e no desligamento (22:57), e nenhuma no bloqueio.
- A regra `packaging/udev/70-halo.rules` dá só `TAG+="uaccess"`. Hoje o `/dev/hidraw2` é `crw-rw----+ root root`, com `user:bumblebee:rw-` vindo da ACL do logind. Sem sessão ativa no assento, a ACL vai para o greeter, e o `halod` do usuário não abre o nó.
- O usuário tem `Linger=yes` e está no grupo `plugdev`. O `halod` reabre o hidraw a cada transação (`Hidraw::open` em `crates/halo-core/src/hid/hidraw.rs`, com `flock`), então a permissão vale em cada envio, e não só no início.
- Medição de referência: "Bloqueio e logout medidos" em `docs/hardware.md`. São ~50 s em duty 100 num logout com 35 s de tela de login.
- Bytes: nenhum novo. O feed continua com `E1` e `E0` (`docs/protocols/castor3.md`, `AorusLcdService` `CoolerDataApi.SendCpuName`/`SendCoolerData`, decompilados com o ilspycmd), com o mesmo preenchimento de 6144 bytes.

## Goals / Non-Goals

**Goals:**

- Na tela de login, depois do logout e no boot até o login, o Waterforce segue o Tctl como na sessão ativa.
- Nenhuma situação nova em que o aparelho fique preso num Tctl frio sem a proteção.
- Continuar sem nenhum processo root e com o `install.sh` sem sudo.

**Non-Goals:**

- Vários usuários no mesmo PC. Outro usuário com o halo instalado teria o próprio `halod`, e os dois disputariam o Castor3. Fica registrado como limite.
- O IT5702: o halo ainda não escreve nele, então ele não precisa de acesso fora da sessão.
- Mudar a janela de 8 s, o período de 2 s ou o valor de 80 °C.
- O LCD.

## Decisions

### 1. Unidade no `default.target`, com o linger

A unidade perde o `PartOf=` e o `After=graphical-session.target`, e passa a ter `WantedBy=default.target`. O resto continua igual: `Type=dbus`, `ExecStopPost`, watchdog, `TimeoutStopSec` e `StartLimit`. Com o linger, o gerenciador do usuário sobe no boot, junto com o barramento da sessão do usuário (`/run/user/<uid>/bus`), e o `halod` adquire o nome sem nenhuma sessão gráfica. Sem o linger, o gerenciador termina no logout da última sessão e para a unidade, que passa pelo `ExecStopPost`. É o comportamento de hoje, então não há regressão para quem não tem linger.

Alternativa: uma unidade do sistema só para o feed. Descartada pelo usuário em 2026-10-27, por exigir um componente root e a coordenação entre dois processos.

### 2. Castor3 com `GROUP="plugdev", MODE="0660"`

Só a linha do Castor3 muda, e o `uaccess` continua. O `plugdev` é o grupo padrão do Ubuntu para dispositivos USB do usuário, e o usuário já está nele. Um grupo próprio (`halo`) exigiria `groupadd` e `usermod`, e mais um logout para valer, sem ganho num PC de um usuário. A instalação da regra continua manual, com os comandos que o script mostra.

### 3. Acesso só pela ACL: barreira no fim da sessão gráfica

Se o `halod` roda com o linger e a regra antiga, ele continuaria vivo no logout e perderia a ACL quando o greeter assumisse o assento. A partir daí, cada `E0` falharia, e o aparelho ficaria no último Tctl frio, sem proteção. Reagir a um sinal (o `PropertiesChanged` do alvo ou da sessão no logind) não basta: o envio da proteção é assíncrono e pode esperar o worker ou o `flock`, enquanto a sessão termina e a ACL é revogada (achado do juiz externo, 2026-10-27). A proteção precisa de uma **barreira**: a sessão gráfica não pode terminar antes de o `E0` de 80 °C ser aceito.

- **Classificação do acesso:** o acesso é "pelo grupo" só quando a permissão de leitura e escrita vem do grupo dono do nó, e o processo tem esse grupo (`getgid`/`getgroups`). Sem ACL estendida, vale o grupo do `st_mode`. Com ACL estendida (`system.posix_acl_access`), os bits de grupo do `st_mode` são a máscara, e não o grupo. Então a decisão lê a ACL: a entrada `ACL_GROUP_OBJ` combinada (AND) com a `ACL_MASK` precisa ter `rw`. Entradas nominais de usuário (a do logind) e de grupo não contam. Qualquer caso não comprovado (xattr ilegível, formato inesperado, erro no `stat`) conta como "só ACL". A decisão é refeita a cada geração do aparelho, porque a regra pode mudar com o daemon rodando.
- **A barreira:** uma unidade nova, `halod-session.service`, com `PartOf=graphical-session.target`, `After=graphical-session.target`, `WantedBy=graphical-session.target`, `Type=oneshot` e `RemainAfterExit=yes`. Pela ordem do systemd, uma unidade com `After=` no alvo para **antes** dele, e o stop do alvo espera o `ExecStop` terminar. É o mesmo mecanismo que garante a proteção hoje, com o `PartOf` do `halod.service`.
  - `ExecStart=halod --session begin`: chama `io.github.leoodaviid.Halo1.Session.Begin()` no `halod`, que retoma o feed (E1, janela de 8 s) se estava pausado por sessão.
  - `ExecStop=halod --session end`: chama `Session.End()` e **espera a resposta**. Com acesso só pela ACL, o `halod` descarta a amostra pendente, envia o `E0` de 80 °C pelo caminho da proteção do SIGTERM, pausa o feed e só então responde. Com acesso pelo grupo, ele responde na hora, sem pausar. Se o `halod` não responder em 10 s, ou se não estiver no barramento, o comando cai no `halod --waterforce-cpu-safe`, o mesmo helper do `ExecStopPost`, e termina com código de falha. O `TimeoutStopSec` vale para a parada inteira da guarda, então ele precisa caber a chamada e o helper em sequência: `TimeoutStopSec=35s` = 10 s do `End` + 18 s do orçamento do helper (com a espera de 8 s e o `flock`) + 7 s de margem (achado do juiz externo). O prazo de 10 s cobre a espera de 5 s pelo `flock` e o envio. Esses 35 s só pesam no caminho de falha: no normal, o `End` responde em menos de 1 s.
  - **`Begin` idempotente:** o `Begin` só abre uma sessão lógica nova (E1 e janela de 8 s) se o feed estiver pausado por sessão. Fora dela (acesso pelo grupo, ou guarda reiniciada sem logout), ele só responde (achado do juiz externo). Assim, num login com a regra nova, nenhum `E0` de 80 °C é enviado.
  - **Watchdog durante a pausa:** o `Feed::progressed` hoje só renova com envio novo aceito, aparelho ausente ou modelo sem feed. Pausado, com o Waterforce presente, o daemon seria morto a cada 20 s. A pausa por sessão passa a contar como progresso no scan concluído, como a ausência e o modelo sem feed (delta em "Watchdog ligado ao progresso" da `user-service`). Um worker travado durante a pausa não é detectado, mas nada frio é enviado nesse estado: o último `E0` é o de 80 °C. Depois do `Begin`, volta a exigir envios aceitos (achado do juiz externo).
  - **Estado inicial:** ao subir, o `halod` lê uma vez o `ActiveState` da `halod-session.service` (`org.freedesktop.systemd1` no D-Bus da sessão). Se ela não estiver ativa e o acesso for só pela ACL, ele começa pausado (boot pelo linger, antes de qualquer login). As transições depois vêm só do `Begin`/`End`.
- **Tela bloqueada:** não mexe no alvo, e nada muda.

Alternativas: o `PropertiesChanged` do alvo ou de `Active` no logind, e o `EACCES` no `open`. Todas foram descartadas porque nenhuma segura a sessão até a proteção ser enviada.

### 4. Instalador

- **Migração:** `systemctl --user disable halod.service` antes de copiar a unidade nova, e `enable halod.service halod-session.service` depois. Depois de subir o `halod`, se o `graphical-session.target` estiver ativo (`systemctl --user is-active`), o script também faz `start halod-session.service`, porque um `enable` não inicia uma unidade presa a um alvo que já está ativo. Sem isso, na primeira instalação dentro de uma sessão, a guarda ficaria inativa até o próximo login, e, com a regra antiga, o daemon começaria pausado e sem barreira no primeiro logout (achado do juiz externo). Numa reinstalação com a guarda já ativa, o `start` não faz nada, e o `halod` novo lê a guarda como ativa. Isso remove o link em `graphical-session.target.wants/halod.service`, que o `[Install]` antigo criou, e cria `default.target.wants/halod.service` e `graphical-session.target.wants/halod-session.service`. O script confere os três links.
- **Linger:** `loginctl show-user "$(id -un)" -p Linger --value`. Se vier `no`, o script avisa e mostra `loginctl enable-linger <usuário>`. Se o `loginctl` não existir ou falhar, avisa que não deu para conferir.
- **Grupo:** `id -nG` sem `plugdev` mostra `sudo usermod -aG plugdev <usuário>` e avisa que vale depois de sair e entrar.
- A conferência da regra udev continua comparando com a do repositório. Uma regra antiga instalada aparece como desatualizada, com os comandos.
- O `--uninstall` para e desabilita as duas unidades e remove os arquivos delas.

### 5. Testes sem o aparelho

- `packaging/tests/install-test.sh`: a migração (link antigo removido, novo criado), o `loginctl` falso com `Linger=yes`/`no` e falhando, e o `id` falso com e sem `plugdev`.
- `packaging/tests/systemd-harness.sh`: as unidades de teste geradas das reais. Parar o alvo de teste não para o daemon falso; parar o alvo espera o `ExecStop` da guarda (o daemon falso demora de propósito para responder ao `End`, e o journal mostra o alvo parado só depois da resposta); e, com o daemon falso fora do ar, a guarda cai no helper.
- `crates/halo-daemon`: testes unitários da classificação (sem ACL; ACL com `group::r--`, `user:<u>:rw-` e `mask::rw-`, que precisa dar "só ACL", que é o caso do achado; ACL com `group::rw-` e a máscara `rw-`; grupo que o processo não tem; xattr ilegível) com xattrs montados nos testes, e da máquina de estado do feed (o `End` só responde depois do `E0` de 80 °C aceito; nenhum `E0` com Tctl depois do `End`; `Begin` com E1 e janela de 8 s; `End` com acesso pelo grupo responde sem pausar; estado inicial pausado), com o relógio e o transporte falsos que o `feed.rs` já usa.

## Risks / Trade-offs

- [A barreira depende de o logind só trocar a sessão ativa do assento (e a ACL) depois de o `graphical-session.target` parar] → É a mesma premissa da proteção de hoje (o `PartOf` do `halod.service`), que funcionou nos logouts medidos em 2026-10-27. O ensaio 5.2 confere no journal que a resposta do `End` vem antes do fim da sessão no logind. O instalador avisa sobre a regra antiga, e a regra nova tira o caso.
- [O `halod` reiniciando (`Restart=on-failure`) bem no meio do logout] → A guarda não acha o daemon e cai no helper, e o daemon novo começa pausado, porque a guarda já não está ativa.
- [O `halod` rodando sem sessão gráfica e sem nenhum cliente: o LED e a velocidade só mudam por pedido, então nada é enviado além do feed] → Isso já é a regra "Nada é enviado ao hardware sem pedido" da `daemon-service`.
- [Grupo `plugdev` abre o Castor3 para qualquer processo do grupo] → É um PC de um usuário, e o `flock` exclusivo do halo continua serializando quem usa o nó.
- [Um segundo usuário com o halo] → Fora de escopo, mas registrado no `docs/architecture.md`.
- [O daemon passa a viver horas sem sessão] → O watchdog e o `Restart` continuam valendo. O journal do usuário continua acessível.

## Migration Plan

1. O usuário instala a regra nova com os comandos que o script mostra (`sudo install`, `udevadm control --reload`, `udevadm trigger`).
2. `packaging/install.sh`: para a unidade antiga (com a proteção), troca o par binário/unidade, migra a habilitação e sobe o daemon.
3. Confere o linger e o grupo (já em dia nesta máquina).
4. Para voltar atrás: primeiro `packaging/install.sh --uninstall` desta versão, que para e desabilita as duas unidades, e conferir que `~/.config/systemd/user/default.target.wants/halod.service` e `graphical-session.target.wants/halod-session.service` não existem mais. O instalador antigo só faz `enable` e não remove esses links. Depois, reinstalar a versão anterior, que volta a habilitar pelo `graphical-session.target`. A regra antiga pode voltar com os mesmos comandos. O grupo `plugdev` e o linger podem ficar como estão. O `docs/permissions.md` e o `README.md` trazem esse roteiro.
