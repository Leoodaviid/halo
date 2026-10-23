# Proposal

## Why

A extensão já está na barra, mas o painel só diz quantos aparelhos o `halod` encontrou. O protótipo aprovado (`docs/ui/panel-prototype.html`) e a spec `gnome-panel` põem no centro do painel um carrossel com o desenho de cada aparelho, e é sobre ele que os controles do LED vão ficar. Esta change é a primeira de duas do painel: ela traz o carrossel e os desenhos, só lendo do daemon, para que a `gnome-panel-led`, que escreve no LED, comece com a navegação, o desenho e a leitura do `LastApplied` já prontos e conferidos.

## What Changes

- **Carrossel dos aparelhos** no painel, no lugar da contagem: só os aparelhos que o `Devices()` lista como presentes, na ordem da lista, com o nome que o daemon informa e o status "Disponível" (com a capacidade `led`) ou "Detectado" (sem). Setas, indicador de página, roda do mouse sobre o desenho (um passo por sequência rápida de eventos) e teclas de seta com o carrossel em foco, tudo circular. Com um aparelho só, sem setas e sem indicador. A página atual é mantida pelo id quando a lista muda; se o aparelho sumir, vai para o primeiro presente; sem nenhum presente, "Nenhum aparelho encontrado".
- **Desenhos locais**: os SVGs do protótipo (Waterforce, placa-mãe IT5702 e o genérico com o nome) portados para uma função pura que gera o texto do SVG, mostrado num `St.Icon`. Nada vem da rede.
- **O anel do Waterforce com a cor do último LED aplicado**, lida da propriedade `LastApplied` de `Waterforce.Led` (só leitura): a cor aplicada, o arco-íris nos efeitos sem cor, o anel esmaecido com o LED desligado e um anel neutro antes da primeira aplicação. Os outros controles continuam com o destaque do sistema.
- **Corpo do aparelho** abaixo do carrossel: num aparelho sem `led`, o aviso de controle em desenvolvimento; no Waterforce, por enquanto, um aviso provisório de que os controles chegam na próxima versão do painel.
- **Liga/desliga no cabeçalho**, desabilitado em todos os aparelhos nesta change.
- O cliente D-Bus passa a guardar a lista completa de aparelhos e o `LastApplied`, por um proxy novo de `Waterforce.Led` com `DO_NOT_AUTO_START`, como os outros.
- **Nada é escrito no hardware**: nenhuma chamada a `Apply`, `Activate` ou outro método que mude um aparelho.
- Fora do escopo, na `gnome-panel-led`: os controles do LED (cor, efeito, brilho, velocidade), o liga/desliga habilitado, a fila de `Apply` e o religar. Na `gnome-panel-extras`: perfis, detalhes, erros, linha de status, preferências e a fonte CPU.

## Capabilities

### New Capabilities

Nenhuma.

### Modified Capabilities

Nenhuma. O comportamento que esta change implementa já está na spec `gnome-panel`: "Aparelhos vindos do daemon", "Navegação do carrossel", "Aparelho sem controle", "Imagens locais" e, em "Tema do sistema", a cor do LED só no desenho. O que muda é só a implementação, por isso a change usa `skip_specs: true`. O aviso provisório no corpo do Waterforce e o liga/desliga desabilitado são estados intermediários até a `gnome-panel-led` e não viram requisito. Como o anel aparece com o LED desligado, com efeitos sem cor ou sem `LastApplied` é detalhe de desenho, registrado no `design.md`.

## Impact

- Novos: `extension/lib/carousel.js`, `extension/lib/art.js`, `extension/carousel.js`, `extension/stylesheet-light.css` e `extension/stylesheet-dark.css` (as cores do protótipo em cada tema) e os testes `extension/tests/carousel.test.js` e `extension/tests/art.test.js`.
- Alterados: `extension/daemon.js` (lista de aparelhos, proxy do `Waterforce.Led` e `LastApplied`), `extension/indicator.js` (cabeçalho com o liga/desliga, carrossel e corpo no lugar da contagem), `extension/lib/view.js` (normalização da lista e do `LastApplied`), `extension/stylesheet.css` e `scripts/dev/headless-smoke.sh` (o proxy novo e o sysfs falso novo).
- Novo `scripts/dev/fake-sysfs.sh`: o Castor3 falso em `hidraw99`, que não existe nesta máquina. Hoje a fumaça e um teste do `halod` chamam o Castor3 falso de `hidraw3`, que aqui é o teclado Redragon; nada abre esse nó ainda, mas a `gnome-panel-led` vai pôr um `Apply` na fumaça. Os testes do Rust deixam de usar números de hidraw que existem (`crates/halo-daemon`, só nos testes).
- Documentação: `docs/architecture.md` (estrutura da extensão, a linha do desenho na tabela do St e a divisão em changes), `docs/roadmap.md` e `README.md`, se ele citar o painel.
- `packaging/install.sh` não muda: ele já copia toda a pasta `extension/` sem `tests/`.
- `crates/`: nada muda. O `halod` já publica o `Devices()` com as capacidades e o `LastApplied` com `PropertiesChanged`.
- Hardware: nenhum comando. A extensão só lê a lista de aparelhos e o `LastApplied` do `halod`.
