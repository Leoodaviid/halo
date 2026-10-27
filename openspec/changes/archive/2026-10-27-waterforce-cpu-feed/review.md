# Revisão dos artefatos

Data: 2026-10-25. Escopo: proposal, design, cinco delta specs, tasks e
coerência com documentação, medições, specs principais e unidade existente.

## Método e limite

O comando/skill literal `/cross-review` não foi encontrado no catálogo,
ferramentas, arquivos de configuração locais nem PATH consultados. Não foi
executado e não é declarado como executado. A revisão disponível foi feita
por agente independente, `/root/artifact_cross_review`, somente leitura,
em duas rodadas. A execução literal solicitada continua pendente por
indisponibilidade do mecanismo; este parecer deixa explícito o método usado.

Parecer final do revisor: **planejamento aprovado após correções, sem
bloqueadores restantes nos artefatos**. Isso não aceita implementação nem
substitui os logs físicos previstos em tasks.md.

## Achados e correções conferidas

| Severidade | Achado inicial | Correção |
|---|---|---|
| P1 | DA seguro antes do primeiro E0 frio do restart poderia ser causado pelo E0 alto inicial do processo novo, mascarando helper quebrado | Exigir DA seguro após E0 do helper e antes do primeiro E0 de qualquer temperatura do principal novo; sem amostra isolada, teste não satisfaz aceite |
| P2 | SetFanCurve poderia enviar frio durante a janela inicial alta | CpuFeedActive=false durante os 8 s e recusa do método até concluir a janela, com cenário e teste de cliente no restart |
| P2 | “Conexão” poderia exigir E1/DE a cada fd aberto por tick | Definir sessão lógica por startup/reconnect/geração/resume, separada de fechar fd e soltar flock |
| P2 | Histórico de investigação deixava arquitetura/root em aberto depois da decisão | Atualizar estado e registrar a decisão posterior em experimentos.md, preservando as evidências e pendências físicas |

O revisor conferiu as quatro correções e confirmou validação estrita e
checagem de whitespace. As correções não alteraram código ou logs medidos.

## Verificação desta fase

- `openspec validate waterforce-cpu-feed --strict`: aprovado, sem achados.
- `git diff --check`: aprovado.
- Proposal/design/specs/tasks presentes; status OpenSpec mede existência dos
  artefatos, não a execução das tasks ou validação das camadas no aparelho.
- Nenhum cargo test de implementação nova foi executado: só artefatos foram
  escritos. Os nomes e comandos de testes em tasks.md são trabalho futuro.

## Segunda rodada: `/cross-review` (2026-10-27)

Com o skill disponível, `/cross-review` foi executado nos artefatos (proposal,
design, tasks, review, experimentos, README, cinco delta specs e o diff de
`docs/hardware.md`). Revisor: Codex CLI, modelo `gpt-6.1-sol` (outro
fornecedor), sandbox somente leitura e sessão efêmera. Autor dos artefatos:
Claude (família Opus). Nenhum arquivo excluído por segredo. Cada achado foi
conferido contra a unidade e os artefatos antes de corrigir.

| Severidade | Achado | Verificação | Correção |
|---|---|---|---|
| Média | O limite padrão de inícios (5 em 10 s; conferido em `systemctl --user show`) não é atingido quando ExecStopPost espera 8 s com o X 240 presente: falha de startup por nome tomado vira laço infinito de E0, contra o requisito de desistir | Confirmado: a unidade atual não declara StartLimit*, e cada ciclo passa de 10 s | StartLimitIntervalSec=180s e StartLimitBurst=5 em [Unit] (design §6, user-service, tarefa 5.2 com teste de desistência); falhas tardias de ~40 s registradas como limite aceito, sempre terminando em E0 alto |
| Média | O watchdog exigia E0 com qualquer aparelho presente, mas o feed não é habilitado em modelos 1/2: um X 280/360 seria morto a cada 20 s, derrubando D-Bus e controles legados | Confirmado: design §1 desabilita modelos 1/2 e §6 não tinha estado saudável para esse caso | Modelo 1/2/fora da tabela no DE: sem E1/E0 na sessão lógica, CpuFeedActive=false, SetFanCurve recusado, heartbeat só pelo scan (design §1/§6, user-service, daemon-waterforce, tarefa 5.1) |

Consequência da segunda correção, encontrada ao reconciliar: o helper saía
com erro em “modelo incorreto”, o que deixaria a unidade failed em toda parada
normal com X 280/360. Corrigido com aval do usuário: modelo 1/2 sai com 0 sem
E1/E0 nem espera; modelo fora da tabela continua falha (design §5,
daemon-waterforce, tarefa 4.2).

Nenhum achado descartado. A revisão gerou correções só em artefatos; nenhum
código ou log medido mudou.

## Gates restantes

- Leitura do DA após corte de energia, com aval específico e antes de E0;
  nenhuma conclusão nova sobre padrão de ~50 °C ou origem Windows.
- Implementação somente após nova solicitação do usuário; depois, testes
  falsos, integração da unidade e provas físicas de SIGKILL, travamento e
  carga/repouso com novo aval por execução.
- Não declarar implementação aceita por restart bem-sucedido, write aceito
  ou por este parecer: é necessário demonstrar a recuperação isolada no DA.

## Verificação integrada (tarefa 7.1, 2026-10-27)

Depois dos ensaios 6.1 a 6.5 e da última correção (sem `E0` colado depois da
recuperação), com o par instalado às 11:06:46:

- `cargo test --workspace`: 296 testes, 0 falhas (CLI 16 + 5 + 7 + 10;
  halo-core 155; halod 89 + 14 de processo).
- `cargo clippy --workspace --all-targets`: sem avisos; `cargo fmt --check`
  em dia.
- `sh packaging/tests/install-test.sh`: todas as verificações passaram.
- `sh packaging/tests/systemd-harness.sh` (gerenciador do usuário, halod
  falso): 26 verificações passaram; nenhuma unidade de teste sobrou.
- `openspec validate waterforce-cpu-feed --strict` e `git diff --check`:
  aprovados.

Os status acima não substituem os ensaios no aparelho, registrados em
`docs/hardware.md` (6.1 a 6.5, com a lacuna do 6.3b).

## Aceite da implementação (tarefa 7.2, 2026-10-27)

Revisão externa: a rodada do `/cross-review` (Codex) sobre os artefatos não
deixou bloqueadores; os três achados foram corrigidos com aval. O código da
implementação **não** passou por revisão externa; recomendável antes do PR.

| Decisão do design | Evidência |
|---|---|
| E0 como referência; Tctl do host | 6.1 e 6.5: duty seguiu o Tctl nos pontos da curva (89/94/97/90–91) |
| 80 °C como proteção | 6.1–6.4: duty 100 em todo E0 de 80 °C; restaurar modo não desfaz |
| Período de 2 s, slot único, sem backlog | 6.5: 99 E0, 2,012–2,037 s; 6.4: um envio após lock de 3 s, sem replay |
| Janela segura de 8 s | todo início/restart: 4 E0 de 80 °C antes do Tctl (journal) |
| Flock curto, espera de 5 s, recuperação | 6.4: CLI sem intercalação; Busy aos 5,0 s; E0 de 80 °C na liberação |
| SIGTERM protege | 6.1 e reinstalações: E0 de 80 °C ~45 ms após o sinal |
| ExecStopPost isolado | 6.2 (SIGKILL): 3 DA em 100 na janela do helper; 6.3a (SIGSTOP): 4 DA |
| Watchdog de progresso com SIGKILL | 6.3a: timeout aos ~20 s, SIGKILL, helper; harness: sem pings → watchdog |
| StartLimit 180 s/5 | harness: 5 tentativas com helper de 8 s e desistência |
| Unidade de usuário, sem root | instalação e todos os ensaios no gerenciador do usuário |

Limitações que continuam (sem confirmação inventada):

- **E1 obrigatório:** não comprovado; o halo envia por compatibilidade.
- **Flash:** sem `B6` no feed; o corte de energia mostrou que o último E0
  (80 °C) não sobreviveu, mas isso não prova ausência de escrita interna.
- **Referência padrão após o corte:** duty 88 em Performance, compatível
  com ~50 °C; o DA não lê a referência, segue hipótese. A leitura veio depois
  dos ensaios, fora da ordem planejada.
- **6.3b:** bloqueio só do worker não ensaiado no aparelho (decisão do
  usuário); coberto por teste automático e pelo harness.
- **Janela de rotação baixa:** até ~22 s medidos com o principal travado
  (6.3a); morte do gerenciador, kernel travado, OOM global, USB e perda de
  ACL seguem sem proteção garantida.
- **Curva com degraus:** só ensaiada a 0:2200/30:2200/50:2350/80:2500;
  presets e curvas antigas não têm garantia de 100 % com E0 de 80 °C.

Parecer: implementação aceita dentro desses limites. Arquivar e commitar
dependem de pedido do usuário.
