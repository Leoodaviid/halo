# Tasks

Nenhuma task envia comandos ao hardware nem fala com o `halod`: o protótipo usa só o daemon simulado (design, Decisão 4).

## 1. Base do protótipo

- [x] 1.1 Extrair `gnome-shell-light.css` e `gnome-shell-dark.css` de `/usr/share/gnome-shell/gnome-shell-theme.gresource` com `gresource extract` para uma pasta no scratchpad (só leitura, fora do repositório) e anotar os valores do popover, do item de menu, do slider, do switch, do submenu e do separador (design, Decisão 2). Verificar que os dois arquivos foram extraídos e que cada token usado no protótipo tem o seletor de origem anotado num comentário do CSS
- [x] 1.2 Criar `docs/ui/panel-prototype.html` a partir do `halo-v2.html`, sem o JPEG de fundo (que vira um gradiente em CSS), sem as fotos de produto e com os tokens da task 1.1 em variáveis CSS para claro e escuro, mais o comentário de origem (v2, Shell 50.1). Verificar com `grep -nE 'https?://|data:image/(jpeg|png)'` que não sobra nenhuma URL externa nem imagem rasterizada, que o arquivo tem menos de 100 KB e que ele abre do disco sem erros no console do navegador
- [x] 1.3 Trocar os elementos que o St não tem pelos equivalentes da tabela da Decisão 3: tirar o `<input type=color>` e o `<select>`, e trocar a fonte por uma pilha local, sem fonte da web. Verificar com `grep -nE 'type="color"|<select|fonts.googleapis'` que nenhum resultado fica dentro do painel simulado (os `<select>` do painel de cenários, fora da barra, são só da revisão). O `backdrop-filter` voltou com o canvas (grupo 8) e vira `Shell.BlurEffect` na extensão

## 2. Daemon simulado e cenários

- [x] 2.1 Implementar o `fakeDaemon` com os nomes da interface real (`Devices` no formato `(id, nome, status)` da Decisão 5, `Apply`, `LastApplied`, `ActiveProfile`, `List`, `Get`, `Activate` e as propriedades de telemetria, com a temperatura da CPU opcional), latência configurável (padrão de ~400 ms) e erros `io.github.leoodaviid.Halo1.Error.*`. Verificar no console do navegador que um `Apply` válido atualiza o `LastApplied` e que um erro injetado rejeita com o nome do erro
- [x] 2.2 Criar o painel de cenários fora da barra simulada, com cada controle da lista da Decisão 4 (daemon, aparelhos, `LastApplied` vazio, erro do próximo `Apply`, telemetria, CPU, perfis, tema e destaque), e o registro de pedidos com o contador de `Apply`. Verificar no navegador que cada controle muda o estado do `fakeDaemon` e que o registro mostra cada pedido com os argumentos

## 3. Barra e disponibilidade do daemon

- [x] 3.1 Implementar o indicador com o ícone e a temperatura arredondada e com algarismos tabulares (design, Decisão 8), abrindo e fechando o painel. Verificar no navegador os cenários "Temperatura disponível" e "Abrir e fechar" da spec `gnome-panel`, além de 38,5 °C mostrar `39°`
- [x] 3.2 Implementar os estados da temperatura e a simulação das preferências (fonte: líquido ou CPU, com CPU desabilitada sem o dado). Verificar no navegador os cenários "Padrão", "CPU indisponível no daemon", "CPU escolhida e depois indisponível", "Líquido inválido" e "Telemetria indisponível"
- [x] 3.3 Implementar o estado do daemon fora do ar (ícone esmaecido, painel sem controles ativos) e a volta sem reabrir. Verificar no navegador os cenários "Daemon fora do ar" e "Daemon volta"

## 4. Carrossel de aparelhos

- [x] 4.1 Ligar o carrossel ao `Devices` simulado, com a ordem e o status do daemon, a navegação escondida com um aparelho só e o tratamento do `DevicesChanged`. Verificar no navegador os cenários "Lista do daemon", "Um aparelho só" e "Aparelho desconectado"
- [x] 4.2 Manter a navegação do v2 (setas, roda com um passo por sequência, teclas de seta, indicador de página e volta circular). Verificar no navegador os cenários "Roda do mouse" e "Volta ao início"
- [x] 4.3 Separar cada desenho em corpo e LED (design, risco do anel tingido), escolher o desenho pelo `id` e criar o desenho genérico com o nome. Mostrar o aviso de controle em desenvolvimento, com o liga/desliga desabilitado, nos aparelhos `detected`. Verificar no navegador os cenários "Placa-mãe detectada", "Sem rede" (com a rede do navegador desligada nas ferramentas de desenvolvedor) e "Aparelho sem desenho próprio"

## 5. Controles de iluminação

- [x] 5.1 Implementar a lista dos 9 efeitos de uma cor, num menu no estilo do submenu do Shell, com a cor e a velocidade aparecendo conforme o efeito. Verificar no navegador os cenários "Efeito com cor e velocidade", "Efeito sem cor" e "Efeito estático"
- [x] 5.2 Implementar o seletor de cor com swatches, a barra de matiz arrastável e o campo hex com validação, tingindo só o desenho e o seletor com a cor do LED. Verificar no navegador os cenários "Cor pronta", "Hex inválido" e "Cor do LED"
- [x] 5.3 Implementar a máquina de estados da Decisão 6 (ociosa, aplicando e pendente, com o último valor vencendo, debounce de 300 ms nos swatches, nada enviado quando o valor for igual ao aplicado e a volta ao `LastApplied` em erro). Verificar no navegador, pelo contador do registro de pedidos, os cenários "Arrastar o brilho" (1 `Apply`), "Mudança durante um pedido em andamento" (2 `Apply` no total) e "Valor igual ao aplicado" (0 `Apply`)
- [x] 5.4 Implementar o estado inicial a partir do `LastApplied`, o aviso de estado desconhecido e a exibição de efeitos de paleta só para leitura. Verificar no navegador os cenários "Daemon com estado aplicado", "Daemon sem estado aplicado" e "Perfil com tri-color"
- [x] 5.5 Implementar o liga/desliga com a ordem da Decisão 7 (último estado ligado da sessão, LED do perfil ativo e `cycle`). Verificar no navegador os cenários "Desligar", "Religar com estado conhecido" e "Religar sem estado conhecido"

## 6. Perfis, detalhes e erros

- [x] 6.1 Implementar o menu de perfis no cabeçalho com `Activate` e a atualização dos controles. Verificar no navegador os cenários "Ativar perfil" e "Nenhum perfil"
- [x] 6.2 Implementar a tela de detalhes com fan, bomba e líquido só para leitura, o comando da CLI equivalente e a ação de copiar, sem nenhum controle de velocidade. Verificar no navegador os cenários "Telemetria válida", "Comando da CLI" e "Detalhes do Waterforce"
- [x] 6.3 Implementar as mensagens de erro no painel para `NotFound`, `Busy`, `PermissionDenied` e `Device`. Verificar no navegador os cenários "Aparelho ocupado" e "Sem permissão", e que os controles voltam ao último estado aplicado nos quatro erros
- [x] 6.4 Implementar a troca de tema claro/escuro e de destaque com o painel aberto. Verificar no navegador o cenário "Troca para o escuro" e que a cor do LED não muda o destaque

## 7. Documentação

- [x] 7.1 Criar `docs/ui/README.md` dizendo o que é o protótipo, como abrir, o que cada cenário simula, que ele é o registro do painel aprovado e que não é atualizado depois que a extensão existir. Verificar que os passos de "como abrir" funcionam como escritos
- [x] 7.2 Atualizar o `docs/roadmap.md` (Fase 2.2 feita, com os pré-requisitos do daemon para a 2.3), o `docs/architecture.md` (o painel, a tabela de equivalência com o St e as dependências do `halod`) e o `openspec/changes/gnome-shell-extension/README.md` (seguir `docs/ui/panel-prototype.html` e a spec `gnome-panel`). Verificar que os links relativos abrem os arquivos certos

## 8. Refino pelo canvas "halo · Apple HIG"

- [x] 8.1 Trocar os tokens do Shell pelos do canvas (claro e escuro), com o destaque do GNOME e o `--accent-strong` da decisão 2, a pilha de fontes local e a barra de menus do macOS simulada, com a temperatura numa pílula. Verificar no navegador, lado a lado com os artboards "Painel · claro" e "Painel · escuro" do canvas, e com `grep -n fonts.googleapis` sem resultados
- [x] 8.2 Refazer a página principal no layout do canvas: cabeçalho, bloco do aparelho com o carrossel, bloco de efeito e cor (popup, hex na linha da cor, swatches e matiz), bloco dos sliders no estilo da Central de Controle, a velocidade desabilitada com "não se aplica", os controles esmaecidos com o LED desligado, a paleta "definida pelo perfil" com brilho e velocidade editáveis, a linha de status e os itens "Detalhes do aparelho…" e "Preferências do halo…". Verificar no navegador os cenários "Efeito estático", "LED desligado", "Brilho com efeito de paleta", "Aplicando com fila", "Aplicado" e "Preferências"
- [x] 8.3 Refazer Aparelhos e Detalhes no layout do canvas (lista agrupada com o status e o ✓ do selecionado, "‹ halo" para voltar, telemetria com rpm e duty em %, efeito, brilho e velocidade, e os dados da placa-mãe). Verificar no navegador os cenários "Telemetria válida", "Detalhes da placa-mãe" e "Comando da CLI"
- [x] 8.4 Refazer o estado do daemon fora do ar e o banner de erro do canvas, com "Iniciar halod", "Ver registro" (página com o registro simulado) e "Tentar de novo", e o cenário "unidade systemd instalada" (decisão 9). Verificar no navegador os cenários "Iniciar o halod", "Ver o registro", "Unidade não instalada" e "Tentar de novo"
- [x] 8.5 Atualizar o `docs/ui/README.md` e a seção "Painel do GNOME Shell" do `docs/architecture.md` com o visual do canvas, o `Shell.BlurEffect`, os sliders e as ações do systemd. Verificar que os links relativos continuam abrindo os arquivos certos

## 9. Aprovação

- [x] 9.1 Percorrer de novo todos os cenários da spec `gnome-panel` no protótipo, em claro e em escuro, e anotar o resultado de cada um. Verificar que todos passam e que `openspec validate gnome-panel-prototype --strict` passa
- [x] 9.2 Apresentar o protótipo ao usuário para aprovação, com o caminho do arquivo, e publicá-lo como página privada só se ele pedir. Verificar com a aprovação explícita do usuário. Se ele pedir mudanças, elas voltam para as tasks dos grupos 3 a 8, e a spec é atualizada se o comportamento mudar

## Workflow follow-up

- Sincronizar a spec `gnome-panel`, arquivar a change e commitar, quando o usuário pedir.
- Abrir a change do daemon para o `Devices` com status, antes da `gnome-shell-extension`.
