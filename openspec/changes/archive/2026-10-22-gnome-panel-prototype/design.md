# Design

## Context

Ver `proposal.md` (Why) para a motivação. O ponto de partida é o `halo-v2.html` (fora do repositório, em `~/Downloads`): um HTML único de ~345 KB, sendo ~313 KB de um JPEG de fundo embutido em base64, com CSS e JS inline e fotos de produto carregadas de 5 URLs externas.

O que o painel vai consumir do `halod` hoje (`crates/halo-daemon/src/bus.rs`):

```text
/io/github/leoodaviid/Halo1
  .Manager    Devices() -> a(sbs)  (id, presente, devnode); signal DevicesChanged
  .Profiles   List, Get, Save, Delete, Activate; prop ActiveProfile
/io/github/leoodaviid/Halo1/waterforce
  .Waterforce.Led        Apply(s efeito, as cores, y brilho, y velocidade)
                         prop LastApplied a{sv} (effect, colors, brightness, speed),
                         só em memória: vazio depois de o daemon reiniciar
  .Waterforce.Telemetry  props Available, FanRpm, PumpRpm, FanDuty, PumpDuty,
                         CoolantMilliCelsius, CoolantValid
erros: io.github.leoodaviid.Halo1.Error.{InvalidArgument, NotFound,
       PermissionDenied, Busy, Device, ProfileNotFound, Storage}
```

Duas coisas que a spec pede ainda não existem no daemon: o status de cada aparelho (hoje o `Devices` só diz se está presente) e a temperatura da CPU. O protótipo as simula, e a extensão depende das changes do daemon que vão criá-las.

O ambiente-alvo é o GNOME Shell 50.1 no Wayland, com o estilo `default` e o destaque `orange`. O tema do Shell instalado está em `/usr/share/gnome-shell/gnome-shell-theme.gresource` (`gnome-shell-light.css` e `gnome-shell-dark.css`).

Esta change não envia nada ao hardware nem ao daemon, então não há bytes de protocolo a citar. As regras de aplicação da spec existem por causa do `99 B6` no fim de cada aplicação de LED (`GcLedLib.dll`, `FUN_180040780`, ver `docs/protocols/castor3.md`), cujo desgaste por chamada não é conhecido (`docs/hardware.md`).

## Goals / Non-Goals

**Goals:**
- Um protótipo que permita conferir cada cenário da spec `gnome-panel` no navegador, inclusive os estados de erro, sem daemon e sem hardware.
- Que tudo o que aparece no protótipo tenha um caminho conhecido no St (primitiva pronta, Cairo ou `Shell.BlurEffect`), para que a aprovação valha para a extensão.
- Deixar para a `gnome-shell-extension` o mapa de cada elemento do protótipo para o seu equivalente no St.

**Non-Goals:**
- Fidelidade de pixel entre o protótipo e o St. O canvas define o visual; a extensão chega perto dele com o CSS próprio e os efeitos do Clutter.
- Definir a interface D-Bus final dos aparelhos com status. O formato simulado aqui é uma sugestão para a change do daemon.
- Código reaproveitável pela extensão. O JS do protótipo é descartável.

## Decisions

### 1. Um único HTML autocontido em `docs/ui/panel-prototype.html`

O v3 é um arquivo só, sem dependências externas: nada de CDN, de fontes da web nem de imagens remotas. Ele abre direto do disco (`file://`) e funciona offline, como o requisito "Imagens locais" pede para o painel.

- O JPEG de fundo sai e vira um gradiente em CSS. Isso tira ~313 KB do repositório, e o fundo não faz parte do que se aprova.
- As fotos de produto saem. Ficam só os desenhos vetoriais inline do v2.
- A fonte é a pilha local do canvas (`-apple-system`, `"SF Pro Text"`, `"Inter"`, `"Helvetica Neue"`, `"Adwaita Sans"`, `Cantarell`, `sans-serif`), sem carregar arquivo de fonte. O canvas carrega a Inter do Google Fonts; o protótipo não, porque a spec proíbe recursos da rede.
- O v2 não entra no repositório, porque carrega as URLs externas. O v3 cita a origem num comentário.

Alternativa considerada: quebrar o protótipo em HTML, CSS e JS. Foi descartada porque um arquivo só é mais fácil de abrir, de revisar e de publicar para aprovação.

### 2. Visual do canvas "halo · Apple HIG"

O visual vem do canvas do Claude Design que o usuário refinou depois da primeira versão (https://claude.ai/artifact/CNo765N8aGcxZjN1bVyuzF, artboards do painel claro, do escuro e dos estados). Os tokens dele (`--label`, `--material`, `--tile`, `--sep`, `--track`, `--fill`, `--red-tint`...) entram no protótipo como estão, em dois conjuntos, claro e escuro. A barra simulada é a barra de menus do macOS do canvas, por escolha do usuário. Dela, a extensão só implementa o indicador do halo (ícone e temperatura). Os outros itens (menus, Wi-Fi, bateria, controles e relógio) são cenário, e a barra real continua sendo a do GNOME.

O destaque é a exceção: segue o `accent-color` do GNOME, e não as seis cores do canvas. No protótipo, um seletor fora da barra simulada troca entre os destaques do GNOME, começando pelo laranja, que é o deste PC. O `--accent-strong` (fundo das linhas destacadas, com texto branco) é o destaque misturado com 20 % de preto.

Diferenças em relação ao canvas, por causa do GNOME e da spec: o atalho `⌘,` das preferências sai, porque não existe no GNOME; a fonte não vem do Google Fonts; as ações de iniciar e de ver o registro do `halod` passam a ter comportamento (ver decisão 9).

Primeira versão desta decisão (descartada pelo usuário em 2026-10-22): copiar as cores e as medidas de `gnome-shell-light.css` e `gnome-shell-dark.css` (Shell 50.1). O protótipo ficava igual aos menus do Shell, mas o usuário preferiu o refinamento pelo Apple HIG.

### 3. Um caminho no St para cada elemento

| No protótipo | Na extensão (St/Shell 50) |
|---|---|
| Indicador na barra com ícone e `38°` numa pílula | `PanelMenu.Button` com um `St.BoxLayout` (ícone simbólico + `St.Label`) |
| Painel em material translúcido | O `menu` do `PanelMenu.Button` com CSS próprio (`stylesheet.css`) e um `Shell.BlurEffect` em modo `BACKGROUND` no fundo |
| Blocos agrupados (tiles) | `St.BoxLayout` com fundo e raio no CSS |
| Carrossel com transição | `St.Widget` e `ease()` do Clutter; roda pelo `scroll-event` |
| Desenho do aparelho com o anel tingido | `St.DrawingArea` com Cairo, ou camadas de SVG em que só a camada do LED é simbólica e recebe a cor |
| Swatches | `St.Button` redondos com a cor no estilo inline |
| Barra de matiz | `St.DrawingArea` com gradiente Cairo e um marcador arrastável |
| Campo hex | `St.Entry` |
| Brilho e velocidade no estilo da Central de Controle | Uma subclasse do `Slider` de `resource:///org/gnome/shell/ui/slider.js` com o `vfunc_repaint` próprio (trilho largo, preenchimento e ícone), aplicando no `drag-end` |
| Popup de efeito e menu de perfis | Um `PopupMenu.PopupMenu` próprio ancorado no botão |
| Linha de status e itens de menu | `St.Label` e `PopupMenu.PopupMenuItem` |
| Iniciar o halod e ver o registro | Ver decisão 9 |
| Liga/desliga | O switch do `PopupMenu` |
| Perfis | Submenu no cabeçalho |
| Detalhes e Aparelhos | Troca de página dentro do mesmo menu |
| Copiar comando | `St.Clipboard` |
| Preferências | Janela `Adw.PreferencesWindow` (prefs.js), com a fonte da temperatura em GSettings |

Nada dentro do painel simulado usa `<input type=color>`, `<select>` nativo nem animações que o Clutter não faça. O `backdrop-filter` do material é o único efeito sem CSS equivalente no St: na extensão ele vira o `Shell.BlurEffect`.

### 4. Daemon simulado com os mesmos nomes do D-Bus

O JS do protótipo tem um objeto `fakeDaemon` com os nomes da interface real (`Devices`, `Apply`, `LastApplied`, `ActiveProfile`, `Activate`, `CoolantMilliCelsius`, `CoolantValid`...) e as mesmas respostas: sucesso, ou um erro com o nome de `io.github.leoodaviid.Halo1.Error.*`. Cada `Apply` leva uma latência configurável, ~400 ms por padrão, para que o caso "mudança durante um pedido em andamento" possa ser visto.

Um painel de cenários, fora da barra simulada, controla esse daemon:
- daemon fora do ar ou de volta;
- lista de aparelhos: só o Waterforce, Waterforce + placa-mãe, ou vazia;
- `LastApplied` vazio, como depois de o daemon reiniciar;
- próximo `Apply` falha com `Busy`, `PermissionDenied`, `NotFound` ou `Device`;
- telemetria: líquido válido, líquido inválido (224 °C) ou indisponível;
- CPU publicada ou não;
- perfis: nenhum, ou `noite` e `jogo`, um deles com tri-color;
- tema claro ou escuro, e o destaque.

O mesmo painel mostra um **registro de pedidos** com cada `Apply` e `Activate` enviado e um contador de `Apply`, ou seja, de `B6` que o hardware receberia. É assim que se aprova a regra de aplicar só no fim do gesto: arrastar o brilho de 3 a 9 tem que somar 1, não 6.

Alternativa considerada: ligar o protótipo ao `halod` de verdade com um proxy D-Bus ↔ HTTP. Foi descartada porque isso já seria código de produção, escreveria no cooler durante a revisão visual e iria contra a proposta.

### 5. Formato simulado da lista de aparelhos

O protótipo simula o `Devices` com status como `(id, nome, status)`, com o status em `controllable` ou `detected`. Ele também usa o `id` para escolher o desenho (`waterforce`, `it5702`, ou o desenho genérico). Esse formato é só a sugestão para a change do daemon, que decide a assinatura final. Se ela mudar, a extensão se adapta, e a spec `gnome-panel` não muda, porque fala de "status informado pelo daemon".

### 6. Aplicação: um pedido em voo e o último valor vence

```text
            gesto concluído
   ociosa ------------------> aplicando ----- terminou, sem pendência ----> ociosa
     ^                          |    ^
     |                          |    | terminou, com pendência:
     |      gesto concluído     v    | envia só a pendência
     |      durante o envio  pendente
     |                         (guarda só o último valor)
     +--- erro: volta ao LastApplied e mostra a mensagem
```

- "Gesto concluído" quer dizer soltar o slider, clicar num swatch ou num efeito, soltar a barra de matiz ou dar Enter num hex válido.
- Cliques seguidos nos swatches passam por um debounce de 300 ms antes de virar um gesto concluído. Assim, três cliques rápidos geram um pedido.
- Antes de enviar, o valor é comparado com o último aplicado. Se for igual, nada é enviado.
- Durante o arraste, o desenho do aparelho e os rótulos mudam na hora, e só eles.

Na extensão, a mesma máquina de estados fica atrás do proxy D-Bus. O `drag-end` do `Slider` do Shell é o fim do gesto.

### 7. Religar com estado desconhecido

O `LastApplied` vive só na memória do daemon. Para religar, o painel guarda o último estado **ligado** que viu e segue esta ordem: o último estado ligado visto nesta instância do daemon (zerado quando o daemon reinicia, para valer o cenário "Religar sem estado conhecido"), depois o LED do perfil ativo (`Profiles.Get` do `ActiveProfile`), depois `cycle`. O `cycle` vem do README: é o efeito mais próximo do modo de fábrica, confirmado no aparelho (`docs/protocols/castor3.md`).

Alternativa considerada: o daemon persistir o `LastApplied`. Isso é melhor a longo prazo, mas é uma mudança no daemon, fica para a `systemd-user-service`, que vai precisar disso para reaplicar no login, e não muda o comportamento do painel.

### 8. Temperatura na barra

O valor é arredondado para o inteiro mais próximo (38,4 °C vira `38°`, e 38,5 °C vira `39°`). Ele se atualiza a cada `PropertiesChanged` do daemon, que vem a cada 2 s e só quando o valor muda. A largura do rótulo usa algarismos tabulares, para que a barra não "pule" quando a temperatura muda.

### 9. Iniciar o halod e ver o registro

O painel fala com o systemd do usuário pelo D-Bus da sessão (`org.freedesktop.systemd1`): `GetUnit`/`LoadUnit` para saber se `halod.service` existe, `StartUnit("halod.service", "replace")` para iniciar e, depois, espera o nome `io.github.leoodaviid.Halo1` aparecer, como no cenário "Daemon volta". O registro são as últimas 20 linhas de `journalctl --user -u halod.service -n 20 --no-pager`, lidas com `Gio.Subprocess` e mostradas numa página do próprio painel, só para leitura.

Enquanto a `systemd-user-service` não instalar a unidade, as duas ações ficam desabilitadas. No protótipo, o cenário "unidade systemd instalada" controla isso, e o registro é um texto simulado.

Alternativa considerada: abrir o app Registros (`gnome-logs`). Foi descartada porque ele não abre filtrado por unidade pela linha de comando, e nem sempre está instalado.

## Risks / Trade-offs

- [O HTML sempre parece um pouco melhor do que o St renderiza] → A tabela da decisão 3 limita os elementos, e a `gnome-shell-extension` tem que comparar a primeira versão com o protótipo lado a lado antes de pedir aprovação.
- [O `Shell.BlurEffect` no fundo do menu custa GPU e pode destoar dos outros menus do Shell] → A extensão mede o custo com o painel aberto e, se ficar pesado, troca o blur por um fundo opaco com a mesma cor do material, sem mudar o layout.
- [O visual do macOS destoa do resto do GNOME] → Foi uma escolha do usuário. O destaque e o claro/escuro continuam vindo do sistema, então o painel acompanha as preferências dele.
- [Os sliders da Central de Controle não existem no Shell] → A subclasse do `Slider` mantém o comportamento do Shell (teclado, `drag-end`, acessibilidade) e só troca o desenho.
- [O anel tingido em Cairo dá mais trabalho na extensão do que no SVG do protótipo] → O protótipo separa o desenho em "corpo" e "LED" desde já, para que a extensão possa usar camadas de SVG em vez de Cairo puro.
- [O formato simulado do `Devices` diverge do que o daemon vai fazer] → A spec não depende do formato, e a decisão 5 deixa claro que ele é só uma sugestão.
- [Se o seletor de matiz no St ficar ruim de usar, a aprovação do protótipo não vale para ele] → Os swatches e o hex cobrem o uso básico sozinhos. Se a barra de matiz não funcionar bem no St, a extensão pode cortá-la sem mudar os outros caminhos.
- [Um protótipo com painel de cenários vira um "app" que ninguém mantém] → Ele fica em `docs/ui/` como registro do que foi aprovado e não é atualizado depois que a extensão existir. O README da pasta diz isso.

## Migration Plan

Não há migração. O arquivo é novo e não muda nada no daemon nem na CLI. Para voltar atrás, basta apagar `docs/ui/`.
