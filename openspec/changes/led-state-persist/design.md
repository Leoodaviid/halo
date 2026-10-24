# Design

## Context

- `crates/halo-daemon/src/bus.rs`: a interface `Led` guarda o último LED aplicado num `Mutex<Option<LedRecord>>`, criado vazio em `Led::new(worker)`. `apply_led()` é o único caminho que o atualiza: o `Led.Apply` e o `Profiles.Activate` com a seção `waterforce.led` passam por ela, que chama o worker, grava o `LedRecord` e emite o `PropertiesChanged` do `LastApplied`.
- `crates/halo-daemon/src/profiles.rs` já tem o formato e a validação do LED: `LedSection` (`effect`, `colors`, `brightness` e `speed` opcionais, `deny_unknown_fields`), `led_setting()`, que transforma esses campos num `LedSetting` validado pelo `halo-core`, e `write_atomic()` (temporário, `sync_all`, `rename`).
- `crates/halo-daemon/src/main.rs` já resolve o `state_dir` (`halo_core::waterforce::speed::state_dir()`: `$XDG_STATE_HOME/halo` ou `~/.local/state/halo`) e o passa aos snapshots de velocidade e ao `ProfileStore` (`active-profile`).
- A extensão (`gnome-panel-led`) já faz o necessário com um `LastApplied` presente ao iniciar: `lastAppliedChanged` põe o valor em `applied`, `draft` e, se não for o desligado, em `lastOn`. Então o painel abre com o estado e o switch ligado, e religar usa o `lastOn`.
- O `packaging/install.sh --uninstall` mantém `~/.local/state/halo`.

## Goals / Non-Goals

**Goals:**
- O `LastApplied` sobrevive a reinícios do `halod` (logout, `systemctl --user restart`, atualização).
- Nenhum comando novo ao aparelho, nem indireto: carregar o arquivo não chama o worker.
- Um arquivo ruim nunca impede o daemon de subir.

**Non-Goals:**
- Reaplicar o LED no login ou depois de um corte de energia (continua fora, `systemd-user-service`).
- A CLI `halo led` gravar ou ler o arquivo: ela fala direto com o hidraw. Depois de um `halo led`, o arquivo e o painel mostram o último LED aplicado **pelo daemon**, como o `LastApplied` já faz hoje.
- Guardar o "último ligado" separado do último aplicado. Com o LED desligado no logout, religar depois do login pede o perfil ativo ou o `cycle` (cenário "Religar sem estado conhecido").
- Mudar a extensão: só testes.

## Decisions

### 1. Arquivo `waterforce-led.last` no `state_dir`, no formato do perfil

`$XDG_STATE_HOME/halo/waterforce-led.last`, em TOML, com os mesmos campos da seção `[waterforce.led]` de um perfil, gravados a partir do `LedSetting` aplicado:

```toml
# Último LED aplicado pelo halod. Não é uma leitura do aparelho.
effect = "static"
colors = ["ff0000"]
brightness = 10
speed = 5
```

- O desligado vai sem `brightness` (o `LedSetting::new` recusa brilho no `off`), e os efeitos sem cor vão sem `colors`. O registro do `LastApplied` continua o mesmo (o `off` com brilho 0).
- Leitura: `toml::from_str::<LedSection>` e `profiles::led_setting()`, as mesmas regras de um perfil. Qualquer falha (arquivo ilegível, campo desconhecido, efeito, cor, brilho ou velocidade inválidos) vira "sem valor guardado", com um `eprintln!` no log do `halod`.
- Escrita: `write_atomic()` do `profiles.rs` (passa a `pub(crate)`), então um corte no meio deixa o arquivo antigo ou o novo, nunca pela metade.

Alternativa: JSON ou o `a{sv}` serializado. Descartada: o TOML e a validação já existem, e o arquivo fica legível e editável à mão como um perfil.

Alternativa: guardar dentro do `active-profile`. Descartada: são dados de natureza diferente (estado aplicado e escolha do usuário).

### 2. Um tipo `LedState` dono do arquivo, usado pelo `Led`

Módulo novo `crates/halo-daemon/src/led_state.rs`, com `LedState { path }`, `load() -> Option<LedSetting>` e `save(&LedSetting) -> io::Result<()>`, testado sozinho com um diretório temporário. `Led::new(worker, store: Option<LedState>)` carrega o registro na criação. `apply_led()` grava depois de atualizar o `LedRecord` e antes de emitir o sinal; uma falha na gravação vira um `eprintln!` e não muda a resposta do `Apply` (cenário "Falha ao guardar"), porque o aparelho já recebeu o `B6`.

- `store` é `None` quando não há `state_dir` (sem `XDG_STATE_HOME` nem `HOME`), como os snapshots: o daemon funciona como hoje.
- A gravação acontece no executor do zbus. É um arquivo de poucos bytes por aplicação bem-sucedida, e as aplicações são raras (no máximo uma por gesto, `gnome-panel-led`).

Alternativa: gravar no worker, depois do `apply_led` do backend. Descartada: o worker não conhece o `LedRecord`, e o `Profiles.Activate` com LED também precisa gravar; `apply_led()` em `bus.rs` já é o ponto único.

### 3. Carregar não toca no aparelho

O `LedState::load()` roda em `main.rs`, antes de publicar os objetos, junto da primeira leitura do sysfs: assim o primeiro `Get` do `LastApplied`, inclusive o que ativa o daemon pelo D-Bus, já vê o valor. Nada chama o worker no carregamento, e o teste do daemon confirma que um `halod` iniciado com o arquivo não abre o hidraw.

## Risks / Trade-offs

- [O valor guardado diferir do aparelho] → Depois de um `halo led`, do GCC no Windows ou de um corte de energia que apague o LED (não testado), o painel mostra o último LED do daemon, e não o do anel. Já é o significado do `LastApplied` ("não é o estado do aparelho"); a diferença é que agora ele dura entre sessões. O README passa a dizer isso.
- [Arquivo corrompido ou editado à mão] → Validado com as regras dos perfis; inválido vira "sem estado", com aviso.
- [Gravação a cada aplicação] → Uma escrita pequena com `sync_all` por aplicação bem-sucedida; as aplicações já são limitadas a uma por gesto.
- [Dois `halod`] → Não acontece: a segunda instância recusa o nome no D-Bus e sai.
