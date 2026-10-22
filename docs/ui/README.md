# Protótipo do painel

[`panel-prototype.html`](panel-prototype.html) é o protótipo interativo do painel do halo no GNOME Shell (change `gnome-panel-prototype`, Fase 2.2 do roadmap). Ele mostra como a extensão (`gnome-shell-extension`) deve se comportar. A spec que ele segue é a `gnome-panel`.

É o registro do painel aprovado. Depois que a extensão existir, ele não é mais atualizado: o comportamento passa a valer pela spec e pelo código da extensão.

## Como abrir

Abra o arquivo direto no navegador, sem servidor e sem internet:

```sh
xdg-open docs/ui/panel-prototype.html
```

O arquivo não carrega nada de fora: nem fontes, nem imagens, nem scripts.

## O que é simulado

Nada fala com o `halod` nem com o hardware. O protótipo tem um daemon falso com os nomes da interface D-Bus real (`Devices`, `Apply`, `LastApplied`, `ActiveProfile`, `Activate`, as propriedades de telemetria e os erros `io.github.leoodaviid.Halo1.Error.*`). O painel de cenários, à esquerda (embaixo, no celular), controla esse daemon:

| Cenário | O que simula |
|---|---|
| halod rodando | O nome `io.github.leoodaviid.Halo1` com ou sem dono no D-Bus. Quando o daemon volta, ele é uma instância nova, com o `LastApplied` vazio |
| `halod.service` instalado | Se a unidade do systemd do usuário existe. Sem ela, "Iniciar halod" e "Ver registro" ficam desabilitados. O registro mostrado é simulado |
| Reiniciar | Um restart do `halod`: o `LastApplied` e o modelo do Waterforce, que só ficam em memória, voltam vazios, e o perfil ativo continua |
| Latência | O tempo de cada `Apply` e `Activate`, para ver a fila de pedidos |
| Próximo pedido falha com | O próximo `Apply` ou `Activate` responde com `Busy`, `PermissionDenied`, `NotFound` ou `Device` |
| Aparelhos | Quais aparelhos estão presentes. O `Devices` falso devolve os três no formato do `halod` (`aa{sv}` com `id`, `name`, `present`, `devnode` e `capabilities`), com os ausentes na lista. O painel mostra só os presentes e põe os controles onde há `led`. O padrão tem o G502 ausente |
| Waterforce revela o modelo | O `DevicesChanged` com o nome `AORUS Waterforce X 240` no lugar de `AORUS Waterforce X`, como depois da primeira operação no aparelho. Um `Apply` ou `Activate` faz o mesmo, e o restart volta ao nome sem modelo |
| Telemetria | Fan, bomba e líquido do hwmon, o líquido inválido (224 °C) e a telemetria indisponível |
| halod publica a CPU | A temperatura da CPU, que o `halod` real ainda não publica (change `waterforce-cpu-feed`) |
| Perfis | Os perfis `noite` (estático) e `jogo` (três cores), ou nenhum |
| Aparência | Claro ou escuro, e os destaques do GNOME |

O **registro de pedidos** mostra cada `Apply` e `Activate` com os argumentos, e o contador mostra quantas gravações (`99 B6`) o Castor3 receberia. Arrastar um slider tem que somar um só, porque cada aplicação de LED termina com `B6` e o desgaste por chamada não é conhecido (ver [hardware.md](../hardware.md)).

As preferências abrem pelo item "Preferências do halo…" do próprio painel.

## De onde vem o visual

O visual segue o canvas "halo · Apple HIG" do Claude Design (https://claude.ai/artifact/CNo765N8aGcxZjN1bVyuzF), que refinou o protótipo v2: material translúcido com blur, blocos agrupados, popup de efeito no estilo do macOS, sliders no estilo da Central de Controle, linha de status e itens de menu no rodapé. A barra simulada é a barra de menus do macOS do canvas. Dela, só o indicador do halo (o ícone com a temperatura) é da extensão: menus, Wi-Fi, bateria, controles e relógio são cenário e não serão implementados. Os tokens do canvas estão no começo do CSS, em claro e em escuro.

Diferenças em relação ao canvas: a cor de destaque segue a do GNOME (laranja neste PC); a fonte não vem do Google Fonts; o atalho `⌘,` saiu; "Iniciar halod" e "Ver registro" funcionam (com o daemon simulado). Ficaram de fora do v2 o seletor de cor do navegador e as fotos de produto, que dependem da rede. O mapa de cada elemento para o St, inclusive o blur com o `Shell.BlurEffect`, está em [architecture.md](../architecture.md#painel-do-gnome-shell).
