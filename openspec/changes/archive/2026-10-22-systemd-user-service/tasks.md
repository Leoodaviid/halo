# Tasks

## 1. Unidade e ativação

- [x] 1.1 Criar `packaging/systemd/halod.service` e `packaging/dbus/io.github.leoodaviid.Halo1.service` como na decisão 3 do design, com `@BINDIR@` no lugar do caminho. Verificar trocando o `@BINDIR@` por um caminho temporário e rodando `systemd-analyze --user verify` na unidade gerada, sem erros (só lê o arquivo, nada é carregado no systemd)

## 2. Script de instalação

- [x] 2.1 Criar `packaging/install.sh` (`sh`, `set -eu`) com a instalação da decisão 1: compilar com `cargo build --release --locked -p halo-cli -p halo-daemon` antes de tocar em qualquer arquivo instalado, gerar e instalar a unidade e a ativação, instalar os binários com `install -m 755`, e chamar `systemctl --user daemon-reload`, `enable` e `restart`. Respeitar as variáveis da decisão 2, mostrar no início para onde vai instalar e avisar se o `BINDIR` não está no `PATH`. Verificar com o teste da task 2.4 (cenários "Primeira instalação", "Nada fora do usuário", "Compilação falha" e "Reinstalar com o daemon rodando" da spec `user-service`)
- [x] 2.2 Acrescentar a conferência da regra udev (decisão 6): `cmp` com `$HALO_UDEV_RULES_DIR/70-halo.rules`, aviso e os comandos do `docs/permissions.md` quando ela falta ou difere, sem chamar `sudo` nem `udevadm`, e a instalação terminando com sucesso. Verificar com o teste da task 2.4 (cenários "Regra ausente", "Regra desatualizada" e "Regra em dia")
- [x] 2.3 Acrescentar `--uninstall` e `--help`: `systemctl --user disable --now halod.service` (sem falhar se a unidade não existe), remover só os binários, a unidade e a ativação, `daemon-reload` e a mensagem com os caminhos dos perfis e snapshots mantidos. Verificar com o teste da task 2.4 (cenários "Desinstalar", "Snapshot original preservado" e "Desinstalar sem nada instalado")
- [x] 2.4 Criar `packaging/tests/install-test.sh` como na decisão 7: `cargo` e `systemctl` falsos no `PATH`, todas as variáveis num diretório temporário, e as verificações dos arquivos, da ordem das chamadas ao `systemctl`, da reinstalação, da desinstalação, da falha de compilação, das três saídas da regra udev e de que nada foi escrito fora do temporário. Verificar que `sh packaging/tests/install-test.sh` passa e que ele falha se uma das verificações for quebrada de propósito (por exemplo, apagar o `snapshot` falso na desinstalação)

## 3. Documentação

- [x] 3.1 Acrescentar ao `README.md` a seção de instalação (`packaging/install.sh`, `--uninstall`, o que fica em cada diretório, a regra udev à parte, `systemctl --user status halod.service`, `journalctl --user -u halod.service` e parar a unidade antes de rodar o `halod` à mão), e trocar a frase "Nesta versão ele não reaplica o perfil no login nem depois da suspensão" pelo motivo de não reaplicar. Verificar que os comandos citados existem no script e funcionam no teste da task 2.4
- [x] 3.2 Atualizar o `docs/architecture.md` (a unidade, a ativação pelo D-Bus, a decisão de não reaplicar e onde cada arquivo é instalado), o `docs/roadmap.md` (a etapa 4 da fase 2) e o `docs/permissions.md` (a conferência feita pelo script). Corrigir o `openspec/changes/gnome-shell-extension/README.md`, que ainda diz que o `Devices` não tem change. Verificar que nenhum desses arquivos promete a reaplicação do perfil no login

## 4. Integração

- [x] 4.1 Verificar o repositório inteiro: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` e `sh packaging/tests/install-test.sh`, sem erros
- [x] 4.2 Corrigir a corrida achada na conferência no PC (decisão 8 do design): o `halod` publica a lista de aparelhos e a telemetria lidas do sysfs antes de pegar o nome, para que a chamada que o sobe pela ativação não receba a lista vazia. Verificar com o teste `estado_inicial_antes_do_primeiro_tick` (sem nenhum tick, a lista e a telemetria já vêm preenchidas, e o primeiro tick não emite sinal), que falha sem a correção, e com o teste de ponta a ponta `primeira_chamada_ja_tem_aparelhos_e_telemetria` no `tests/daemon.rs`
- [x] 4.3 Com o aval do usuário, instalar no usuário real, no terminal dele (a sessão do Claude usa o `HOME` do perfil): `packaging/install.sh`. Conferir `systemctl --user status halod.service` ativo, o nome no D-Bus com `busctl --user status io.github.leoodaviid.Halo1`, e `Manager.Devices` respondendo (só lê o sysfs). Depois, `systemctl --user stop halod.service` e uma chamada `busctl --user call io.github.leoodaviid.Halo1 /io/github/leoodaviid/Halo1 io.github.leoodaviid.Halo1.Manager Devices`, que deve subir o daemon pela ativação. Nenhum comando é enviado aos aparelhos: subir o daemon e o `Devices` só leem o sysfs (spec `daemon-service`)

## Workflow follow-up

- Sincronizar a spec `user-service`, arquivar a change e commitar, quando o usuário pedir.
- Teste opcional do LED, fora desta change e só com aval: `halo led waterforce static --color ff6600` (`C9`, `CD`, `B6`, `GcLedLib.dll`, ver `docs/protocols/castor3.md`), corte de energia, olhar o anel e voltar com `halo led waterforce cycle`. O resultado vai para o `docs/hardware.md` e decide se a reaplicação no login vira uma change.
