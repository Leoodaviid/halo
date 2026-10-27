# Pré-design: referência de temperatura e interrupção do E0

Estado: **ensaios 1 e 2 autorizados e executados; design/specs/tasks redigidos**.
Os protocolos e conclusões abaixo preservam o histórico. A decisão posterior
do usuário sobre proteção está registrada no fim deste documento e prevalece
sobre as pendências arquiteturais antigas. Nenhum ensaio novo foi executado.
Este documento não é o design e não libera implementação. Primeiro validar
a hipótese E0 → referência da curva. Se ela cair, interromper esta change.

## Base observada

Leitura autorizada em 2026-10-25: X 240, fan Performance (5), bomba Turbo (4),
slot Customized da fan 0:1500/30:1500/50:1500/80:1500, slot da bomba
0:2600/30:2600/50:2600/80:2600. CPU AMD Ryzen 5 5600G, 12 threads, 6 núcleos;
sensor `k10temp` com label Tctl. Conferir novamente no início do ensaio;
se o estado mudou, não aplicar esta sequência sem revisá-la.

## Ensaio 1 — proposta para aval específico

Sem stress e sem alteração da bomba. Uma execução, cerca de dois minutos,
com log bruto de DA e Tctl, relógio monotônico para medir atrasos e horário
de cada envio. Descobrir hidraw por VID:PID 1044:7a4d, usage page 0 e usage 2.
Tomar o mesmo flock exclusivo no nó usado pelo halo durante a transação;
CLI e operações de escrita do halod podem retornar Busy durante o ensaio.
Não alterar serviços ou seus arquivos. Sem B6 em toda a execução.

Todos os payloads abaixo são completados com zeros até 6144 bytes.
E5/E6: `docs/protocols/castor3.md`, `DeviceApi.SetSpeedMode/SetSpeedCurve`
do ucCooler; E1/E0: `CoolerDataApi.SendCpuName/SendCoolerData` do
AorusLcdService; transporte: `CoolerApi.Send`.

1. Confirmar modelo, modos e slots com `99 DE 00`, `99 DD`, `99 D9 01`,
   `99 D9 02`, guardando respostas brutas. Observar Tctl e DA por 10 s.
   Não iniciar a fase de escrita se Tctl não estiver disponível ou estiver
   em 60 °C ou mais. Esse limite é uma condição conservadora deste ensaio,
   não um limite térmico declarado da CPU.
2. Enviar nome `Ryzen 5 5600G`:
   `99 E1 0D 52 79 7A 65 6E 20 35 20 35 36 30 30 47`.
   Enviar E0 de 80 °C: `99 E0 00 50 0C 00 00 06 00 00 00 00 00`.
   Threads/núcleos correspondem à CPU observada; GHz, VRAM/líquido, uso e W
   são zeros controlados. Isso testa o conjunto desses zeros; não prova
   isoladamente a inocuidade de cada campo.
3. Selecionar fan Customized: `99 E5 01 01 00`. Não enviar E5 da bomba.
   Gravar a curva 0:2200/30:2200/50:2350/80:2500:
   o piso fica próximo ao RPM observado em Performance, evitando usar
   1500 RPM nesta primeira investigação de comportamento desconhecido:
   `99 E6 00 01 00 08 98 1E 08 98 32 09 2E 50 09 C4`.
4. Enviar um E0 por patamar, nas temperaturas 80, 50, 30 e 80 °C,
   observando DA por 12 s após cada envio. Os pacotes são os do passo 2,
   substituindo apenas o byte 3 por `50`, `32`, `1E` e `50`, respectivamente.
   Registrar duty da fan (`DA[8]`) e RPM (`DA[2..4]`, little-endian),
   além de Tctl. Observar aproximadamente 2500, 2350 e 2200 RPM seria
   consistente com a hipótese; priorizar a mudança de duty porque o RPM
   tem inércia e oscilação. O período do DA limita a precisão temporal.
5. Depois do último E0 alto, passar 60 s sem E0, mantendo registro de DA
   e Tctl. Isso mede apenas a janela observada: não prova ausência de
   timeout posterior nem mede retenção após um último E0 frio.
6. Selecionar fan Turbo com `99 E5 01 04 00`, depois restaurar o slot
   plano original, agora inativo:
   `99 E6 00 01 00 05 DC 1E 05 DC 32 05 DC 50 05 DC`;
   em seguida `99 E5 01 05 00` para Performance. Conferir DD e ambos os D9
   e observar duty/RPM por 10 s. Não restaurar snapshots históricos da CLI:
   eles não necessariamente correspondem ao estado inicial deste ensaio.

Abortar a sequência de patamares se Tctl atingir 60 °C, desaparecer,
se o DA parar de chegar por 6 s ou se houver falha de transporte. Tentar
imediatamente E0 de 80 °C e fan Turbo (`99 E5 01 04 00`); só prosseguir
com restauração quando houver observabilidade e transporte funcionando.
Em falha de restauração, reportar o estado incerto e não declarar sucesso.
Uma morte abrupta do processo não é coberta por cleanup em software; o
usuário deve acompanhar o ensaio e poder interromper a carga da máquina.

E0 não tem operação de desfazer documentada: o último valor enviado ficará
em 80 °C, e não no estado interno desconhecido anterior. Modos e slots
serão conferidos, mas persistência após corte de energia não será testada.
Sem B6 não significa garantia de ausência de escrita interna em flash.

## Ensaios posteriores, ainda sem autorização

- Necessidade de E1: o ensaio 1 usa E1; não responde se ele é obrigatório.
  Exige comparação com estado inicial conhecido, evitando atribuir ao teste
  sem E1 um nome já retido. Definir o procedimento depois do primeiro resultado.
- Retenção após temperatura baixa, interrupções maiores, logout, suspensão
  e morte do processo: definir ensaio separado com proteção independente.
  Não extrapolar o ensaio após 80 °C para segurança após 30 °C.
- Cadência: medir reação e comportamento entre envios; comparar períodos
  candidatos sem copiar um Sleep de outro modelo nem o laço do Windows.
- Persistência e byte do líquido: separar ausência de B6 observada no host
  de conclusões sobre firmware; registrar grau de evidência e limitações.
- Carga real: somente depois de proteção definida, registrar Tctl e DA
  durante subida e descida de carga. `stress-ng` ainda não foi instalado:
  sudo exigiu autenticação em terminal. Não iniciar carga nesta etapa.

Após as medições, escrever proposal/design/specs/tasks coerentes, decidir
fontes dos campos, propriedade D-Bus e convivência com flock; validar com
OpenSpec e submeter os artefatos a `/cross-review` antes de implementar.

## Resultado do ensaio 1 (2026-10-25)

Executado com aval específico do usuário, de 21:42:57 a 21:45:06 UTC.
Log completo: [`experiment1.jsonl`](experiment1.jsonl). O executor diagnóstico
temporário foi `/tmp/waterforce-cpu-feed-experiment1.py`; não houve alteração
de código do produto. O log inclui cada payload transmitido e DA bruto.

| E0 enviado | Duty bruto DA observado após reação | Primeiro DA no duty esperado após envio | RPM no fim do patamar de 12 s |
|---|---|---|---|
| 80 °C | 100 | 3,780 s | 2574 |
| 50 °C | 94 | 1,819 s | 2706 |
| 30 °C | 88 | 5,853 s | 2243 |
| 80 °C | 100 | 3,835 s | 2574 |

O duty bruto do DA é compatível com porcentagem (100, 94 e 88), e não com
a escala 0–255 do pwm do hwmon. Não comparar esses números diretamente.
RPM tem inércia e oscila: o patamar de 50 °C não mostrou RPM menor dentro
dos 12 s, mas o duty mudou exatamente para 2350/2500 × 100 = 94.
O Tctl real ficou aproximadamente entre 37,6 e 40,5 °C durante a execução;
portanto a mudança de duty não acompanhou a temperatura real do host.

**Hipótese 1 confirmada neste firmware com E1 prévio e os campos auxiliares
do ensaio:** E0 controla a referência da curva Customized. A mudança de
duty é inequívoca; o ensaio não demonstra uma política de amostragem ideal.
Os atrasos acima incluem a cadência do DA (~2 s), cache e escalonamento:
não são medidas exatas da latência interna do firmware.

Após o último E0 de 80 °C (t=46,457 s), o último DA antes da restauração
(t=116,9344 s) ainda mostrou duty 100. São **70,4774 s sem novo E0**
observados, sem fallback visível para 50 °C. Isso é compatível com retenção,
mas não prova retenção indefinida nem comportamento igual após valor frio.

Restauração conferida por DE/DD/D9: fan Performance (5), bomba Turbo (4),
slot fan 1500 ×4 e slot bomba 2600 ×4, como no início. A fan permaneceu
em duty 100 nos 10 s finais, compatível com o último E0 alto afetando também
o preset; restaurar modo/slot não desfaz o E0. Nenhum B6 foi enviado.
Não houve abort, carga de CPU ou acionamento do processo de guarda.
E1 obrigatório, persistência em flash, cadência e proteção após morte do
daemon continuam sem comprovação suficiente para fechar o design.

## Ensaio 2 — interrupção após E0 frio, para novo aval

Protocolo autorizado e executado, mantido abaixo para reprodução. Reutilizar
descoberta, flock, baseline de 10 s, guardião,
monitoramento, limite de Tctl de 60 °C, timeout de DA de 6 s e restauração
do ensaio 1. Duração prevista de cerca de dois minutos; sem carga e sem B6.
Conferir novamente modos/slots iniciais com DE/DD/D9; abortar antes de
escritas se divergirem do estado registrado. Nenhum comando de bomba.

1. Enviar E0 de 80 °C (`99 E0 00 50 0C 00 00 06 00 00 00 00 00`),
   E5 Customized (`99 E5 01 01 00`) e a mesma curva conservadora
   (`99 E6 00 01 00 08 98 1E 08 98 32 09 2E 50 09 C4`).
   Observar por 12 s. Não reenviar E1: ele já foi enviado nesta sessão;
   isso não constitui teste da sua necessidade após inicialização limpa.
2. Enviar **um** E0 de 30 °C (`99 E0 00 1E 0C 00 00 06 00 00 00 00 00`)
   e observar por 72 s sem novo E0. Pergunta: permanece em duty 88,
   volta a 94 (50 °C), sobe para 100 ou faz outra coisa? O piso da curva
   continua em 2200 RPM, aproximadamente o Performance anterior ao E0.
3. Enviar E0 de 80 °C e observar por 12 s; depois selecionar Turbo,
   restaurar slot plano da fan e Performance com os mesmos payloads do
   ensaio 1. Conferir modos e slots e observar por 10 s.

O guardião independente tenta E0 alto e Turbo ao detectar morte do executor
ou prazo excedido; não é garantia se o transporte falhar e não é uma decisão
de arquitetura do halod. Não testar morte abrupta, logout ou suspensão neste
ensaio. Como antes, o último E0 permanece alto intencionalmente.

## Resultado do ensaio 2 (2026-10-25)

Executado com novo aval específico, de 21:46:41 a 21:48:38 UTC.
Log: [`experiment2.jsonl`](experiment2.jsonl); executor diagnóstico temporário
`/tmp/waterforce-cpu-feed-experiment2.py`. Não foi alterado código do produto.

- E0 de 30 °C enviado em t=22,2199 s; primeiro duty 88 em t=26,2804 s
  (4,0605 s depois). Todos os DA seguintes até o E0 alto mostraram duty 88.
- Último DA antes do E0 alto: t=92,8373 s, **70,6174 s após o E0 frio**,
  duty 88, fan 2297 RPM, Tctl 40,75 °C. Não houve fallback observado para
  o duty de 50 °C nem para o máximo nessa janela. A espera entre envios
  durou 72,2202 s; o DA não amostra continuamente.
- E0 final de 80 °C em t=94,4401 s; duty 100 observado em t=96,8773 s
  (2,4372 s depois), fan 2550 RPM. Tctl no ensaio inteiro: 40,125–43 °C.
- DE/DD/D9 finais conferiram Performance/Turbo e slots 1500 ×4/2600 ×4.
  Nenhum E1 reenviado, nenhum B6, nenhuma carga, nenhum abort; guardião
  desarmado normalmente. O último E0 permaneceu em 80 °C intencionalmente.

**Implicação para o design:** não há timeout curto de segurança demonstrado.
Uma curva com piso baixo pode continuar nesse piso depois da morte do
alimentador. Cleanup no halod, um último E0 alto no desligamento normal,
reinício eventual pelo systemd ou um watchdog na mesma sessão não bastam
para cobrir, por si só, SIGKILL, logout e falha da sessão. Não alegar proteção
contra esses casos sem uma arquitetura independente e verificações próprias.
O piso conservador do ensaio não foi validado como proteção térmica permanente.

## Pendências levantadas antes do design (histórico)

1. Definir o limite arquitetural da proteção. O serviço existente é de
   usuário, sem root e ligado à sessão; uma proteção que sobreviva ao logout
   muda instalação e ciclo de vida. Esse conflito precisa ser resolvido
   explicitamente, incluindo falha do protetor e contenção de flock.
2. E1 obrigatório: a segunda execução funciona sem reenviá-lo, mas herda o
   E1 da primeira. Para discriminar a necessidade após inicialização,
   preparar ensaio a partir de corte de energia confirmado, com observação
   antes de E1 e comparação posterior. Não há autorização para corte de
   energia ou para esse próximo ensaio.
3. E0 e flash: o laço oficial não inclui B6, mas isso não é prova do que
   o firmware grava internamente. Uma leitura de estado após corte de energia
   pode limitar hipóteses de persistência, sem provar ausência de desgaste.
   Não prometer prova que o material disponível não permite obter.
4. Após resolver essas questões, medir cadências candidatas e validar
   reação à carga real sob proteção definida. Não usar o piso de 2200 RPM
   deste ensaio como argumento para aceitar curvas arbitrárias com piso baixo.

## Decisão posterior e evidência ainda pendente

O usuário escolheu não incluir serviço de sistema/root nesta change. A
proteção planejada está em `design.md`: SIGTERM, comando independente de
ExecStopPost, watchdog ligado ao progresso e Restart=on-failure, todos na
mesma unidade de usuário. Os riscos residuais de morte do gerenciador,
travamento do kernel, indisponibilidade de hardware/ACL e suspensão foram
aceitos como limites; isso não transforma esses casos em protegidos.

E0 de proteção escolhido: 80 °C, pela tabela medida, enviado em qualquer
modo. Isso não garante duty 100 em uma curva plana baixa ou em modo não
ensaiado. Os novos degraus pelo D-Bus terminam em 80:2500; a CLI continua
com RPM único. 2 s é a cadência proposta, sujeita às provas definidas em tasks.

Permanece pendente, com aval específico, a leitura do DA após corte real de
energia e antes de qualquer E0 do halod. Fazer preferencialmente antes de
instalar a implementação nova; ela não pode estar ativa durante a coleta.
Registrar o procedimento de prevenção de ativação automática e seus limites
antes de executar. O observador só abre hidraw para leitura; a leitura de
hwmon provoca DA pelo kernel. Sem E0/E1, sem alterar curva, modo ou mídia
durante essa coleta. Não solicitar corte de energia automaticamente.

Nenhuma conclusão sobre origem Windows, padrão de ~50 °C, obrigatoriedade de
E1 ou ausência de flash pode ser extraída dos logs existentes. O design não
depende de confirmar ~50 °C: ele usa a retenção medida e envia E1 como o GCC.
Provas de SIGKILL/travamento, watchdog e carga/repouso ainda são tarefas
futuras após autorização de implementação, com aval próprio por execução.
