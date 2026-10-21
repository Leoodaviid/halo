# Proposal

## Why

O halo já lê os modos e as curvas de velocidade do Waterforce (`halo probe waterforce`), mas ainda não muda nada. Hoje a fan está em Performance e a bomba em Turbo, e só o GIGABYTE Control Center, no Windows, troca isso. Esta é a primeira escrita do halo que mexe no resfriamento. Por isso ela começa pelos presets do firmware, que a Gigabyte já validou, e não por curvas próprias. Ela também cria o snapshot e o "voltar ao anterior", que a change `waterforce-speed-curve` vai usar.

## What Changes

- Novo comando `halo speed waterforce`, com três subcomandos:
  - `status`: só leitura (`DE 00`, `DD`, `D9 01`, `D9 02`). Mostra o modo da fan e da bomba e o slot Customized guardado no aparelho. Deixa claro que o `D9` não é a curva ativa quando o modo é um preset, e que as curvas dos presets são tabelas de referência da interface do GCC, não leituras do aparelho.
  - `set --fan <modo> --pump <modo>`: aplica presets com `E5` e um `B6` no fim, como o `FanControl.OnClickApply` do GCC. Fan: Default, Quiet, Balanced, Performance e Turbo. Bomba: Balanced e Turbo.
  - `restore [--original]`: volta ao estado anterior à última escrita, ou ao estado de antes da primeira escrita do halo.
- Snapshot obrigatório antes de qualquer escrita: lê `DD`, `D9 01` e `D9 02` e grava em `$XDG_STATE_HOME/halo/` (fallback `~/.local/state/halo/`). Sem snapshot gravado, nada é enviado.
- Opção experimental `--no-save` no `set`, que envia os `E5` sem o `B6`. Serve para descobrir se o `B6` é preciso para a mudança valer ou se só grava na flash.
- Fora do escopo: ZeroRPM, FixedRPM, Customized como alvo novo, curvas (`E6` só na restauração de um estado salvo em Customized) e Quiet na bomba.

## Capabilities

### New Capabilities
- `waterforce-speed`: leitura e escolha do modo de velocidade da fan e da bomba do Waterforce X, com snapshot e restauração do estado anterior. A change `waterforce-speed-curve` deve estender esta capability com a curva Customized.

### Modified Capabilities
(nenhuma: o `halo probe waterforce` continua igual)

## Impact

- `crates/halo-core/src/waterforce/`: novo módulo `speed` (pacotes `E5`, `E6` e `B6`, snapshot e restauração), reaproveitando `packet`, `wait_response`, `ReadQuery` e `SpeedMode` do `probe`.
- `crates/halo-cli/src/main.rs`: subcomando `speed`, e testes em `crates/halo-cli/tests/`.
- Novos arquivos de estado do usuário em `$XDG_STATE_HOME/halo/`. Nenhuma dependência nova.
- Hardware: a primeira escrita que muda a rotação da fan e da bomba. Cada `set` e cada `restore` manda um `B6`, que provavelmente grava na flash.
- `docs/hardware.md`, `docs/protocols/castor3.md` e `docs/roadmap.md`: o resultado do teste do `B6`, o efeito observado de cada preset e a confirmação de que o `D9` é o slot Customized.
