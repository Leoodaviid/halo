# Proposal

## Why

O halo ainda não tem código, e toda a Fase 1 depende de falar com o controlador ITE IT5702 da placa-mãe. Antes de qualquer escrita de cor, precisamos de uma ferramenta que só lê e confirma que encontramos o dispositivo certo e interpretamos a resposta dele como o OpenRGB documenta. A placa já respondeu a esse comando de leitura (`CC 60`) quando o OpenRGB a detectou em 2026-10-17, então é o primeiro passo de menor risco.

## What Changes

- Cria o workspace Rust do projeto, com licença GPL-2.0-or-later, porque a lógica de protocolo é portada do driver `GigabyteRGBFusion2USBController` do OpenRGB.
- Adiciona uma camada de descoberta de dispositivos HID que encontra controladores por VID:PID e usage page, nunca por `/dev/hidrawN` fixo.
- Adiciona um comando de linha `halo probe it5702` (nome final no design) que envia apenas `CC 60`, lê o feature report `0xCC` e mostra: descrição do produto, versão de firmware, número do dispositivo, contagem de LEDs por fita ARGB, flags de comandos suportados, chip id e valores de calibração.
- Diferente do OpenRGB, o probe **não** envia os comandos que o construtor do driver original manda logo depois da leitura (`ResetController`, `EnableBeat`, `EnableLampArray`).
- Atualiza `docs/hardware.md` com os valores reais lidos da placa.

## Capabilities

### New Capabilities

- `device-discovery`: encontrar controladores RGB suportados pelo VID:PID e usage page HID, e informar de forma clara quando não há dispositivo ou quando falta permissão.
- `it5702-probe`: leitura somente-leitura da configuração do controlador ITE IT5702 e apresentação dos campos decodificados.

### Modified Capabilities

Nenhuma. Não há specs existentes.

## Impact

- Código novo: workspace Rust na raiz do repositório (até agora só havia documentação).
- Dependências novas: toolchain Rust (não está instalada nesta máquina) e um crate para acesso HID via hidraw.
- Hardware: só a placa-mãe IT5702 (`048d:5702`), já acessível ao usuário sem root. Nenhum outro dispositivo é aberto.
- Licença: o repositório passa a ser GPL-2.0-or-later.
