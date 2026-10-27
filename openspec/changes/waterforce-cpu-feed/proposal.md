# Proposal

## Why

No Linux, o Waterforce X não recebe Tctl e a curva Customized não acompanha
a carga. Os ensaios confirmaram que E0 controla essa referência e que um
último E0 frio fica retido por pelo menos 70,62 s: o envio exige proteção.

## What Changes

- Alimentar o Waterforce com E1 e E0 de CPU, usando Tctl; expor temperatura
  e validade pelo D-Bus, sem alterar painel ou LCD.
- Enviar a cada 2 s, com amostra atual e transações curtas sob flock.
- Proteger no SIGTERM e por `halod --waterforce-cpu-safe` em ExecStopPost;
  usar watchdog associado ao avanço do fluxo e Restart=on-failure.
- Manter tudo na unidade de usuário. Morte do gerenciador, kernel travado,
  indisponibilidade de USB/permissão e suspensão têm limites explícitos.
- Permitir curva de quatro pontos da fan pelo D-Bus, com último ponto
  80 °C:2500 RPM e supervisão ativa; preservar a API plana e a CLI atual.
- Exigir testes falsos e provas de SIGKILL/travamento no DA antes de aceitar
  a implementação. Submeter os artefatos a `/cross-review` antes de implementar.

## Capabilities

### New Capabilities

Nenhuma.

### Modified Capabilities

- `daemon-waterforce`: E1/E0 periódicos, recuperação segura, propriedades CPU
  e curva de quatro pontos da fan pelo D-Bus.
- `daemon-service`: exceção para alimentação da CPU na inicialização,
  ticks, encerramento e resume; serialização e amostras sem backlog.
- `device-exclusive-access`: transações curtas do feed e espera limitada,
  preservando o flock da CLI e a independência das leituras de sysfs.
- `waterforce-speed`: manter curva plana na CLI, permitir a nova curva da
  fan pelo daemon com supervisão; preservar snapshots e restauração.
- `user-service`: SIGTERM, ExecStopPost, watchdog e reinício na mesma unidade
  de usuário, sem serviço de sistema ou instalação com root.

## Impact

Na implementação futura: halo-core (aquisição CPU e payloads), halo-daemon
(worker/ticks/bus, sinais, modo independente de proteção), unidade e instalador
do usuário. APIs antigas continuam funcionando; o daemon passa a enviar
E1/E0 espontaneamente e a rotação pode mudar também em presets.

**Somente planejamento nesta etapa.** Design, specs e tasks documentam as
medições em `experiment1.jsonl`, `experiment2.jsonl` e `docs/hardware.md`.
A existência dos arquivos ou validação OpenSpec não substitui revisão externa
nem prova no aparelho. A leitura inicial após corte de energia permanece
pendente de aval; E1 obrigatório e ausência de flash não são fatos provados.

Fora do escopo: serviço de sistema/root, LCD (modo, mídia e cores), painel,
gnome-panel-extras, RAM/SMBus e edição geral de curvas na CLI. A dependência
systemd usa a unidade já instalada e as permissões udev da sessão.
