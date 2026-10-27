# waterforce-cpu-feed

Fase 3.4: enviar o nome (E1) e os dados da CPU (E0) ao Waterforce, como o
AorusLcdService, para a curva Customized acompanhar Tctl e a carga da CPU.
O envio contínuo depende do halo-daemon e da proteção na unidade de usuário.
LCD, painel e serviço de sistema/root ficam fora do escopo desta change.

Artefatos: [proposal.md](proposal.md), [design.md](design.md), [specs](specs/)
e [tasks.md](tasks.md). Medições: [experimentos.md](experimentos.md),
[experiment1.jsonl](experiment1.jsonl) e [experiment2.jsonl](experiment2.jsonl).
Parecer da revisão independente e seus limites: [review.md](review.md).
Validação estrutural não equivale a aceite da implementação: faltam as provas
futuras de morte/travamento e carga no DA. A leitura após corte de energia
continua pendente de aval e não tem resultado inventado.
