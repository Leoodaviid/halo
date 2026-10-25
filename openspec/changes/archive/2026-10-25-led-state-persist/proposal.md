# Proposal

## Why

O `LastApplied` do `Waterforce.Led` existe só na memória do `halod`. Ao sair e entrar na sessão, o daemon reinicia sem saber o que está no anel, e o painel abre com "Estado do LED desconhecido" e o switch desligado, mesmo com o cooler aceso. Pior: ligar o switch nesse estado é um "religar sem estado conhecido", que pede o LED do perfil ativo ou o `cycle`, e não o estado que estava no aparelho (observado em 2026-10-23: logout com o estático vermelho, e o switch religou para o `cycle`). O aparelho não tem leitura do LED, então a única forma de o painel saber o que está aplicado é o halo lembrar o que aplicou.

## What Changes

- O `halod` grava o último LED que aplicou com sucesso num arquivo em `$XDG_STATE_HOME/halo/` (o mesmo diretório dos snapshots de velocidade e do perfil ativo), a cada mudança do `LastApplied`, e o lê ao iniciar. O `LastApplied` já nasce com esse valor.
- Nenhum comando novo vai ao aparelho: ler o arquivo não reaplica nada, e a decisão de não reaplicar no login (`systemd-user-service`) continua valendo.
- O dado continua sendo "o último LED que o halo aplicou", não uma leitura do aparelho. Um arquivo ausente ou inválido deixa o `LastApplied` vazio, como hoje.
- No painel, nada muda no código: ele já abre com o `LastApplied` e religa com o último estado ligado. O que muda é o cenário: depois de um login, o painel abre com o estado aplicado antes e o switch ligado, e religar volta a esse estado.
- Fora do escopo: a CLI (`halo led`) não grava nem lê o arquivo, porque ela fala direto com o hidraw e não passa pelo daemon; reaplicar no login continua fora do escopo. Em 2026-10-25, o estático branco aplicado pelo painel (com `B6`, sem `halo led` no histórico e sem perfil ativo) sobreviveu ao desligamento com corte de energia: no boot às 16:52, o anel continuava branco, mas o painel abriu desconhecido e com o switch desligado.

## Capabilities

### New Capabilities

Nenhuma.

### Modified Capabilities

- `daemon-waterforce`: o requisito "Último LED aplicado" passa a guardar o valor entre execuções do daemon. A propriedade só fica vazia quando o halo nunca aplicou um LED pelo daemon (ou o arquivo foi perdido), e não mais a cada início.
- `gnome-panel`: os cenários de "Estado mostrado vem do daemon" e de "Liga e desliga" que dependiam de "o daemon acabou de iniciar" passam a depender de "o daemon não tem estado registrado"; e um cenário novo cobre abrir o painel depois de um login.

## Impact

- `crates/halo-daemon/src/bus.rs` (`Led`: carregar o registro ao criar, gravar a cada aplicação bem-sucedida), `crates/halo-daemon/src/main.rs` (passa o `state_dir` ao `Led`) e um módulo pequeno para o arquivo (formato, leitura e escrita atômica), com testes.
- Arquivo novo no estado do usuário: `$XDG_STATE_HOME/halo/waterforce-led.last` (padrão `~/.local/state/halo/`). O `packaging/install.sh --uninstall` já mantém `~/.local/state/halo`.
- Documentação: `README.md` (a propriedade `LastApplied`), `docs/architecture.md` (estado do daemon e o painel) e `docs/roadmap.md`.
- Extensão: nenhum código muda. A spec `gnome-panel` só ajusta cenários.
- Hardware: nenhum comando novo. `docs/hardware.md` registra a persistência do LED com `B6` observada em 2026-10-25.
