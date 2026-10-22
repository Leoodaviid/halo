# Proposal

## Why

Hoje o halo só existe como CLI: cada `halo led` ou `halo speed` abre o aparelho, envia uma sequência e termina. A extensão GNOME (`gnome-shell-extension`), o serviço de login (`systemd-user-service`) e o envio contínuo da CPU ao Waterforce (`waterforce-cpu-feed`) precisam de um processo que fique de pé, guarde perfis e ofereça uma interface estável. Essa é a Fase 2.1 do roadmap, e as três changes dependem dela.

Com um processo de longa duração aparece um risco que a CLI sozinha não tinha: dois processos conversando com o Castor3 ao mesmo tempo. O protocolo é de pedido e resposta (`wait_response` espera o eco do comando), então um `halo probe waterforce` rodando junto de uma escrita do daemon pode misturar as respostas. O acesso ao hidraw precisa ser exclusivo antes de existir um segundo cliente.

## What Changes

- Novo crate `crates/halo-daemon` com o binário `halod`: um daemon do usuário que se registra no D-Bus da sessão como `io.github.leoodaviid.Halo1` e recusa uma segunda instância.
- Interface D-Bus do Waterforce sobre o `halo-core`, sem comandos novos ao hardware:
  - LED: aplicar efeito, cores, brilho e velocidade, com as mesmas sequências e validações do `halo led waterforce`.
  - Velocidade: status, presets, curva plana e restauração, com as mesmas sequências, snapshots e validações do `halo speed waterforce`. As opções experimentais (`--no-save`) e a simulação ficam só na CLI.
  - Telemetria: propriedades lidas do hwmon a cada 2 s, com sinal só quando o valor muda.
- Perfis salvos em `$XDG_CONFIG_HOME/halo/profiles/<nome>.toml`, com seções opcionais por aparelho (`waterforce.led`, `waterforce.speed`). Listar, ler, salvar, apagar e ativar pelo D-Bus. Ativar aplica cada seção. A velocidade é comparada antes com o estado lido no aparelho e só é gravada (com `B6`) quando difere, para não gastar a flash.
- Laço de tarefas periódicas, que nesta change só lê o sysfs (telemetria e presença dos aparelhos), e tratamento do resume (`PrepareForSleep` do logind): no resume o daemon atualiza presença e telemetria na hora e não envia nada ao hardware.
- Acesso exclusivo ao hidraw no `halo-core`: `Hidraw::open` passa a tomar um `flock` exclusivo no nó do aparelho e espera até 5 s. Vale para a CLI e para o daemon. Se o aparelho continuar ocupado, a CLI informa isso e termina com o código 5.
- Fora do escopo: reaplicar o perfil no login ou no resume (`systemd-user-service`), o envio de `E0`/`E1` (`waterforce-cpu-feed`), a CLI passar pelo D-Bus, o IT5702 nos perfis (ainda não tem escrita, `it5702-static-color`) e a unidade systemd.

## Capabilities

### New Capabilities
- `device-exclusive-access`: um único processo por vez com o hidraw de um controlador aberto, a espera limitada e o erro de aparelho ocupado.
- `daemon-service`: o processo `halod`, o nome no D-Bus da sessão, a instância única, a lista de aparelhos, os erros D-Bus, o laço de tarefas periódicas, o resume e a regra de não enviar nada ao hardware sem um pedido.
- `daemon-waterforce`: a interface D-Bus do Waterforce (LED, velocidade e telemetria), com as mesmas regras das capabilities `waterforce-led`, `waterforce-speed` e `waterforce-telemetry`.
- `profiles`: formato, local e validação dos perfis, as operações pelo D-Bus e a ativação.

### Modified Capabilities
- Nenhuma. As capabilities `waterforce-led` e `waterforce-speed` continuam valendo como estão: cada chamada D-Bus de escrita conta como uma execução, com uma única sequência e no máximo um `B6`. A ativação de um perfil usa a sequência de escrita do `FanControl.OnClickApply` (`E5`, `E5`, `E6` por lado em curva plana e `B6`), a mesma do `set`, do `curve` e da restauração, que já está especificada e foi confirmada no aparelho.

## Impact

- `crates/halo-core/src/hid/hidraw.rs`: o `flock` no `open` e o erro `Busy` no `OpenError`. `waterforce/speed.rs`: uma escrita para um estado de velocidade desejado, sobre o `write_to` e o `Snapshot::target` já existentes.
- `crates/halo-cli`: a mensagem e o código de saída do aparelho ocupado, com testes.
- Novo `crates/halo-daemon`, membro do workspace. Dependências novas: `zbus` 5, `serde`, `toml`, `async-channel`, `async-io` e `futures-util` (as duas últimas já vinham com o `zbus`), além do `clap`, que a CLI já usa.
- `README.md`, `docs/architecture.md` e `docs/roadmap.md`: o daemon, a interface D-Bus, os perfis e o lock.
- Hardware: nenhum comando novo. As escritas do daemon são as mesmas da CLI. A ativação de um perfil grava na flash do Castor3 (`B6`) quando muda a velocidade, e sempre no LED, que não tem leitura. O teste no aparelho depende do aval do usuário.
