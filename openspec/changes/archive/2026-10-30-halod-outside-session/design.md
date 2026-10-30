# Design

## Context

- A unidade `packaging/systemd/halod.service` tem `PartOf=graphical-session.target`, `After=graphical-session.target` e `WantedBy=graphical-session.target`. O journal de 2026-10-27 mostra paradas na instalação, no logout (22:39 e 23:31) e no desligamento (22:57), e nenhuma no bloqueio.
- A regra `packaging/udev/70-halo.rules` dá só `TAG+="uaccess"`. Hoje o `/dev/hidraw2` é `crw-rw----+ root root`, com `user:bumblebee:rw-` vindo da ACL do logind. Sem sessão ativa no assento, a ACL vai para o greeter, e o `halod` do usuário não abre o nó.
- O usuário tem `Linger=yes` e está no grupo `plugdev`. O `halod` reabre o hidraw a cada transação (`Hidraw::open` em `crates/halo-core/src/hid/hidraw.rs`, com `flock`), então a permissão vale em cada envio, e não só no início.
- Medição de referência: "Bloqueio e logout medidos" em `docs/hardware.md`. São ~50 s em duty 100 num logout com 35 s de tela de login.
- Ensaio 5.2 (2026-10-29, 22:41): depois do logout, o GNOME **para o D-Bus da sessão do usuário**. O `gnome-session-shutdown.target` tem `Wants=gnome-session-restart-dbus.service`, que roda `gnome-session-ctl --restart-dbus` (`StopUnit dbus.service`), sem condição, em todo logout; é uma gambiarra para derrubar os serviços ativados pelo barramento (bug 764029 do GNOME). O linger mantém o `systemd --user`, mas não o barramento. Com a conexão morta, cada tick do `halod` falhou com `Broken pipe` antes do envio da CPU e do `WATCHDOG=1`, e o watchdog o matou 20 s depois.
- Bytes: nenhum novo. O feed continua com `E1` e `E0` (`docs/protocols/castor3.md`, `AorusLcdService` `CoolerDataApi.SendCpuName`/`SendCoolerData`, decompilados com o ilspycmd), com o mesmo preenchimento de 6144 bytes.

## Goals / Non-Goals

**Goals:**

- Na tela de login, depois do logout e no boot até o login, o Waterforce segue o Tctl como na sessão ativa.
- Nenhuma situação nova em que o aparelho fique preso num Tctl frio sem a proteção.
- Continuar sem nenhum processo root e com o `install.sh` sem sudo.
- O `halod` atravessa o logout do GNOME como o mesmo processo, mesmo com o D-Bus da sessão reiniciando, sem `E0` de 80 °C no LCD quando o acesso é pelo grupo.

**Non-Goals:**

- Vários usuários no mesmo PC. Outro usuário com o halo instalado teria o próprio `halod`, e os dois disputariam o Castor3. Fica registrado como limite.
- O IT5702: o halo ainda não escreve nele, então ele não precisa de acesso fora da sessão.
- Mudar a janela de 8 s, o período de 2 s ou o valor de 80 °C.
- O LCD.

## Decisions

### 1. Unidade no `default.target`, com o linger

A unidade perde o `PartOf=` e o `After=graphical-session.target`, e passa a ter `WantedBy=default.target`. O `Type=dbus` vira `Type=notify` (decisão 6). O resto continua igual: `BusName=`, `ExecStopPost`, watchdog, `TimeoutStopSec` e `StartLimit`. Com o linger, o gerenciador do usuário sobe no boot, junto com o barramento da sessão do usuário (`/run/user/<uid>/bus`), e o `halod` adquire o nome sem nenhuma sessão gráfica. Sem o linger, o gerenciador termina no logout da última sessão e para a unidade, que passa pelo `ExecStopPost`. É o comportamento de hoje, então não há regressão para quem não tem linger.

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
  - **Watchdog durante a pausa:** o `Feed::progressed` hoje só renova com envio novo aceito, aparelho ausente ou modelo sem feed. Pausado, com o Waterforce presente, o daemon seria morto a cada 20 s. A pausa por sessão passa a contar como progresso no scan concluído, como a ausência e o modelo sem feed (delta em "Watchdog ligado ao progresso" da `user-service`). A pausa só conta depois de a proteção ser aceita: o feed guarda se o último `E0` aceito foi um Tctl, e, pausado com ele (a proteção do `End()` ainda na fila, com o worker preso, ou falhando), o scan não renova, e o watchdog mata o processo, liberando o `flock` para o `ExecStopPost` (achado do `/cross-review`, 2026-10-30). A pausa do boot renova, porque o processo ainda não mandou nenhum Tctl. Depois da proteção aceita, um worker travado durante a pausa não é detectado, mas nada frio é enviado nesse estado: o último `E0` é o de 80 °C. Depois do `Begin`, volta a exigir envios aceitos (achado do juiz externo).
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

### 6. D-Bus da sessão que reinicia no logout

O `halod` precisa sobreviver à queda do barramento. Reiniciar o processo funcionaria, mas cada reinício passa pelo `E0` de 80 °C do `ExecStopPost` e pela janela de 8 s do início: ~10–12 s com o LCD em 80 °C e a fan em 100 % a cada logout. Por isso, o daemon reconecta no mesmo processo, e o reinício fica só como rede de segurança.

- **Feed independente do barramento:** num tick, o envio da CPU, a reclassificação do acesso e o `WATCHDOG=1` rodam mesmo quando a emissão de um sinal falha. O erro do barramento só é registrado. O watchdog continua preso ao progresso do feed, e não ao D-Bus.
- **Detecção:** `Connection::closed()` do zbus avisa o fim da conexão (desconexão do outro lado ou erro de E/S). O daemon registra "D-Bus da sessão caiu; reconectando" uma vez, e não a cada tick.
- **Reconexão:** uma tentativa por segundo com `connection::Builder::session()`. O socket do usuário (`dbus.socket`) continua de pé e ativa o `dbus.service` de novo. A conexão nova publica os mesmos objetos e pede o nome com as mesmas regras (`claim_name`). O estado que só existe em memória nas interfaces (o `LastApplied` do LED, o perfil ativo, a lista de aparelhos e a telemetria) é copiado da conexão antiga, cujo `ObjectServer` local continua legível depois da queda. Os ticks passam a usar a conexão nova.
- **Prazo e rede de segurança:** sem nome adquirido em 10 s, o daemon registra o motivo e sai com falha. O `ExecStopPost` manda o `E0` de 80 °C, e o `Restart=on-failure` sobe outro processo, que faz o caminho normal do início. Nome tomado por outro processo também conta como falha. O pior caso da reconexão é, portanto, o reinício.
- **`Type=notify`:** com `Type=dbus`, quando o barramento volta o systemd faz `GetNameOwner` do `BusName` (systemd 259, `bus_setup_api` → `unit_install_bus_match`); se o nome ainda não tem dono, `service_bus_name_owner_change` leva a unidade a `service_enter_stop` com sucesso, e o `Restart=on-failure` **não** reinicia. Seria a tela de login sem `halod`. Com `Type=notify`, o systemd não age sobre o nome; o daemon manda `READY=1` só depois de adquirir o nome na primeira conexão, então "pronto quando o nome aparece" continua valendo. O `BusName=` fica, porque a ativação D-Bus (`SystemdService=`) e a documentação o usam. O `READY=1` depende só do `NOTIFY_SOCKET`, e não da validade do watchdog: com o ambiente do watchdog incompatível, o daemon continua servindo leituras, e a unidade precisa ficar ativa (sem o `READY=1`, o systemd a mataria no `TimeoutStartSec`). Sem `NOTIFY_SOCKET` (execução à mão), nada é enviado.
- **Reconciliação depois de reconectar:** o daemon relê o `ActiveState` da guarda, como no início. Se a guarda estiver ativa e o feed pausado por sessão, ele retoma como o `Begin()` faria. Isso cobre um `Begin()` que chegue durante a reconexão e falhe.
- **Fora desta decisão:** desabilitar ou mascarar o `gnome-session-restart-dbus.service` (mudança no sistema, que o GNOME espera que exista) e usar o barramento do sistema.

Alternativas: `PartOf=`/`BindsTo=dbus.service` e o reinício como caminho principal. Ambas foram descartadas porque mostram 80 °C no LCD a cada logout. A reconexão com `Type=dbus` também, pela corrida do `GetNameOwner`.

## Risks / Trade-offs

- [A reconexão copia o estado errado ou perde parte dele] → Testes com o barramento do harness derrubado e reiniciado: o mesmo nome, as mesmas propriedades e o perfil ativo depois da reconexão. Um erro aqui afeta a extensão, e não o cooler, porque o worker e o feed são os mesmos.
- [`Type=notify` com o `READY=1` antes do nome] → O `READY=1` sai só depois do `build()` com `claim_name` concluído; um teste do binário confere a ordem.
- [Barramento morto sem ninguém notar] → O prazo de 10 s tira o caso: sem reconectar, o processo sai pela proteção.
- [O `halod` reativa o D-Bus logo depois de o GNOME derrubá-lo] → Os serviços que a gambiarra do GNOME quer derrubar já morreram com o barramento antigo. Com o linger, o barramento já sobe sem sessão no boot hoje.

- [A barreira depende de o logind só trocar a sessão ativa do assento (e a ACL) depois de o `graphical-session.target` parar] → É a mesma premissa da proteção de hoje (o `PartOf` do `halod.service`), que funcionou nos logouts medidos em 2026-10-27. O ensaio 5.2 confere no journal que a resposta do `End` vem antes do fim da sessão no logind. O instalador avisa sobre a regra antiga, e a regra nova tira o caso.
- [O `halod` reiniciando (`Restart=on-failure`) bem no meio do logout] → A guarda não acha o daemon e cai no helper, e o daemon novo começa pausado, porque a guarda já não está ativa.
- [O `halod` rodando sem sessão gráfica e sem nenhum cliente: o LED e a velocidade só mudam por pedido, então nada é enviado além do feed] → Isso já é a regra "Nada é enviado ao hardware sem pedido" da `daemon-service`.
- [Grupo `plugdev` abre o Castor3 para qualquer processo do grupo] → É um PC de um usuário, e o `flock` exclusivo do halo continua serializando quem usa o nó.
- [Um segundo usuário com o halo] → Fora de escopo, mas registrado no `docs/architecture.md`.
- [O daemon passa a viver horas sem sessão] → O watchdog e o `Restart` continuam valendo. O journal do usuário continua acessível.

## Migration Plan

1. O usuário instala a regra nova com os comandos que o script mostra (`sudo install`, `udevadm control --reload`, `udevadm trigger`) e reinicia o PC. Medido em 2026-10-29: num nó que já tem a ACL da sessão, o `MODE="0660"` vira um `chmod` que muda só a máscara, e a entrada do grupo fica `group::---`; o grupo só vale num nó criado de novo (boot ou replug). O script e o `docs/permissions.md` pedem o reinício.
2. `packaging/install.sh`: para a unidade antiga (com a proteção), troca o par binário/unidade, migra a habilitação e sobe o daemon.
3. Confere o linger e o grupo (já em dia nesta máquina).
4. Para voltar atrás: primeiro `packaging/install.sh --uninstall` desta versão, que para e desabilita as duas unidades, e conferir que `~/.config/systemd/user/default.target.wants/halod.service` e `graphical-session.target.wants/halod-session.service` não existem mais. O instalador antigo só faz `enable` e não remove esses links. Depois, reinstalar a versão anterior, que volta a habilitar pelo `graphical-session.target`. A regra antiga pode voltar com os mesmos comandos. O grupo `plugdev` e o linger podem ficar como estão. O `docs/permissions.md` e o `README.md` trazem esse roteiro.
