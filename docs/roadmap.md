# Roadmap

## Fase 0: análise (concluída em 2026-10-20)

Inventário de hardware, protocolos e permissões. Ver [hardware.md](hardware.md).

## Fase 1: placa-mãe (IT5702)

1. ~~Criar o workspace Rust e decidir a licença.~~ Feito em 2026-10-20: workspace `crates/halo-core` + `crates/halo-cli`, GPL-2.0-or-later.
2. ~~Ferramenta só de leitura (`CC 60`).~~ Feito em 2026-10-20: `halo probe it5702 --raw`. Valores lidos em [hardware.md](hardware.md).
3. Cor estática por zona (`D_LED1`, `D_LED2`, `LED_C1`, `LED_CPU`, `LED_C2`) e ligar/desligar.
4. Efeitos nativos do controlador.

## Fase 2: daemon e interface

1. Daemon com D-Bus e perfis salvos.
2. Protótipo visual do painel para aprovação antes de codar a extensão.
3. Extensão GNOME Shell com o ícone na barra e o painel.
4. Serviço `systemd --user` para reaplicar o perfil no login.

## Fase 3: Waterforce

1. ~~Regra udev para liberar `1044:7a4d` ao usuário.~~ Feito em 2026-10-20: `packaging/udev/70-halo.rules`.
2. ~~Telemetria e leitura de firmware.~~ Feito em 2026-10-20: `halo telemetry waterforce` e `halo probe waterforce`. O firmware não confirmou a compatibilidade com o protocolo de anel do OpenRGB (ver [hardware.md](hardware.md)).
3. Analisar o GIGABYTE Control Center com o REA: protocolo de cor do anel e das fans, controle de velocidade da fan e da bomba, e envio de imagens para a tela LCD.
4. Cor e efeitos do anel e das fans, com o protocolo confirmado no passo 3.
5. Controle de velocidade e tela LCD, se o passo 3 mostrar como.

## Fase 4: periféricos

1. Mouse G502 SE HERO.
2. Teclado Redragon (só efeitos e brilho do LED vermelho). Alto risco: só depois de analisar o software oficial da Redragon com o REA. Ver `hardware.md`.

## Fase 5: RAM (decisão pendente)

Só se decidirmos liberar o SMBus com `acpi_enforce_resources=lax`. Kingston Fury DDR4, endereço `0x58`.

## Perguntas em aberto

- A placa não tem nenhuma fita RGB conectada. Como validar visualmente os comandos de cor da Fase 1 (itens 3 e 4)?
- O Waterforce é um X II 240 ou um Waterforce X 240 da primeira geração? O PID e a resposta ao `DE` apontam para a primeira.

- Quais efeitos do RGB Fusion 2.0 são indispensáveis?
