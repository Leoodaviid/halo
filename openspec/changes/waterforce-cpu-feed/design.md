# Design

## Context

Ver `proposal.md` para motivação. A implementação atual usa um worker bloqueante
serial em `crates/halo-daemon/src/worker.rs`; `ticks.rs` só lê sysfs a cada 2 s;
`bus.rs` publica Telemetry sem temperatura CPU. `Hidraw::open` toma flock
exclusivo por até 5 s. A unidade `packaging/systemd/halod.service` tem Type=dbus,
PartOf=graphical-session.target, Restart=on-failure e RestartSec=2, sem stop
helper, watchdog ou tratamento de SIGTERM no processo.

### Evidências e limites

Fontes primárias locais: `docs/hardware.md`, seções “E0 como referência” e
“Interrupção após E0 frio”; `experimentos.md`; logs JSONL dos dois ensaios.

| Medida | Resultado | Consequência |
|---|---|---|
| Curva 0:2200/30:2200/50:2350/80:2500; E0 80→50→30→80 | Duty DA 100→94→88→100, Tctl real ~37,6–40,5 °C | E0 controla a referência; usar Tctl do host |
| Primeiro DA no duty esperado | 3,780 / 1,819 / 5,853 / 3,835 s | São atrasos observados, incluindo DA/cache; não latência interna exata |
| Parar E0 depois de 80 °C | Duty 100 após 70,4774 s sem envio | Não depender de timeout do firmware |
| Parar E0 depois de 30 °C | Duty 88 após 70,6174 s; Tctl 40,75 °C | Morte abrupta pode reter o patamar frio |
| E0 final 80 °C, ensaio 2 | Duty 100 após 2,4372 s; 2550 RPM | 80 °C é o valor de proteção medido |
| Restaurar Performance/Turbo e slots originais | Duty final 100, com último E0 em 80 °C | Restaurar modos/slots não desfaz E0; afeta também Performance |

DA[8] é porcentagem observada; pwm1 do hwmon é 0–255. RPM oscila: em 50 °C
chegou a 2706 no fim de 12 s apesar do duty 94. Aceite exige log mais longo
com RPM subindo e descendo, não só extrapolação desses patamares.

## Goals / Non-Goals

**Goals:** fechar o contrato de envio e recuperação em uma unidade de usuário;
reagir ao Tctl, permitir degraus sem editor geral de CLI e definir testes que
separem envio aceito no host de reação observada no firmware.

**Non-Goals:** garantia contra kernel travado, morte do gerenciador do usuário,
falha de energia/USB ou perda de ACL. Não instalar protetor de sistema. Não
alterar a tela nem atribuir ao byte ambíguo VRAM/VRM um sensor não demonstrado.

## Decisions

### 1. Fontes, protocolo e modelo

Descobrir por VID:PID `1044:7a4d`, usage page 0; conferir DE 00 antes do primeiro
E1/E0 de cada sessão lógica do dispositivo. Uma sessão lógica começa no
startup, reconexão física, mudança de identidade/geração do sysfs ou resume;
abrir e fechar hidraw entre ticks não encerra essa sessão. Invalidar o modelo
e nome enviados ao detectar mudança de geração, mesmo se o número hidraw for
reutilizado. O aceite físico desta change é no X 240, modelo 0,
firmware 1.6 dos ensaios. Não habilitar automaticamente em modelos 1/2 sem
validação específica; outros controles já suportados continuam como antes.
Se o DE da sessão lógica responde modelo 1/2 ou fora da tabela, o feed fica
desabilitado até a próxima sessão lógica: nenhum E1/E0, CpuFeedActive=false,
SetFanCurve recusado e o watchdog trata o caso como ausência (seção 6).
DE sem resposta ou com erro não é modelo conhecido: conta como falha.
E1 precede o primeiro E0 de cada sessão lógica e no comando de proteção;
não é reenviado em todo tick só porque o fd foi reaberto.
É compatibilidade com o serviço oficial, não afirmação de que E1 é obrigatório.

Fonte dos bytes: decompilados em
`/mnt/workspace/tools/samples/gigabyte-control-center/extracted/`:
`ilspy-aoruslcdservice/AorusLcdService.API.Cooler/CoolerDataApi.cs`,
`SendCpuName`/`SendCoolerData`, e `CoolerApi.cs`, `Send`;
`CpuDataApi.cs`, `MakeCpuData`, explica a aquisição no Windows. Análise REA
registrada em `docs/protocols/castor3.md`, “Dados de sistema”.

| Byte no output report | Conteúdo e fonte Linux |
|---|---|
| 0 / 1 | 99 / E0, literais de SendCoolerData |
| 2 | Fabricante: 1 para GenuineIntel, 0 para AuthenticAMD; /proc/cpuinfo; ausente/outro = 0 |
| 3 | Temperatura: Tctl, arredondar para cima e saturar em 80 °C; inválida = 80 |
| 4 | Threads online em /sys/devices/system/cpu/online, limitado a 255; indisponível = 0 |
| 5 / 6 | GHz inteiro/décimo = 0/0 nesta change; não inventar frequência instantânea |
| 7 | Núcleos: pares únicos physical_package_id/core_id das CPUs online no sysfs, até 255; indisponível = 0 |
| 8 | Campo chamado VRAMTemperature no decompilado, VRM na tabela antiga: 0, sem sensor atribuído |
| 9 | Temperatura do líquido = 0, como no ramo 7A4D do GCC; não usar DA como fonte CPU |
| 10 | Uso = 0; carga é refletida por Tctl, não é necessário novo coletor de uso |
| 11 / 12 | W little-endian = 0/0; sem RAPL/SMBus ou estimativa de potência |

Todos os reports completados com zeros até 6144 bytes. E1 é
`99 E1 <len:u8> <ASCII>`, nome de /proc/cpuinfo, substituir não ASCII por `?`,
limitar a 30 bytes (o limite do `GetCpuShortName` do GCC, o maior que o
firmware já recebeu; não é limite descoberto do firmware),
fallback `CPU`. Comprimento conta bytes. Metadados indisponíveis não impedem
proteção: usar os zeros documentados; essa degradação tem teste falso e deve
ser exercitada antes do aceite físico. Nenhum E0/E1 envia B6 ou snapshots.

Tctl: achar hwmon pelo nome k10temp e canal por `temp*_label = Tctl`, sem
índice fixo. Valor inteiro de 0 a 125000 m°C é válido; acima de 80000 continua
visível no D-Bus e envia 80 no fio, o maior ponto de controle demonstrado.
Ausência, ambiguidade ou falha de leitura invalida a amostra e envia proteção;
nunca enviar 0 °C por falha. Não cair para Tdie, gigabyte_wmi ou líquido.
Os limites de validade são política defensiva, não uma especificação térmica
universal. Intel sem Tctl recebe proteção; ampliar sensores é outra change.

Alternativas: uso/GHz/W reais aumentam aquisição sem contribuir ao controle
exigido; permanecem zero como no ensaio. Não copiar a dependência WMI/GbtCpuDll.
A ausência de B6 no laço oficial sustenta tratá-lo como dados voláteis no host,
mas não prova ausência de escrita interna em flash. Não prometer essa prova.

### 2. Cadência e estado inicial

Enviar a cada **2 s**, mesmo com Tctl constante. Reutilizar a cadência existente,
com prazos monotônicos e uma só amostra pendente. Não produzir rajadas após
atraso nem reexecutar amostras velhas: reler Tctl imediatamente antes de enviar.
Os atrasos medidos de ~1,8–5,9 s com observação DA a ~2 s justificam começar
nessa escala, sem tráfego do laço sem Sleep do GCC. Não demonstram que 2 s
seja ótimo: o ensaio de carga e os logs de intervalos são critério de aceite;
se falharem, revisar os artefatos, não trocar o período silenciosamente.
Dois envios do slot ficam a pelo menos 1 s um do outro (uma volta extra dos
ticks por modelo novo, operação de velocidade ou recuperação não gera `E0`
colado); resume e sessão nova pedida por reset passam na hora. Para ensaios,
`HALOD_TRACE_FEED=1` no ambiente da unidade registra cada envio no journal.

Depois de adquirir o nome D-Bus, inicializar DE/E1/E0=80 e manter janela de
proteção de **8 s** antes de enviar temperatura real. Nessa janela repetir E0
seguro a cada 2 s; o prazo cobre o maior atraso observado mais um DA (~2 s),
sem alegar limite físico garantido. CpuFeedActive permanece false durante os 8 s e só pode ficar true após
o fim da janela e conclusão do primeiro envio baseado em Tctl recém-lido
(ou proteção por sensor inválido). SetFanCurve permanece recusado durante
a janela, inclusive se chegar durante startup/restart. Não reaplicar perfil, curva ou LED.
Apresentar modelo conhecido também quando descoberto pelo feed.

A medição após corte de energia é **pendência de leitura, com aval próprio**:
antes de instalar/ativar o feed novo, parar e impedir ativação automática do
halod, incluindo chamada D-Bus de clientes, desligar com corte de energia pelo
usuário e ler DA antes de qualquer E0. Registrar tempo, modos/slot e contexto
Windows anterior. O último E0 alto dos ensaios é conhecido. Um DA após reset
não lê a temperatura interna usada na curva: duty sob uma configuração conhecida
só limita hipóteses de padrão/persistência; não identifica a origem “Windows”
sozinho. Sem corte confirmado ou com halod já enviando, marcar inconclusivo.
Não executar corte, alteração de unidade ou leitura nesta fase de artefatos.

### 3. Exclusão mútua, fila e CLI

Feed passa pelo mesmo worker por abertura curta de hidraw, fecha o fd e solta
flock após cada envio; DE/E1 pertencem à inicialização da sessão lógica e E0
aos ticks seguintes. A operação confirma identidade/geração no sysfs a cada
abertura e reinicializa se mudou; nunca mantém fd aberto entre ticks. Preservar
ordem FIFO entre chamadas D-Bus. Entre chamadas, executar no máximo um feed
vencido antes da próxima chamada; não interromper uma sequência LED/E5/E6/B6.
Amostra periódica usa slot único e não a fila ilimitada de jobs atual.

CLI mantém espera de 5 s e erro Busy; feed respeita o mesmo limite, sem burlar
lock nem conversar pelo hwmon. Leitura de presença/temperatura/D-Bus não espera
worker. Contenção de 1–5 s é tolerada: perde-se o período, não se acumula backlog.
Falha de envio ou Busy encerra esse ciclo, tenta recuperação de 80 °C em nova
transação limitada e termina com erro se não recuperar; ExecStopPost tentará
independentemente. Um cliente que mantém lock indefinidamente pode impedir
ambas tentativas: registrar falha, não chamar de protegido. O watchdog detecta
worker parado, mas não pode retirar lock de um processo externo.

Após qualquer operação de velocidade, priorizar o próximo E0 atual dentro
do mesmo escalonamento; ao parar, cancelar amostras e novos pedidos antes da
proteção para não enviar frio depois. Não misturar DE/E1/E0 com snapshots nem
alterar os pacotes das operações legadas.

### 4. E0 de proteção em todos os modos

Usar **80 °C**, byte 3 = 0x50, com demais campos conforme tabela, independente
de DD: não condicionar a Customized. Houve duty 100 em Customized no ensaio e
em Performance após restauração. Não usar 100/255 °C como se fosse medido.
Não restaurar o E0 anterior, não reverter para 50 °C e não chamar restore.

“Proteção” designa uma temperatura alta de referência, não comando de duty
absoluto. Uma curva plana de 1500 RPM continuará plana; presets não ensaiados,
ZeroRPM ou uma curva cujo máximo fique acima de 80 °C não têm garantia de
100%. Não enviar E5/E6/B6 na proteção para esconder essa limitação.
Novas curvas da fan por esta change devem terminar em **80 °C:2500 RPM**;
curvas legadas/restaurações continuam válidas e têm esse limite explicitado.
O aceite de falha usa a curva ensaiada, não presume RPM máximo em qualquer
configuração. O feed normal também roda em presets, pois E0 os afeta.

### 5. SIGTERM e comando independente de ExecStopPost

No SIGTERM: parar a produção/aceitação de jobs, descartar feed pendente,
concluir ou limitar a operação em andamento e fazer DE/E1/E0 seguro antes de
sair; registrar sucesso/falha. O limite do caminho de proteção é **18 s**,
sem esperar indefinidamente pelo worker. Travamento nessa saída será limitado
pela unidade; não é justificativa para um signal handler fazer I/O diretamente.

Criar na implementação `halod --waterforce-cpu-safe`, modo de uma execução,
que não adquire nome D-Bus, não ativa daemon, não depende de perfis, snapshots,
Tctl nem diretório de estado. Descobre dispositivo, toma flock (até 5 s),
confere modelo (até 2 s), manda E1 e E0=80 com writes limitados, fecha fd,
espera 8 s para tornar observável a proteção antes do restart e termina.
Orçamento total 18 s; ausência do aparelho = saída 0 com log “ausente”.
Modelo 1/2 no DE = saída 0 com log “modelo sem feed, nada a proteger”, sem
E1/E0 nem espera de 8 s: o feed nunca roda nesses modelos (seção 1) e falhar
deixaria a unidade failed em toda parada normal. Modelo fora da tabela,
permissão, Busy ou falha = saída não zero e diagnóstico. Sem `-` para
mascarar falha no ExecStopPost. Write bloqueado exige transporte com prazo,
verificado nos testes; um syscall preso em estado D é risco de kernel.

SIGTERM e helper compartilham o gerador de pacotes e a operação de proteção.
Duas execuções sucessivas repetem E0=80, não desfazem nada nem gravam B6.
ExecStopPost roda depois do processo principal terminar inclusive falha de
startup; isso não é o mesmo que ExecStop nem depende de D-Bus ainda vivo.
Uma segunda instância com nome ocupado não inicia feed; o helper pode ainda
repetir E0 alto pela unidade que falhou, causando no máximo alta temporária.
Instalador precisa parar a unidade antiga antes de substituir o binário/unidade,
para o stop helper instalado combinar com as opções do binário em execução.

### 6. Unidade e watchdog que detecta falha de progresso

Planejar na unidade de usuário (não editar agora):

```ini
[Unit]
StartLimitIntervalSec=180s
StartLimitBurst=5

[Service]
Type=dbus
BusName=io.github.leoodaviid.Halo1
ExecStart=@BINDIR@/halod
ExecStopPost=@BINDIR@/halod --waterforce-cpu-safe
NotifyAccess=main
WatchdogSec=20s
WatchdogSignal=SIGKILL
TimeoutStopSec=20s
Restart=on-failure
RestartSec=2
```

Preservar PartOf/After=graphical-session.target e identidade udev do usuário.
O limite padrão (5 inícios em 10 s) deixa de valer com o helper: falha de
startup com X 240 presente custa o startup, até 18 s do helper (8 s de espera)
e RestartSec=2, mais de 10 s por ciclo, e a unidade nunca desistiria. Declarar
StartLimitIntervalSec=180s e StartLimitBurst=5: o sexto início de falhas de
startup seguidas cai dentro de ~5 × 20 s. Falhas tardias (watchdog de 20 s +
helper + pausa, ~40 s por ciclo) podem não atingir esse limite; cada ciclo
termina em E0 alto e na janela segura de 8 s, sem deixar E0 frio; o custo é
tráfego e log repetidos. Não mudar intervalo ou burst sem rever esse cálculo. WatchdogSignal=SIGKILL evita core dump/espera de SIGABRT em
processo SIGSTOP, fazendo o helper começar depois da morte do principal.
20 s é orçamento de supervisão escolhido, não timeout medido do firmware:
permite contenção de 5 s e operação legada com leituras limitadas, mas exige
prova de que a operação normal mais longa não gera falso positivo.

O PID principal chama a API sd_notify, respeitando WATCHDOG_USEC/WATCHDOG_PID,
com WATCHDOG=1 **somente após nova conclusão saudável** do ciclo: scan e
publicação de telemetria avançaram e, se o aparelho estava presente, um E0
atual ou seguro foi efetivamente aceito pelo transporte. Aparelho ausente
permite heartbeat após scan concluído; sem hardware não reiniciar em laço.
Aparelho presente com feed desabilitado pelo modelo (1/2 ou fora da tabela no
DE da sessão lógica, seção 1) tem a mesma regra da ausência: scan concluído
basta, sem E0, e os controles legados continuam disponíveis.
Com dispositivo presente, erro/worker bloqueado não renova saúde. Callback de
relógio, thread auxiliar independente ou resposta D-Bus isolada não bastam.
Uma conclusão não autoriza heartbeats repetidos enquanto não há novo progresso.
Notificar a cada conclusão (~2 s), sempre no máximo metade de WATCHDOG_USEC;
validar ambiente incompatível e recusar iniciar alimentação sem supervisão.

Sob unidade sem watchdog efetivo, ou execução manual fora dela, publicar CPU
por sysfs mas não iniciar E0 frio nem aceitar nova curva com degraus. O modo
standalone de proteção é a exceção, pois só envia alto. Testes injetam o
notificador/relógio e ambiente, sem exigir systemd real para a política.

SIGKILL/OOM/panic no principal → ExecStopPost → RestartSec=2 → novo daemon.
Travamento → cessam pings → em até 20 s desde último ping o gerenciador mata
principal → helper de até 18 s → restart. Atraso de DA acrescenta observação
física (até ~6 s medidos, não garantidos). Explicitar a janela de rotação baixa;
não prometer reação instantânea ou “nunca lento” em sentido absoluto.
Parada explícita/logout via PartOf não reiniciam; o helper ainda é tentado.
OOM global pode impedir até iniciar o helper: risco residual distinto de um
OOM apenas no principal. Não alterar Restart=on-failure para always.

Referências oficiais verificadas: [systemd.service](https://www.freedesktop.org/software/systemd/man/262/systemd.service.html)
(ExecStopPost, WatchdogSec, Restart, NotifyAccess) e
[sd_notify](https://freedesktop.org/software/systemd/man/latest/sd_notify.html)
(WATCHDOG=1). Type=dbus continua usando posse do nome como prontidão;
READY=1 não substitui esse contrato. A prova do watchdog real usa systemd do
usuário em testes de integração, não só um socket falso.

### 7. Suspensão, resume e riscos residuais aceitos

Em PrepareForSleep(true), descartar feeds frios e tentar E0=80 antes de dormir;
a entrega do sinal sem inibidor de atraso não garante conclusão antes do sono.
Durante suspensão não há alimentação; não interpretar tempo suspenso como
amostras que precisam ser reenviadas. Em resume e ao detectar salto no relógio
de boot, refazer descoberta/DE/E1 e proteção imediatamente, iniciar nova janela
segura de 8 s e então amostrar Tctl novamente, sem esperar o próximo tick.
Se logind não estiver disponível, registrar limitação e usar detecção de salto
do relógio; teste específico deve cobrir reinício pelo watchdog ao retomar.
Não alegar timeout/ação do watchdog durante o sono nem ordenação garantida
antes de a CPU voltar a executar. É risco residual aceito pelo usuário.

Alternativa descartada por decisão do usuário: protetor como serviço de
sistema/root. Acrescentaria cobertura para morte do gerenciador do usuário,
mas não para kernel travado; custa root, outra unidade e acesso hidraw fora
da regra udev da sessão. Não instalar, não exigir linger nem ampliar ACLs.
Também descartados: só SIGTERM (não pega SIGKILL), só restart (pode desistir),
watchdog com timer independente (mascara worker travado), e piso conservador
fixo (2200 não foi validado termicamente e sacrifica o objetivo de controle).

### 8. Propriedades D-Bus e curva mínima

Em `/io/github/leoodaviid/Halo1/waterforce`, interface existente
`io.github.leoodaviid.Halo1.Waterforce.Telemetry`, adicionar:

- `CpuMilliCelsius: i` (i32): Tctl bruto em m°C, 0 até a primeira leitura;
- `CpuValid: b`: false sem leitura única/válida; erro mantém último número;
- `CpuFeedActive: b`: true somente com supervisão e envio atual/seguro
  bem-sucedido há no máximo 4 s, depois de completada a janela inicial de
  8 s; false durante essa janela, em stop/erro/atraso. Não é sensor de RPM.

CPU pode ser válida mesmo com Waterforce ausente; Available e CoolantValid
preservam semântica anterior. O valor de proteção 80000 não substitui a
propriedade de sensor nem transforma CpuValid=false em true. PropertiesChanged
só para valores alterados. Não tocar gnome-panel-extras.

Adicionar `Speed.SetFanCurve(points: a(yq))`, exatamente quatro pares
(temperatura u8, RPM u16), temperaturas estritamente crescentes em 0..80,
RPM não decrescentes 900..2500 e último ponto exatamente 80:2500. Antes de
aceitar, exigir CpuValid e CpuFeedActive e supervisão da unidade; sem isso,
erro Device antes de abrir hidraw. Recusar também durante toda a janela
inicial/reinicialização segura, sem encurtá-la por pedido de cliente. Dentro da operação reler Tctl/estado de
supervisão e enviar E0 atual antes de aplicar a curva, para não usar a amostra
que ficou velha na fila. Validar conteúdo antes de procurar hardware.
Preservar snapshot e sequência existente E5 fan, E5 bomba, E6 fan, B6 uma vez;
E0 é transação de dados distinta sob o mesmo lock, não pertence ao snapshot.
Manter SetFlatCurve e CLI de RPM único; não adicionar editor de pontos na CLI.
Restaurar snapshots históricos não muda sua validação nem os arquivos.

## Risks / Trade-offs

- [Gerenciador do usuário morre, kernel trava, USB falha ou ACL some no logout]
  → as camadas podem falhar juntas; registrar risco, não prometer proteção.
- [E0 alto não produz máximo em qualquer configuração] → novos degraus exigem
  80:2500; documentar curvas legadas, presets não medidos e modos externos.
- [Janela de até watchdog + helper + reação do aparelho] → ensaiar tempos no DA,
  manter recuperação limitada e tornar o risco explícito antes do aceite.
- [Flash interna/E1 obrigatório não provados] → enviar como GCC sem B6;
  manter incerteza documentada, sem chamar leitura após corte de prova de flash.
- [Feed muda RPM também em presets] → enviar proteção em qualquer modo, nunca
  restaurar frio; avisar no contrato de serviço e testar em Performance.
- [Curvas baixas/temperatura de terceiros] → flock coordena processos halo,
  não kernel ou outro software; não prometer exclusão global do USB.

## Migration Plan

1. Revisar artefatos e resolver achados de `/cross-review`; não implementar
   antes desse gate. Validar OpenSpec. Leitura pós-corte pode ficar como
   evidência pendente; não bloqueia as decisões, pois não dependem de 50 °C.
2. Implementar e provar cada camada com backend falso e unidade em ambiente
   de usuário isolado; atualizar documentação junto das mudanças.
3. Instalar em usuário somente após autorização de implementação e gates
   automatizados, mantendo backup do binário e unidade como par compatível.
4. No aparelho, aval específico por execução: SIGTERM, SIGKILL, SIGSTOP do
   principal e bloqueio do worker, com journal + DA provando recuperação após o E0 do helper e antes
   do primeiro E0 de qualquer temperatura do processo reiniciado; depois carga/repouso com Tctl.
5. Para rollback, enviar proteção e parar a unidade nova enquanto ainda tem
   helper compatível, observar DA e só depois restaurar o par antigo. Não
   restaurar último E0 frio; snapshots/perfis continuam preservados.

A proposta só pode ser chamada revisada depois da revisão externa; a
implementação só pode ser aceita com as provas de morte/travamento no DA.
Nenhum desses testes futuros foi executado nesta fase sem código.
