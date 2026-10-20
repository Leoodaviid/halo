# halo

Controle de RGB nativo para Linux (GNOME/Wayland) para o hardware deste PC. Leia `README.md` e `docs/` antes de começar: a análise de hardware, a arquitetura e o roadmap já estão lá.

## Regras

- Nunca enviar ao hardware um comando que não esteja documentado em `docs/hardware.md`, no código do OpenRGB ou numa análise feita com o REA. Todo dispositivo novo começa com uma ferramenta que só lê.
- Teclado Redragon (`258a:013b`): os drivers Sino Wealth do OpenRGB já inutilizaram teclados Redragon. Não usar comandos deles. Só agir com protocolo confirmado pelo software oficial da Redragon.
- Não mexer na RAM/SMBus nem em parâmetros de boot (`acpi_enforce_resources=lax`) sem pedido explícito do usuário.
- Encontrar dispositivos por VID:PID e usage page, nunca por `/dev/hidrawN` fixo.
- O OpenRGB é só referência de protocolo, não dependência. Ao portar código dele, o projeto precisa ser GPL-2.0-or-later.
- Ao descobrir algo novo sobre o hardware ou o protocolo, atualizar `docs/hardware.md`.

## Ferramentas disponíveis

- Código do OpenRGB: `/mnt/workspace/tools/repos/openrgb` (clone raso, commit `72262b8`), drivers em `Controllers/`.
- REA (Reverse Engineer Anything): clone e build em `/mnt/workspace/tools/repos/rea`, registrado como MCP `rea` no perfil `ld` do Claude Code. Funciona para .NET, JavaScript/Electron e web. Binários nativos ainda precisam de JDK 21+ e Ghidra 12.1.x, que não estão instalados.
