# Proposal

## Why

Hoje o `halod` só roda com `cargo run`, num terminal aberto. Antes da extensão do GNOME, ele precisa estar instalado no usuário e subir sozinho. O painel (spec `gnome-panel`) já conta com uma unidade `halod.service` para "Iniciar halod" e "Ver registro", e um cliente D-Bus precisa encontrar o daemon sem que ninguém o inicie à mão. Não existe nenhum jeito de instalar, atualizar ou remover o halo: só os comandos da regra udev no `docs/permissions.md`.

## What Changes

- **Script de instalação** `packaging/install.sh`, só com a parte do usuário e sem sudo:
  - compila o `halo` e o `halod` em release, com o `Cargo.lock`;
  - instala os binários em `~/.local/bin`, a unidade em `~/.config/systemd/user/` e o arquivo de ativação D-Bus em `~/.local/share/dbus-1/services/`;
  - recarrega o systemd do usuário, habilita a unidade e (re)inicia o daemon, para que uma atualização troque o binário em uso;
  - confere a regra udev em `/etc/udev/rules.d/70-halo.rules` contra a do repositório e, se ela faltar ou for diferente, mostra os comandos com sudo, sem executá-los;
  - `--uninstall` para e desabilita a unidade e remove o que o script instalou, mas mantém os perfis (`~/.config/halo`) e os snapshots (`~/.local/state/halo`), inclusive o `waterforce-speed.original`.
- **Unidade `halod.service`** do systemd do usuário: `Type=dbus` com o nome `io.github.leoodaviid.Halo1`, reinício em caso de falha e início com a sessão gráfica (`graphical-session.target`).
- **Ativação pelo D-Bus:** uma chamada a `io.github.leoodaviid.Halo1` sobe o daemon pela unidade, mesmo com ela desabilitada.
- **Sem reaplicar perfil no login.** O Castor3 guarda a velocidade gravada com `B6` (confirmado no aparelho, `docs/hardware.md`), e o GCC não reenvia LED nem velocidade no boot nem no resume: o `AorusLcdService` só manda dados da tela, e o `ucCooler` só envia quando o usuário aplica. Reaplicar no login seria um `B6` a mais por login, sem ganho. O requisito "Nada é enviado ao hardware sem pedido" da `daemon-service` continua valendo.
- Documentação: seção de instalação no `README.md`, a unidade em `docs/architecture.md`, o roadmap, e o README da change `gnome-shell-extension` (ainda diz que o `Devices` não tem change).
- Fora do escopo:
  - reaplicar o perfil no login ou no resume, que só volta como change própria se o anel perder o LED ao desligar (não testado: os testes do LED terminaram no `cycle`, que é o padrão de fábrica);
  - instalar a extensão do GNOME (a `gnome-shell-extension` acrescenta essa etapa ao script);
  - instalar a regra udev pelo script, pacotes de distribuição e CI.

## Capabilities

### New Capabilities
- `user-service`: instalação do halo no usuário, sem root (binários, unidade do systemd do usuário e ativação pelo D-Bus), o início do daemon com a sessão gráfica e por ativação, a atualização, a desinstalação que preserva perfis e snapshots, e a conferência da regra udev.

### Modified Capabilities

Nenhuma. A `daemon-service` já proíbe enviar comandos na inicialização, e esta change não muda isso. A `device-permissions` continua com a regra e a instalação documentada; o script só confere.

## Impact

- Novos: `packaging/install.sh`, `packaging/systemd/halod.service`, `packaging/dbus/io.github.leoodaviid.Halo1.service` e um teste do script com `cargo`, `systemctl` e diretórios falsos.
- `crates/halo-daemon`: o `halod` passa a publicar a lista de aparelhos e a telemetria lidas do sysfs antes de pegar o nome (decisão 8 do design), para que a chamada que o sobe pela ativação receba a lista. O resto já combina com `Type=dbus`: o nome é pedido sem fila, e o daemon termina com erro se outra instância o tiver.
- `README.md`, `docs/architecture.md`, `docs/roadmap.md` e `openspec/changes/gnome-shell-extension/README.md`.
- Hardware: nenhum comando novo. Subir o daemon não abre nenhum hidraw.
- Sistema: nada fora do `$HOME` é escrito. A regra udev continua sendo instalada à mão, com sudo.
