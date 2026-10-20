# halo

Controle de RGB nativo para Linux, feito para o hardware deste PC. A ideia é cobrir o que o RGB Fusion 2.0 e o GIGABYTE Control Center fazem no Windows, com uma interface própria e minimalista: um ícone na barra superior do GNOME que abre um painel com cores, zonas, efeitos e perfis.

Não é um clone da interface da Gigabyte nem uma camada sobre o OpenRGB. O halo fala direto com cada controlador. O código do OpenRGB serve só como referência de protocolo.

## Estado

Fase de análise concluída em 2026-10-20. Já existem ferramentas que só leem, sem alterar nada:

```sh
cargo run -q -p halo-cli -- probe it5702 --raw          # placa-mãe
cargo run -q -p halo-cli -- telemetry waterforce         # fan, bomba e líquido
cargo run -q -p halo-cli -- probe waterforce --raw      # firmware do Waterforce
```

O probe do Waterforce precisa da regra udev do halo (ver [docs/permissions.md](docs/permissions.md)).

Precisa do Rust estável (rustup). Licença: GPL-2.0-or-later, porque a lógica de protocolo é portada do OpenRGB.

Veja:

- [docs/hardware.md](docs/hardware.md): inventário do hardware, protocolos e permissões.
- [docs/architecture.md](docs/architecture.md): arquitetura proposta.
- [docs/roadmap.md](docs/roadmap.md): fases e próximos passos.

## Hardware alvo

| Dispositivo | Controlador | Situação |
|---|---|---|
| Placa-mãe Gigabyte B550M AORUS ELITE | ITE IT5702 (`048d:5702`), USB HID | Protocolo conhecido, acesso liberado |
| AORUS Waterforce X II 240, telemetria | Driver `gigabyte_waterforce` do kernel (hwmon) | Funciona, só leitura |
| AORUS Waterforce X II 240, anel e fans RGB | Castor3 (`1044:7a4d`), USB HID | Protocolo do OpenRGB não confirmado neste firmware, investigar com REA |
| AORUS Waterforce X II 240, tela LCD | Castor3 (`1044:7a4d`), USB HID | Protocolo desconhecido, investigar com REA |
| Mouse Logitech G502 SE HERO | `046d:c08b`, USB HID | Protocolo conhecido |
| Teclado Redragon edição Horda (só LED vermelho) | Sino Wealth (`258a:013b`), USB HID | Protocolo desconhecido e de alto risco, investigar com REA |
| RAM Kingston Fury DDR4 (`KF3200C16D4/16GX`) | SMBus, endereço `0x58` | Adiado: SMBus bloqueado pelo ACPI |
