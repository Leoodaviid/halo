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
3. ~~Analisar o GIGABYTE Control Center com o REA.~~ Feito em 2026-10-20: velocidade, status e LCD no C#, e anel e fans na `GcLedLib.dll` com o Ghidra. Ver [protocols/castor3.md](protocols/castor3.md).
4. Velocidade, dividida em duas changes:
   - ~~`waterforce-speed-mode`~~: feito em 2026-10-20. `halo speed waterforce status|set|restore`, só com os presets do firmware (`E5` + `B6`) e com snapshots anterior e original. Confirmado no aparelho: o `E5` vale sem o `B6`, mas só até desligar, o `B6` persiste o modo, e o `D9` é o slot Customized.
   - ~~`waterforce-speed-curve`~~: feito em 2026-10-21. `halo speed waterforce curve <fan|pump> <rpm>`, uma curva Customized plana (`E6`). Confirmado no aparelho: o `E6` vale sem o `B6`, e a referência da curva não é o líquido; tudo indica a CPU, que só o Windows envia. Por isso a curva é plana.
   - Curva que segue a temperatura: enviar a temperatura da CPU com `E0`/`E1`, como o `AorusLcdService`. Depende do daemon da Fase 2, porque o envio precisa ser contínuo.
5. `waterforce-lcd`: ler itens e espaço livre (`DF`, `FA`) primeiro (o modo, `E8`, já está no probe); depois modo e cores (`E7`, `E4`, `B6`) e, por último, envio de imagem (`F1`/`F2`), que grava na flash.
6. ~~`waterforce-led`~~: feito em 2026-10-20. `halo led waterforce <efeito>` com os 14 efeitos do GCC e o desligado, paleta (`B0`–`B3`), brilho e velocidade. Confirmado no aparelho: o anel e as fans acendem juntos, e o `cycle` reproduz o modo de fábrica. O probe ganhou as leituras `DD`, `D9`, `C7` e `E8`.

## Fase 4: periféricos

1. Mouse G502 SE HERO.
2. Teclado Redragon (só efeitos e brilho do LED vermelho). Alto risco: só depois de analisar o software oficial da Redragon com o REA. Ver `hardware.md`.

## Fase 5: RAM (decisão pendente)

Só se decidirmos liberar o SMBus com `acpi_enforce_resources=lax`. Kingston Fury DDR4, endereço `0x58`.

## Perguntas em aberto

- A placa não tem nenhuma fita RGB conectada. Como validar visualmente os comandos de cor da Fase 1 (itens 3 e 4)?
- ~~O Waterforce é um X II 240 ou um Waterforce X 240?~~ É um **Waterforce X** (primeira geração): o GCC registra `1044_7A4D` como `GP-AORUS WATERFORCE X` (ver [hardware.md](hardware.md)).

- Quais efeitos do RGB Fusion 2.0 são indispensáveis?
