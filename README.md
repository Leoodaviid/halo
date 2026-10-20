# halo

Controle de RGB nativo para Linux, feito para o hardware deste PC. A ideia é cobrir o que o RGB Fusion 2.0 e o GIGABYTE Control Center fazem no Windows, com uma interface própria e minimalista: um ícone na barra superior do GNOME que abre um painel com cores, zonas, efeitos e perfis.

Não é um clone da interface da Gigabyte nem uma camada sobre o OpenRGB. O halo fala direto com cada controlador. O código do OpenRGB serve só como referência de protocolo.

## Estado

Fase de análise concluída em 2026-10-20. Ainda não há código. Veja:

- [docs/hardware.md](docs/hardware.md): inventário do hardware, protocolos e permissões.
- [docs/architecture.md](docs/architecture.md): arquitetura proposta.
- [docs/roadmap.md](docs/roadmap.md): fases e próximos passos.

## Hardware alvo

| Dispositivo | Controlador | Situação |
|---|---|---|
| Placa-mãe Gigabyte B550M AORUS ELITE | ITE IT5702 (`048d:5702`), USB HID | Protocolo conhecido, acesso liberado |
| AORUS Waterforce X II 240, anel RGB | Castor3 (`1044:7a4d`), USB HID | Protocolo conhecido, precisa de regra udev |
| AORUS Waterforce X II 240, tela LCD | Castor3 (`1044:7a4d`), USB HID | Protocolo desconhecido, investigar com REA |
| Mouse Logitech G502 SE HERO | `046d:c08b`, USB HID | Protocolo conhecido |
| Teclado Redragon edição Horda (só LED vermelho) | Sino Wealth (`258a:013b`), USB HID | Protocolo desconhecido e de alto risco, investigar com REA |
| RAM Kingston Fury DDR4 (`KF3200C16D4/16GX`) | SMBus, endereço `0x58` | Adiado: SMBus bloqueado pelo ACPI |
