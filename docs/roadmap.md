# Roadmap

## Fase 0: análise (concluída em 2026-10-20)

Inventário de hardware, protocolos e permissões. Ver [hardware.md](hardware.md).

## Fase 1: placa-mãe (IT5702)

1. Criar o workspace Rust (`cargo new`) e decidir a licença (GPL-2.0-or-later se portar código do OpenRGB).
2. Ferramenta só de leitura: enviar `CC 60`, ler o feature report e mostrar produto, firmware, contagem de LEDs por fita e chip id.
3. Cor estática por zona (`D_LED1`, `D_LED2`, `LED_C1`, `LED_CPU`, `LED_C2`) e ligar/desligar.
4. Efeitos nativos do controlador.

## Fase 2: daemon e interface

1. Daemon com D-Bus e perfis salvos.
2. Protótipo visual do painel para aprovação antes de codar a extensão.
3. Extensão GNOME Shell com o ícone na barra e o painel.
4. Serviço `systemd --user` para reaplicar o perfil no login.

## Fase 3: Waterforce X II

1. Regra udev para liberar `1044:7a4d` ao usuário.
2. Anel RGB com o protocolo do driver Castor3, primeiro só leitura (firmware).
3. Tela LCD: analisar o GIGABYTE Control Center com o REA para descobrir como as imagens são enviadas.

## Fase 4: periféricos

1. Mouse G502 SE HERO.
2. Teclado Redragon (só efeitos e brilho do LED vermelho). Alto risco: só depois de analisar o software oficial da Redragon com o REA. Ver `hardware.md`.

## Fase 5: RAM (decisão pendente)

Só se decidirmos liberar o SMBus com `acpi_enforce_resources=lax`. Kingston Fury DDR4, endereço `0x58`.

## Perguntas em aberto

- Quais efeitos do RGB Fusion 2.0 são indispensáveis?
