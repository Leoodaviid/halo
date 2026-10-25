# gnome-panel Specification

## Purpose

Define o comportamento do painel do halo no GNOME Shell: o indicador na barra superior com a temperatura, o painel que abre ao clicar, os controles de iluminação, os perfis, os detalhes e como cada estado de erro aparece. O painel é só um cliente do `halod` e não fala com o hardware.

## Requirements

### Requirement: Indicador na barra superior
O painel SHALL ter um indicador na área de status da barra superior do GNOME Shell, com o ícone do halo seguido da temperatura em graus inteiros (por exemplo, `38°`). Clicar no indicador SHALL abrir e fechar o painel abaixo dele.

#### Scenario: Temperatura disponível
- **WHEN** o daemon publica uma temperatura válida de 38,4 °C para a fonte escolhida
- **THEN** a barra mostra o ícone do halo e `38°`

#### Scenario: Abrir e fechar
- **WHEN** o usuário clica no indicador com o painel fechado e depois clica de novo
- **THEN** o painel abre abaixo do indicador no primeiro clique e fecha no segundo

### Requirement: Fonte da temperatura configurável
As preferências SHALL oferecer duas fontes de temperatura para a barra: o líquido do Waterforce, que é o padrão, e a CPU. A opção CPU SHALL ficar desabilitada enquanto o daemon não publicar a temperatura da CPU. A troca de fonte SHALL valer sem reabrir o painel nem reiniciar o Shell.

#### Scenario: Padrão
- **WHEN** o usuário nunca alterou as preferências
- **THEN** a barra mostra a temperatura do líquido

#### Scenario: CPU indisponível no daemon
- **WHEN** o usuário abre as preferências e o daemon não publica a temperatura da CPU
- **THEN** a opção CPU aparece desabilitada, com a explicação de que o daemon ainda não fornece esse valor

#### Scenario: CPU escolhida e depois indisponível
- **WHEN** a fonte escolhida é a CPU e o daemon deixa de publicá-la
- **THEN** a barra mostra só o ícone, sem trocar a fonte sozinha para o líquido

### Requirement: Temperatura inválida ou ausente
A barra MUST NOT mostrar uma temperatura do líquido que o daemon marcou como inválida. Um valor inválido SHALL aparecer como `--`. Quando o daemon não tiver leitura da fonte escolhida, a barra SHALL mostrar só o ícone.

#### Scenario: Líquido inválido
- **WHEN** o daemon publica o líquido em 224 °C com a validade falsa
- **THEN** a barra mostra o ícone e `--`, e nunca `224°`

#### Scenario: Telemetria indisponível
- **WHEN** o daemon informa que a telemetria do Waterforce não está disponível
- **THEN** a barra mostra só o ícone

### Requirement: Daemon indisponível
Quando o `halod` não estiver no D-Bus da sessão, o indicador SHALL continuar na barra, só com o ícone em estado esmaecido. O painel SHALL mostrar que o serviço não está rodando, sem nenhum controle ativo. Quando o daemon voltar, o painel SHALL se atualizar sozinho.

#### Scenario: Daemon fora do ar
- **WHEN** o usuário abre o painel e o nome `io.github.leoodaviid.Halo1` não tem dono no D-Bus da sessão
- **THEN** o painel mostra que o serviço do halo não está rodando, e nenhum controle de iluminação ou perfil pode ser acionado

#### Scenario: Daemon volta
- **WHEN** o daemon é iniciado com o painel aberto
- **THEN** o painel passa a mostrar os aparelhos e os controles sem que o usuário precise fechá-lo e reabri-lo

### Requirement: Iniciar o serviço e ver o registro
Com o daemon fora do ar, o painel SHALL oferecer duas ações: iniciar o serviço do usuário `halod.service` e mostrar no próprio painel as últimas linhas do registro dele. Enquanto a unidade não estiver instalada, as duas ações SHALL ficar desabilitadas, com a explicação de que o serviço não está instalado.

#### Scenario: Iniciar o halod
- **WHEN** o daemon está fora do ar, a unidade `halod.service` está instalada e o usuário escolhe iniciar o halod
- **THEN** o painel pede ao systemd do usuário que inicie a unidade e, quando o nome do daemon aparecer no D-Bus, mostra os aparelhos e os controles

#### Scenario: Ver o registro
- **WHEN** o daemon está fora do ar e o usuário escolhe ver o registro
- **THEN** o painel mostra as últimas linhas do registro do `halod.service`, só para leitura, com uma ação para voltar

#### Scenario: Unidade não instalada
- **WHEN** o daemon está fora do ar e a unidade `halod.service` não existe
- **THEN** as ações de iniciar e de ver o registro aparecem desabilitadas, com a explicação de que o serviço não está instalado

### Requirement: Aparelhos vindos do daemon
O carrossel SHALL mostrar só os aparelhos que o daemon lista como presentes, na ordem da lista, com o nome que o daemon informa e o status "Disponível" quando o aparelho tem a capacidade `led`, ou "Detectado" quando não tem. O painel MUST NOT ter um catálogo próprio de hardware. Com um único aparelho, a navegação do carrossel (setas e indicador de página) SHALL ficar oculta.

#### Scenario: Lista do daemon
- **WHEN** o daemon lista o Waterforce presente com a capacidade `led`, a placa-mãe presente sem capacidades e o G502 ausente
- **THEN** o carrossel tem duas páginas, o Waterforce como "Disponível" e a placa-mãe como "Detectado", nessa ordem, sem o G502

#### Scenario: Um aparelho só
- **WHEN** o daemon lista só o Waterforce como presente
- **THEN** o carrossel mostra o Waterforce sem setas e sem indicador de página

#### Scenario: Aparelho desconectado
- **WHEN** o daemon anuncia que a lista mudou e o aparelho da página atual deixou de estar presente
- **THEN** o carrossel vai para o primeiro aparelho presente da nova lista, ou mostra que nenhum aparelho foi encontrado se nenhum estiver presente

#### Scenario: Nome muda
- **WHEN** o daemon anuncia o Waterforce com o nome `AORUS Waterforce X 240` no lugar de `AORUS Waterforce X`
- **THEN** o carrossel continua na mesma página e mostra o nome novo

### Requirement: Navegação do carrossel
O usuário SHALL poder trocar de aparelho pelas setas, pela roda do mouse sobre a imagem, pelas teclas de seta com o carrossel em foco e pelo indicador de página. A navegação SHALL ser circular.

#### Scenario: Roda do mouse
- **WHEN** o usuário rola a roda para baixo uma vez sobre a imagem do aparelho
- **THEN** o carrossel avança um aparelho, e uma sequência rápida de eventos da roda conta como um passo só

#### Scenario: Volta ao início
- **WHEN** o último aparelho está selecionado e o usuário avança
- **THEN** o primeiro aparelho é selecionado

### Requirement: Aparelho sem controle
Um aparelho presente sem a capacidade `led` SHALL aparecer com o aviso de controle em desenvolvimento no lugar dos controles, com o status "Detectado". O liga/desliga SHALL ficar desabilitado nessa página.

#### Scenario: Placa-mãe detectada
- **WHEN** o usuário navega até a placa-mãe, listada sem capacidades
- **THEN** o painel mostra o aviso de controle em desenvolvimento, sem cor, efeito, brilho ou velocidade, e com o liga/desliga desabilitado

### Requirement: Imagens locais
Cada aparelho SHALL ser representado por um desenho vetorial que vem com a extensão. O painel MUST NOT buscar imagens ou outros recursos na rede.

#### Scenario: Sem rede
- **WHEN** o computador está sem conexão com a internet
- **THEN** o painel mostra os desenhos de todos os aparelhos normalmente

#### Scenario: Aparelho sem desenho próprio
- **WHEN** o daemon lista um aparelho para o qual a extensão não tem desenho
- **THEN** o painel mostra um desenho genérico com o nome que o daemon informou

### Requirement: Controles de iluminação do Waterforce
No Waterforce, o painel SHALL oferecer cor, efeito, brilho (1 a 10) e velocidade (1 a 5). Os efeitos SHALL ser os nove de uma cor: estático, pulso, flash, flash duplo, gradiente, ciclo, onda, arco-íris e desligado. A cor SHALL aparecer só nos efeitos com cor. Nos efeitos sem animação, a velocidade SHALL aparecer desabilitada, como "não se aplica".

#### Scenario: Efeito com cor e velocidade
- **WHEN** o usuário escolhe o efeito pulso
- **THEN** o painel mostra cor, brilho e velocidade

#### Scenario: Efeito sem cor
- **WHEN** o usuário escolhe o efeito ciclo
- **THEN** o painel mostra brilho e velocidade, sem o seletor de cor

#### Scenario: Efeito estático
- **WHEN** o usuário escolhe o efeito estático
- **THEN** o painel mostra cor e brilho, e a velocidade aparece desabilitada, como "não se aplica"

### Requirement: Controles com o LED desligado
Com o LED desligado, o painel SHALL mostrar a cor, o brilho e a velocidade esmaecidos, com os últimos valores, sem aceitar mudanças. A escolha de efeito e o liga/desliga SHALL continuar disponíveis.

#### Scenario: LED desligado
- **WHEN** o efeito aplicado é o desligado
- **THEN** cor, brilho e velocidade aparecem esmaecidos e não respondem a cliques nem a arraste, e o efeito e o switch continuam ativos

### Requirement: Escolha de cor
O seletor de cor SHALL oferecer uma fileira de cores prontas, uma barra de matiz e um campo hexadecimal de seis dígitos, que mostra o valor atual. O hex SHALL ser confirmado com Enter ou ao sair do campo. Um valor hexadecimal inválido MUST NOT ser aplicado.

#### Scenario: Cor pronta
- **WHEN** o usuário clica na cor laranja da fileira
- **THEN** a cor atual passa a `#FF6600` e é aplicada

#### Scenario: Hex inválido
- **WHEN** o usuário digita `#12345G` no campo hexadecimal
- **THEN** o campo indica o erro, nenhum pedido é enviado ao daemon e a cor atual não muda

### Requirement: Aplicação só ao concluir o gesto
Cada escrita no LED grava no aparelho. Por isso, o painel SHALL pedir uma aplicação só ao fim de cada gesto: ao soltar um slider, ao escolher um efeito ou cor, ou ao confirmar o hex. Durante o arraste, a mudança SHALL aparecer só na interface. O painel MUST NOT ter mais de um pedido de aplicação em andamento nem repetir pedidos em laço.

#### Scenario: Arrastar o brilho
- **WHEN** o usuário arrasta o brilho de 3 até 9 e solta
- **THEN** o valor mostrado acompanha o arraste, e o painel envia um único pedido de aplicação, com brilho 9, depois de soltar

#### Scenario: Mudança durante um pedido em andamento
- **WHEN** o usuário muda a cor duas vezes enquanto um pedido de aplicação ainda não terminou
- **THEN** o painel espera o pedido em andamento terminar e envia um único pedido, com a última cor escolhida

#### Scenario: Valor igual ao aplicado
- **WHEN** o usuário solta o slider no mesmo valor que já está aplicado
- **THEN** nenhum pedido é enviado

#### Scenario: Pedido em andamento falha com uma mudança esperando
- **WHEN** o usuário muda a cor enquanto um pedido de aplicação está em andamento, e esse pedido falha
- **THEN** o painel descarta a mudança que esperava, não envia mais nenhum pedido, e os controles voltam ao último estado aplicado

### Requirement: Estado mostrado vem do daemon
Ao abrir, o painel SHALL mostrar o último estado de LED que o daemon informa como aplicado. Sem esse dado, o painel SHALL mostrar os controles sem seleção de efeito e indicar que o estado atual do aparelho não é conhecido. Depois de um erro, os controles SHALL voltar ao último estado aplicado.

#### Scenario: Daemon com estado aplicado
- **WHEN** o daemon informa como último estado o pulso laranja, com brilho 8 e velocidade 2
- **THEN** o painel abre com pulso, `#FF6600`, brilho 8 de 10 e velocidade 2 de 5

#### Scenario: Daemon sem estado aplicado
- **WHEN** o daemon não tem nenhum LED aplicado registrado
- **THEN** o painel indica que o estado atual do LED não é conhecido e não envia nada até o usuário escolher um efeito

#### Scenario: Depois de sair e entrar na sessão
- **WHEN** o usuário aplicou o estático vermelho, saiu da sessão e entrou de novo
- **THEN** o painel abre com estático, `#FF0000` e o switch ligado, sem enviar nada ao aparelho

### Requirement: Liga e desliga
O liga/desliga do cabeçalho SHALL desligar o LED do aparelho selecionado com o efeito desligado. Religar SHALL reaplicar o último estado ligado que o daemon conhece. Sem esse dado, SHALL reaplicar o LED do perfil ativo e, sem perfil com LED, o efeito ciclo, o mais próximo do modo de fábrica.

#### Scenario: Desligar
- **WHEN** o usuário desliga com o pulso laranja aplicado
- **THEN** o painel pede o efeito desligado, e o switch fica desligado

#### Scenario: Religar com estado conhecido
- **WHEN** o usuário religa depois de desligar o pulso laranja na mesma sessão do daemon
- **THEN** o painel pede de novo o pulso laranja com o mesmo brilho e a mesma velocidade

#### Scenario: Religar depois de entrar na sessão
- **WHEN** o usuário desliga o LED pelo switch depois de entrar na sessão, com o estático vermelho registrado pelo daemon, e religa
- **THEN** o painel pede de novo o estático vermelho

#### Scenario: Religar sem estado conhecido
- **WHEN** o usuário religa sem estado ligado conhecido (o daemon não tem LED registrado, ou o último registrado é o desligado), sem perfil ativo com LED
- **THEN** o painel pede o efeito ciclo

#### Scenario: Religar com o perfil ativo desligado
- **WHEN** o usuário religa sem estado ligado conhecido, e o LED do perfil ativo é o efeito desligado
- **THEN** o painel pede o efeito ciclo, e não o desligado do perfil

#### Scenario: Reboot com corte de energia
- **WHEN** o usuário aplicou o estático branco pelo painel, desligou o PC com corte de energia e ligou de novo
- **THEN** o painel abre com estático, `#FFFFFF` e o switch ligado, sem Apply no login nem ao abrir o painel; desligar e religar pedem off e o mesmo estático branco

### Requirement: Efeitos de paleta só para leitura
Os efeitos de paleta (color-shift, tri-color, spin, switch, lego e aurora) MUST NOT ser oferecidos para escolha. Se o estado aplicado for um deles, o painel SHALL mostrar o nome e as cores da paleta sem permitir editá-los. Brilho e velocidade SHALL continuar editáveis, com a mesma paleta, e escolher outro efeito SHALL continuar possível.

#### Scenario: Perfil com tri-color
- **WHEN** o perfil ativo aplicou o tri-color vermelho, verde e azul
- **THEN** o painel mostra o tri-color com as três cores, sem edição da paleta, e a lista de efeitos continua oferecendo só os nove efeitos de uma cor

#### Scenario: Brilho com efeito de paleta
- **WHEN** o tri-color vermelho, verde e azul está aplicado e o usuário solta o brilho em 5
- **THEN** o painel pede o tri-color com as mesmas três cores e brilho 5

### Requirement: Perfis no cabeçalho
O cabeçalho SHALL mostrar o perfil ativo num menu com os perfis salvos no daemon. Escolher um perfil SHALL pedir ao daemon que o ative. Os controles SHALL refletir o resultado. Sem perfis salvos, o menu SHALL indicar que não há perfis, sem a opção de ativar.

#### Scenario: Ativar perfil
- **WHEN** o usuário escolhe o perfil `noite` no menu
- **THEN** o painel pede a ativação de `noite`, e o menu e os controles passam a mostrar o estado desse perfil quando a ativação termina

#### Scenario: Nenhum perfil
- **WHEN** o daemon não tem perfis salvos
- **THEN** o menu de perfis indica que não há perfis e não oferece nenhuma ativação

### Requirement: Detalhes do aparelho
Os detalhes SHALL mostrar, só para leitura, no Waterforce: RPM e duty em % da fan e da bomba, o líquido (inválido quando o daemon disser) e o efeito, o brilho e a velocidade aplicados, além do comando da CLI equivalente com uma ação para copiá-lo. Num aparelho sem controle, SHALL mostrar o controlador, a conexão e que a iluminação ainda não tem suporte.

#### Scenario: Telemetria válida
- **WHEN** o usuário abre os detalhes do Waterforce, com a fan em 1200 RPM e duty 143, a bomba em 2600 RPM e duty 224, e o líquido em 31 °C
- **THEN** a tela mostra a fan em 1.200 rpm e 56 %, a bomba em 2.600 rpm e 88 % e o líquido em 31,0 °C, sem controles para alterá-los

#### Scenario: Detalhes da placa-mãe
- **WHEN** o usuário abre os detalhes da placa-mãe, listada como detectada
- **THEN** a tela mostra o controlador IT5702, a conexão USB HID e que a iluminação ainda não tem suporte, sem o comando da CLI

#### Scenario: Comando da CLI
- **WHEN** o estado atual é pulso, `#FF6600`, brilho 8 e velocidade 2
- **THEN** a tela mostra `halo led waterforce pulse --color ff6600 --brightness 8 --speed 2`, e a ação de copiar coloca esse texto na área de transferência

### Requirement: Sem controle de velocidade
O painel MUST NOT oferecer controles que alterem o modo ou a curva da fan e da bomba.

#### Scenario: Detalhes do Waterforce
- **WHEN** o usuário percorre o painel e os detalhes do Waterforce
- **THEN** não há controle de presets, de curva ou de restauração da velocidade

### Requirement: Erros de aplicação
Quando o daemon recusar um pedido, o painel SHALL mostrar uma mensagem curta, conforme o erro: aparelho não encontrado, ocupado por outro processo, sem permissão (com a regra udev) ou falha de comunicação. Os controles SHALL voltar ao último estado aplicado. A mensagem SHALL ter uma ação para tentar de novo, que reenvia o pedido que falhou.

#### Scenario: Aparelho ocupado
- **WHEN** o usuário aplica uma cor enquanto um `halo probe waterforce` mantém o aparelho aberto, e o daemon responde que ele está ocupado
- **THEN** o painel mostra que o aparelho está em uso por outro processo, e a cor volta à última aplicada

#### Scenario: Sem permissão
- **WHEN** o daemon responde que não tem permissão para abrir o aparelho
- **THEN** o painel mostra que falta a regra udev do halo

#### Scenario: Tentar de novo
- **WHEN** a cor roxa falhou com o aparelho ocupado e o usuário escolhe tentar de novo quando ele está livre
- **THEN** o painel envia um único pedido com a cor roxa, e a mensagem some quando ele dá certo

### Requirement: Linha de status
Abaixo dos controles, o painel SHALL mostrar uma linha de status: aplicando (com quantos pedidos esperam na fila), aplicado ao aparelho selecionado, LED desligado, ou estado desconhecido.

#### Scenario: Aplicando com fila
- **WHEN** um pedido está em andamento e outro espera
- **THEN** a linha de status mostra que está aplicando e que há um pedido na fila

#### Scenario: Aplicado
- **WHEN** o último pedido para o Waterforce terminou sem erro e o LED está ligado
- **THEN** a linha de status mostra que o estado foi aplicado ao Waterforce X 240

### Requirement: Itens de menu do painel
O fim da página principal SHALL ter dois itens de menu: um que abre os detalhes do aparelho selecionado e outro que abre as preferências da extensão.

#### Scenario: Preferências
- **WHEN** o usuário escolhe as preferências do halo no painel
- **THEN** a janela de preferências da extensão abre, com a fonte da temperatura da barra

### Requirement: Tema do sistema
O painel SHALL seguir o estilo claro ou escuro do sistema e a cor de destaque do GNOME, e SHALL acompanhar a troca de tema com o painel aberto. A cor do LED aplicada SHALL aparecer só no desenho do aparelho e no seletor de cor, sem alterar a cor de destaque dos outros controles.

#### Scenario: Troca para o escuro
- **WHEN** o usuário troca o sistema do estilo claro para o escuro com o painel aberto
- **THEN** o painel passa ao estilo escuro sem fechar

#### Scenario: Cor do LED
- **WHEN** a cor aplicada é azul e o destaque do sistema é laranja
- **THEN** o anel no desenho do Waterforce aparece azul, e o switch, a seleção das cores e os destaques dos menus continuam laranja

### Requirement: Painel não inicia o daemon sozinho
O painel SHALL só observar se o nome `io.github.leoodaviid.Halo1` tem dono no D-Bus da sessão e MUST NOT provocar a ativação do daemon ao ler a temperatura, a lista de aparelhos ou qualquer outra informação. Iniciar o daemon SHALL acontecer só pela ação de iniciar o serviço.

#### Scenario: Daemon parado pelo usuário
- **WHEN** o usuário para o `halod.service` com o painel ativo na barra
- **THEN** o indicador fica esmaecido e o daemon continua parado, sem ser reiniciado pelo painel

#### Scenario: Iniciar depois de falhas seguidas
- **WHEN** o `halod.service` parou depois de falhar várias vezes seguidas e o usuário escolhe iniciar o halod
- **THEN** o painel limpa o estado de falha da unidade antes de pedir o início, e o daemon sobe
