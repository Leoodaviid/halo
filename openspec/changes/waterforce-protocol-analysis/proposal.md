# Proposal

## Why

O probe do Waterforce (`waterforce-read-probe`) não confirmou que o protocolo de anel do OpenRGB vale para este aparelho. O `DE 00` respondeu versão 0 em vez de 2, e o PID `1044:7a4d` é o que o kernel registra para o Waterforce X da primeira geração. Também não há fonte documentada para o RGB das fans, para o controle de velocidade da fan e da bomba nem para a tela LCD. Pelas regras do projeto, nenhum desses comandos pode ser enviado antes de ser confirmado no software oficial. Agora temos os dois instaladores da Gigabyte para analisar com o REA.

## What Changes

- Extração dos instaladores NSIS do GIGABYTE Control Center 26.09.10.01 e do RGB Fusion B26.0923.1 em `tools/samples/*/extracted/`, sem executar nada.
- Análise estática com o REA para descobrir:
  - qual dos dois programas reconhece o `1044:7a4d`, e com que nome de modelo;
  - os comandos de cor e efeito do anel e das fans, comparados com o protocolo do OpenRGB (`c9`, `cd`, `b0`–`b3`, `b6`);
  - os comandos de controle de velocidade da fan e da bomba;
  - o formato e a sequência de envio de imagens para a tela LCD.
- Documentação do que for confirmado em `docs/protocols/castor3.md` e um resumo em `docs/hardware.md`, com a fonte de cada byte (arquivo, tipo e método).
- Lista das próximas changes de implementação, cada uma limitada ao que a análise confirmou.

Nenhum comando é enviado ao hardware nesta change.

## Capabilities

### New Capabilities

Nenhuma. A change só produz documentação de protocolo, sem comportamento novo no halo (`skip_specs: true`).

### Modified Capabilities

Nenhuma.

## Impact

- Sistema: precisa do 7-Zip para abrir os instaladores NSIS (`sudo apt install 7zip`, feito pelo usuário). Se a lógica estiver em código nativo, também precisa do JDK 21+ e do Ghidra 12.1.x para o REA.
- Disco: os instaladores extraídos ocupam alguns GB em `tools/samples/`, fora do repositório.
- Repositório: só documentação nova (`docs/protocols/castor3.md`) e atualizações em `docs/hardware.md` e `docs/roadmap.md`.
