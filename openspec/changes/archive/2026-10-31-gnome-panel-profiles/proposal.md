# Proposal

## Why

O painel já controla o LED do Waterforce, mas os perfis salvos no `halod` só podem ser ativados pela CLI ou pelo `busctl`, e uma falha de aplicação aparece como uma linha genérica ("Não foi possível aplicar: …"), sem dizer o que fazer nem oferecer nova tentativa. A spec `gnome-panel` já pede o menu de perfis no cabeçalho, as mensagens por tipo de erro com "Tentar de novo" e a linha de status, e esses requisitos estão na `gnome-panel-extras`, que ainda junta os detalhes do aparelho. Esta change separa a parte que gira em torno de aplicar e ativar; os detalhes ficam para a `gnome-panel-details`.

## What Changes

- **Menu de perfis no cabeçalho**, abaixo do título "halo", como o `.profile-btn` do protótipo (`docs/ui/panel-prototype.html`): "Perfil: <nome>", "Sem perfil", "Ativando…" ou "—" com o daemon fora do ar. O menu lista os perfis do daemon, lidos de novo a cada abertura, com o ativo marcado e o efeito do LED de cada um ao lado. Escolher um perfil pede `Profiles.Activate`. Sem perfis, o menu mostra "Nenhum perfil salvo", sem ação.
- **Mensagem de erro por tipo**, no lugar da linha genérica: uma faixa com título e texto para aparelho não encontrado, ocupado, sem permissão (com a regra udev) e falha de comunicação, e uma mensagem genérica para os outros erros. A faixa tem "Tentar de novo", que reenvia o pedido que falhou (um `Led.Apply` ou um `Profiles.Activate`) uma única vez. Os controles voltam ao último estado aplicado, como hoje.
- **Linha de status** no rodapé, acima das preferências: "Aplicando…" (com "1 na fila" quando há um pedido esperando), "Ativando perfil…", "Aplicado ao <aparelho>", "LED desligado" ou "Estado do LED desconhecido". Só aparece no aparelho com controle de LED.
- A ativação de um perfil e os `Apply` não se cruzam: durante uma ativação, um ajuste do LED espera e só sai depois, se ainda for diferente do aplicado; com um `Apply` em andamento ou esperando, o menu de perfis fica insensível.
- O `docs/roadmap.md` passa a mostrar a `halod-outside-session` e a `gnome-panel-cpu-source` como concluídas e a `gnome-panel-extras` dividida em `gnome-panel-profiles` (esta) e `gnome-panel-details`.
- Nenhuma mudança no `halod` e nenhum comando novo ao aparelho: o painel só usa `Profiles.List`, `Profiles.Get`, `Profiles.Activate` e `Led.Apply`, que já existem. Cada ativação e cada `Apply` gravam no aparelho (`99 B6`), como hoje pela CLI.

## Capabilities

### New Capabilities

Nenhuma.

### Modified Capabilities

- `gnome-panel`: "Perfis no cabeçalho" ganha os cenários da lista lida a cada abertura, do perfil ativo marcado, da ativação em andamento, da ativação que falha sem mudar o perfil ativo, do menu insensível com um `Apply` pendente e do daemon fora do ar. "Erros de aplicação" ganha os cenários de aparelho não encontrado, falha de comunicação, erro sem tipo conhecido, erro na ativação de um perfil e a mensagem que some no pedido seguinte. "Linha de status" ganha os cenários da ativação, do LED desligado, do estado desconhecido e do aparelho sem LED.

## Impact

- `extension/`: `lib/ledState.js` com a ativação na mesma fila do LED e o pedido que falhou; uma `lib/` nova com os textos do erro, da linha de status e do botão de perfil, testada com `gjs`; `daemon.js` com `Profiles.List`, `Profiles.Get` por perfil e `Profiles.Activate`, devolvendo o nome do erro D-Bus; `indicator.js` com o botão e o menu de perfis no cabeçalho e a linha de status no rodapé; `ledControls.js` com a faixa de erro e o "Tentar de novo" no lugar da linha genérica; os estilos com as medidas do protótipo.
- `docs/roadmap.md`, `README.md` (o que o painel faz) e `docs/architecture.md` (a fila com a ativação).
- `halod`, unidades e regras udev sem mudança.
