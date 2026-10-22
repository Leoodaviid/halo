# Proposal

## Why

A extensão GNOME (`gnome-shell-extension`, Fase 2.3) precisa de um painel aprovado antes de virar código. Já existe um protótipo interativo (`halo-v2.html`), mas ele foi desenhado sem as restrições do St e do hardware: aplica a cada movimento de slider (e cada `Led.Apply` termina com um `B6`, cujo desgaste por chamada não é conhecido), busca fotos em URLs externas, usa `backdrop-filter` e `<input type=color>`, que o St não tem, e lista aparelhos fora de escopo a partir de um catálogo fixo. Esta change revisa o protótipo, registra o comportamento do painel numa spec e entrega o protótipo aprovado que a extensão vai seguir.

## What Changes

- Protótipo v3 em HTML, a partir do `halo-v2.html`, versionado no repositório. Continua sem código de produção e sem acesso ao hardware ou ao daemon.
- Visual: segue o canvas "halo · Apple HIG" do Claude Design (https://claude.ai/artifact/CNo765N8aGcxZjN1bVyuzF), que refinou o v2: material translúcido com blur, blocos agrupados, popup de efeito no estilo do macOS, sliders no estilo da Central de Controle, linha de status e itens de menu no rodapé. Claro e escuro seguem o sistema, e a cor de destaque segue a do GNOME. Sem seletor de cor do navegador.
- Com o daemon fora do ar: ações para iniciar o `halod.service` e ver o registro dele no próprio painel, desabilitadas enquanto a unidade da `systemd-user-service` não existir.
- Barra superior: ícone do halo com a temperatura sempre visível ao lado. A fonte da temperatura é configurável nas preferências da extensão: o líquido por padrão, e a CPU quando o daemon passar a publicá-la. Um valor inválido (`CoolantValid` falso) aparece como `--`, nunca como o número informado. Sem dado, fica só o ícone.
- Cabeçalho do painel: perfil ativo num menu (`Profiles.Activate` e `ActiveProfile`), liga/desliga e acesso à lista de aparelhos.
- Carrossel: mostra os aparelhos que o daemon lista, com o status que ele informa. Os que não têm controle aparecem com o aviso de controle em desenvolvimento. A extensão não guarda catálogo de hardware. O teclado Redragon e a RAM não aparecem.
- Aparelhos sem foto: só desenhos vetoriais locais, sem imagens buscadas na internet.
- Cor: swatches, uma barra de matiz e um campo hex.
- Efeitos: os 9 de uma cor (estático, pulso, flash, flash duplo, gradiente, ciclo, onda, arco-íris e desligado). Os efeitos de paleta (`color-shift`, `tri-color`, `spin`, `switch`, `lego`, `aurora`) ficam fora da v1 do painel. Um perfil ativo com efeito de paleta aparece só para leitura, sem edição.
- Aplicação: durante o arraste de um slider, a mudança aparece só na interface. O painel pede um `Led.Apply` ao soltar, com debounce, no máximo um por vez e nunca em laço.
- Liga/desliga: desligar aplica `off`. Religar reaplica o `LastApplied` do daemon, porque o protocolo não lê o estado do LED.
- Detalhes: fan e bomba (rpm e duty), líquido, efeito, brilho e velocidade, só para leitura, com `inválido` quando o líquido não for válido, e o comando equivalente da CLI para copiar.
- Estados de erro descritos no protótipo e na spec: daemon fora do ar, aparelho ausente, aparelho ocupado, sem permissão (regra udev) e líquido inválido, com "Tentar de novo" nos erros de aplicação.
- Fora do escopo: o controle de velocidade no painel (continua só na CLI), os efeitos de paleta, o código da extensão (`gnome-shell-extension`) e as mudanças no daemon listadas em Impacto.

## Capabilities

### New Capabilities
- `gnome-panel`: o comportamento do painel do halo no GNOME Shell. Cobre o indicador na barra e a temperatura, o carrossel guiado pelo daemon, os controles de LED e quando eles aplicam, perfis, detalhes e telemetria, e os estados de erro. A `gnome-shell-extension` implementa esta capability.

### Modified Capabilities
- Nenhuma. A lista de aparelhos com status e a temperatura da CPU mudam requisitos do `daemon-service` e do `daemon-waterforce`, mas ficam em changes do daemon (ver Impacto), não nesta.

## Impact

- Novo arquivo de protótipo em `docs/` (o v3 do HTML). Nenhum código Rust ou GJS.
- Pré-requisitos no `halod` para a extensão, não para esta change:
  - A lista de aparelhos com status (detectado ou controlável), para o carrossel. Nenhuma change cobre isso ainda: precisa de uma change nova e pequena antes da `gnome-shell-extension`.
  - A temperatura da CPU como propriedade, para a opção CPU na barra. Entra na `waterforce-cpu-feed`.
- `gnome-shell-extension`: passa a seguir o protótipo e a spec `gnome-panel` aprovados aqui.
- `docs/roadmap.md` e `docs/architecture.md`: o painel aprovado e os pré-requisitos do daemon.
- Hardware: nenhum comando enviado. A regra de aplicar só ao soltar existe para não multiplicar `B6` no Castor3.
