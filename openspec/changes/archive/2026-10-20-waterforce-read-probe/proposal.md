# Proposal

## Why

A placa-mãe não tem nenhuma fita RGB ligada, então o primeiro dispositivo em que dá para ver resultado é o AORUS Waterforce (`1044:7a4d`). Antes de mandar qualquer cor, falta resolver três coisas:

- O hidraw dele só abre como root.
- Não está confirmado que o protocolo do anel do OpenRGB vale para este aparelho. O driver do OpenRGB foi escrito para o X II 360 (`0414:7a5e`), e o kernel registra o `1044:7a4d` como Waterforce X 240/280/360 da primeira geração.
- Ninguém olhou ainda o que o aparelho já informa sozinho.

Também descobrimos que o kernel já tem o driver `gigabyte_waterforce`, que expõe a telemetria de fan, bomba e líquido em hwmon sem nenhum comando do halo.

## What Changes

- Telemetria do Waterforce pelo hwmon do kernel: RPM e duty da fan e da bomba, e temperatura do líquido. O halo só lê arquivos do sysfs e não envia nada ao aparelho.
- Regra udev própria do halo (`TAG+="uaccess"`) para o IT5702 (`048d:5702`) e o Castor3 (`1044:7a4d`). Hoje o acesso ao IT5702 depende do `60-openrgb.rules`, que também libera todos os barramentos `i2c-*`. A regra fica no repositório e o usuário a instala com sudo.
- `halo probe waterforce`: leitura de identificação pelo hidraw com os dois pedidos de leitura documentados, `D6` (firmware, driver do kernel) e `DE 00` (firmware, driver do OpenRGB). Nenhum comando de LED, LCD, fan ou bomba.
- Registro em `docs/hardware.md` da telemetria, das respostas de firmware e da situação das regras udev.

Fora do escopo, para changes seguintes:
- Cor e efeitos do anel, só depois que o probe confirmar a compatibilidade com o protocolo do OpenRGB.
- Controle de velocidade da fan e da bomba, que não tem protocolo documentado e depende de análise do GIGABYTE Control Center com o REA.
- RGB das fans e tela LCD, também dependentes do REA.

## Capabilities

### New Capabilities

- `waterforce-telemetry`: leitura de RPM, duty e temperatura do Waterforce pelo hwmon do kernel.
- `waterforce-probe`: leitura de identificação e firmware do controlador Castor3 pelo hidraw, sem alterar o estado dele.
- `device-permissions`: regra udev que libera ao usuário da sessão os controladores que o halo usa, sem depender de regras de terceiros.

### Modified Capabilities

Nenhuma. `device-discovery` já cobre a busca por VID:PID e usage page, e a falta de permissão.

## Impact

- Código: módulos novos em `halo-core` (hwmon e Castor3) e subcomandos novos em `halo-cli`.
- Sistema: arquivo de regra udev em `packaging/udev/`, instalado à mão em `/etc/udev/rules.d/` (precisa de sudo, feito pelo usuário).
- Hardware: só o Waterforce, com dois pedidos de leitura. O pedido `D6` já é enviado pelo próprio kernel ao carregar o driver.
- Fontes de protocolo: `drivers/hwmon/gigabyte_waterforce.c` do Linux (GPL-2.0+) e `Controllers/GigabyteCastor3Controller/` do OpenRGB (`72262b8`).
