# Design

## Context

- A fila do LED fica em `extension/lib/ledState.js`, que não depende do St e é testada com o `gjs`. Ela tem um pedido em voo (`inFlight`) e só o último pendente (`pending`). Na falha, `finished()` descarta o pendente e volta o rascunho ao aplicado, e guarda em `error` só o texto. O `ledControls.js` mostra esse texto numa linha ("Não foi possível aplicar: …").
- `extension/daemon.js` já tem o proxy de `Profiles`, com a propriedade `ActiveProfile` e o `Get`, usado para religar o LED pelo perfil ativo. O `apply()` já devolve o nome do erro D-Bus (`Gio.DBusError.get_remote_error`).
- No `halod`, `Profiles.Activate(name)` aplica a velocidade e depois o LED. Ele tenta as duas seções e só marca o perfil como ativo se todas derem certo. Na falha, devolve o primeiro erro com o contexto "aplicadas: …; falharam: …; o perfil ativo não mudou". O LED aplicado pela ativação muda o `LastApplied` e emite `PropertiesChanged`, e a ativação bem-sucedida também emite o de `ActiveProfile`. `Profiles.Save` não emite nada, então a lista só é conhecida chamando `List`.
- Os erros D-Bus do daemon têm o prefixo `io.github.leoodaviid.Halo1.Error.`: `NotFound`, `PermissionDenied`, `Busy`, `Device`, `ProfileNotFound`, `InvalidArgument` e `Storage` (`crates/halo-daemon/src/error.rs`).
- O protótipo (`docs/ui/panel-prototype.html`) define o `.profile-btn` (11 px, `--label2`, seta de 9 px), o `.menu-list` com `.menu-item` (24 px de altura, marca de 14 px e o efeito à direita, a 11 px), o `.banner` de erro (fundo vermelho translúcido, raio de 12 px, padding de 10 px por 12 px, título em negrito e texto em `--label2`, com "Tentar de novo") e o `.status` (22 px, 11 px, ícone de check verde ou spinner). Os textos do erro e do status saem do `showError()` e do `renderDevice()` dele.
- No tema da sessão real (MacTahoe-Dark com o User Themes), as regras do tema vencem o CSS da extensão em todo widget com classe do Shell. As medidas desses widgets vão inline, e só as classes `halo-*` ficam no CSS.

## Goals / Non-Goals

**Goals:**

- Uma única fila decide quando sai um `Apply` e quando sai uma ativação, com o pedido que falhou guardado para o "Tentar de novo".
- Todo texto novo (erro, status e botão de perfil) sai de funções puras, testadas pelos cenários da spec.

**Non-Goals:**

- Salvar, renomear ou apagar perfis pelo painel. O menu só ativa.
- A página de detalhes, o item "Detalhes do aparelho…" e o comando da CLI (`gnome-panel-details`).
- O botão de lista de aparelhos do cabeçalho do protótipo (`#openDevices`), que nenhuma spec pede ainda.
- Mudar o `halod` ou o `Profiles.Activate`.

## Decisions

### 1. A ativação entra na fila do LED

`ledState.js` ganha `activating` (o nome do perfil em ativação, ou `null`), `failed` (o pedido que falhou: `{kind: 'apply', setting}` ou `{kind: 'activate', name}`) e `editing` (um gesto em andamento: um slider ou a barra de matiz já mudaram o rascunho, e o fim do gesto ainda não fez o `commit`). O `error` passa de texto a `{name, message}`, com o nome do erro D-Bus.

- `activate(led, name)`: só com nada em voo, nada pendente e nenhuma ativação. Marca `activating`, limpa `error` e `failed` e devolve `{activate: name}`. Fora disso, devolve `send: null`, e o estado não muda.
- `draft()` (o `ledDraft` de hoje, chamado pelo `int-changed` dos sliders e pelo `hue-changed`) liga `editing`. `commit()` o desliga. O campo hex não liga: ele guarda o texto até o Enter ou a saída de foco, sem rascunho.
- `commit()` com uma ativação em andamento guarda o ajuste em `pending`, como com um `Apply` em voo.
- `activateFinished(led, ok, error)`: limpa `activating`. Na falha, guarda `error` e `failed`, descarta o pendente e volta o rascunho ao aplicado, como o `finished()` de um `Apply`. No sucesso, um pendente sai sempre, sem comparar com o aplicado conhecido: esse aplicado pode ainda ser o de antes do perfil, se a resposta chegou antes do `LastApplied`, e um ajuste que o usuário voltou ao valor antigo durante a ativação seria descartado por igualdade e perdido (achado do juiz externo). Sem pendente e sem `editing`, o rascunho passa ao aplicado. O daemon emite o `LastApplied` do LED do perfil antes de responder ao `Activate` (dentro do `apply_led`), então a resposta pode chegar com o `applied` já novo e o rascunho ainda antigo; sem essa cópia, os controles ficariam no perfil anterior, e um brilho mexido depois reaplicaria o efeito e a cor antigos (achado do juiz externo).
- `finished()` de um `Apply` que falha guarda `failed: {kind: 'apply', setting: inFlight}`.
- `retry(led)`: com `failed`, faz o `commit(setting)` ou o `activate(name)` e limpa `failed`. Assim sai um único pedido, mesmo com cliques repetidos: o segundo encontra a fila ocupada.
- `lastAppliedChanged()`: sem nada em voo, nada pendente e sem `editing`, o rascunho passa ao `LastApplied` novo, inclusive durante uma ativação. Com essas regras, os controles mostram o LED do perfil nas duas ordens possíveis (o `LastApplied` antes da resposta ou depois dela), e um gesto em andamento nunca perde o valor: o `_commit()` do `ledControls.js` envia o rascunho atual, então trocá-lo no meio do arraste faria o `drag-end` mandar o LED do perfil no lugar do valor escolhido (achado do juiz externo). Isso também corrige o mesmo caso com um `Apply` de outro cliente durante o arraste, que já existia.
- Com `editing`, os controles também não recebem o valor novo no `update()`: o slider não salta sob o dedo. Depois do `commit`, a fila decide como hoje.

Alternativa: uma fila separada para os perfis no `daemon.js`. Foi descartada porque o `Apply` e o `Activate` passam pelo mesmo worker no daemon e gravam o mesmo LED. Duas filas no painel poderiam mandar um `Apply` no meio da ativação, e o usuário veria o LED do perfil trocado pelo ajuste antigo sem entender por quê.

### 2. Textos em `extension/lib/feedback.js`

Três funções puras, com os textos do protótipo:

- `errorView(error)` devolve `{title, text}`. Pelo nome D-Bus, sem o prefixo:
  - `Busy`: "O aparelho está em uso por outro processo" / "Outro programa do halo está falando com ele. Tente de novo em alguns segundos."
  - `PermissionDenied`: "Sem permissão para abrir o aparelho" / "Falta a regra udev do halo. Veja docs/permissions.md."
  - `NotFound`: "Aparelho não encontrado" / "Confira se o Waterforce está conectado."
  - `Device`: "Falha de comunicação com o aparelho" / "O pedido não foi confirmado. O LED pode estar no estado anterior."
  - Qualquer outro, inclusive o `ProfileNotFound` e um erro sem nome: "O halod recusou o pedido", com a mensagem do daemon como texto.
  - Numa ativação, o texto ganha o que o perfil já mudou no aparelho e "O perfil ativo não mudou." no fim, que é o que o daemon garante. O `Activate` tenta a velocidade e o LED mesmo quando uma seção falha, e a mensagem do erro termina com "aplicadas: <seções>; falharam: <seções>" (`Error::with_context` na `activate` do `bus.rs`). O `errorView()` lê esse trecho e o traduz (`waterforce.speed` → velocidade, `waterforce.led` → LED, `nenhuma` → nada): por exemplo, "Já aplicado: velocidade. Falhou: LED. O perfil ativo não mudou.". Sem o trecho, fica só a frase final (achado do juiz externo).
- `statusView(led, deviceName)` devolve `{icon: 'spinner' | 'check' | null, text}`, nesta ordem: "Aplicando…" ou "Aplicando… (1 na fila)" com um `Apply` em voo; "Ativando perfil…"; "Estado do LED desconhecido" sem `applied`; "LED desligado"; "Aplicado ao <nome>".
- `profileLabel(owner, activating, active)` devolve "—" sem o daemon, "Ativando…", "Perfil: <nome>" ou "Sem perfil".

Alternativa: deixar os textos nos widgets. Foi descartada porque o `gjs` não testa o St, e os cenários da spec são justamente esses textos.

### 3. O menu de perfis

- Um `St.Button` com o `.profile-btn` abaixo do título, na coluna esquerda do cabeçalho, e um `PopupMenu.PopupMenu` preso a ele, no mesmo padrão do menu de efeitos do `ledControls.js` (`PopupMenuManager` próprio e o ator no `Main.uiGroup`).
- Ao abrir: `Profiles.List` com `NO_AUTO_START`, e o menu é montado com a resposta. O ativo leva `Ornament.CHECK`. O efeito de cada perfil (o `hint` do protótipo) vem de um `Profiles.Get` por perfil, chamados juntos depois da lista, e cada linha é preenchida quando o seu chega. Um `Get` que falha deixa a linha sem efeito. Com a lista vazia, aparece uma linha insensível "Nenhum perfil salvo".
- Com a fila ocupada (`inFlight`, `pending` ou `activating`), as linhas ficam insensíveis.
- Sem o daemon, o botão fica insensível e mostra "—".
- O `Profiles.Activate` sai de `daemon.js`, como o `apply()`: devolve `{ok}` ou `{ok: false, error: {name, message}}`, e `null` se foi cancelado no `disable()`.

Alternativa: guardar a lista e atualizar só pelo `ActiveProfile`. Foi descartada porque o `Save` não emite sinal: um perfil salvo pela CLI com o painel aberto não apareceria.

### 4. A faixa de erro e a linha de status

- **A faixa:** a linha `halo-led-error` do `ledControls.js` vira a faixa do protótipo, acima dos controles (no protótipo, o `#errorNote` vem entre o aparelho e os controles), com o ícone de alerta, o título, o texto e o botão "Tentar de novo". Ela só aparece com `error` e no aparelho com LED. O botão chama um handler novo, `ledRetry`, que passa pelo `retry()` da decisão 1.
- **A linha de status:** fica no rodapé do `indicator.js`, antes do separador e do item de preferências, como o `.foot` do protótipo. Ela só aparece com o daemon no ar e o aparelho selecionado com a capacidade `led`. O spinner é um `St.Icon` girando por um `Clutter` transition, e o check é um ícone simbólico em verde (`--green` do protótipo, nas duas variantes do tema). Ambos com as medidas inline.

### 5. Roadmap

A linha da Fase 2.4 passa a dizer que a `halod-outside-session` foi concluída e ensaiada (2026-10-30), e a da `gnome-panel-cpu-source`, que ela foi concluída e arquivada (2026-10-27). A `gnome-panel-extras` vira duas linhas: `gnome-panel-profiles` (esta) e `gnome-panel-details` (pendente).

## Risks / Trade-offs

- [A resposta do `Activate` pode chegar antes do `PropertiesChanged` do `LastApplied`] → Por isso o pendente sai sem comparação depois de uma ativação bem-sucedida. Ele só existe se o usuário mexeu nos controles durante a ativação, e mandá-lo é o que ele pediu por último. No pior caso, sai um `Apply` igual ao LED do perfil, que custa um `B6` a mais.
- [O `errorView()` depende do formato "aplicadas: …; falharam: …" da mensagem do daemon] → O formato é do próprio `halod` e está coberto pelos testes dele; sem o trecho, a faixa perde só o detalhe das seções, e a frase "O perfil ativo não mudou." continua.
- [Um `Profiles.Get` por perfil a cada abertura do menu] → Os perfis são poucos e ficam em arquivos TOML locais. Os `Get` não abrem o aparelho e não atrasam a abertura, porque o menu aparece com a lista.
- [Cada ativação grava no aparelho (`B6` do LED e, se a velocidade for diferente, a dela)] → É o comportamento da CLI e do `Profiles.Activate`. O painel não ativa nada sozinho: só no clique, e um por vez.
- [O spinner, o check e as medidas no tema da sessão real] → Conferir no Shell aninhado (Adwaita) e, com o MacTahoe-Dark, na sessão do usuário na conferência final. O harness de screenshot com o tema do usuário, usado na `gnome-panel-led`, não está no repositório.

## Migration Plan

Nada a migrar: a extensão é reinstalada pelo `packaging/install.sh`, que pede para sair e entrar na sessão quando ela muda. Para voltar atrás, basta reinstalar a versão anterior da extensão. Os perfis no daemon não mudam.
