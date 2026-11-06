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

Os caminhos abaixo (`/mnt/workspace/...`, `/home/bumblebee/...`) existem só na máquina Linux do mantenedor. Os mesmos caminhos aparecem em `docs/` como registro de onde cada análise foi feita. Em outra máquina:

- OpenRGB: clonar `https://gitlab.com/CalcProgrammer1/OpenRGB` e usar o commit `72262b8`, que é a referência citada nos docs.
- Software oficial: baixar no site do fabricante e conferir o início e o fim do sha256 citados abaixo; se a versão for outra, anotar o sha256 completo no PR. Os instaladores para análise estática nunca são executados. No Windows, o software oficial instalado normalmente (GCC, RGB Fusion) pode ser usado para capturar o tráfego USB com o USBPcap e o Wireshark, a melhor fonte para confirmar um protocolo.
- REA, Ghidra, ilspycmd e as saídas já extraídas: só na máquina do mantenedor. Se a análise depender deles, peça no PR em vez de supor o conteúdo.

Na máquina do mantenedor:

- Código do OpenRGB: `/mnt/workspace/tools/repos/openrgb` (clone raso, commit `72262b8`), drivers em `Controllers/`.
- Instaladores oficiais para análise com o REA, em `/mnt/workspace/tools/samples/` (nunca executar). Cada produto tem `installer/` com o arquivo original e `extracted/` para o conteúdo extraído:
  - `gigabyte-control-center/`: `GCC_26.09.10.01.zip` (sha256 `b046a63a…2f1b`), com `GIGABYTE Control Center_2026_AUG_release_All_Setup_26.09.10.01.exe` (NSIS) em `extracted/zip/`.
  - `rgb-fusion/`: `RGB_Fusion_B26.0923.1.zip` (sha256 `bd63109b…a0d`), com `RGBFusion/UpdPack_B26.0923.1.exe` (NSIS) em `extracted/zip/`.
  - O REA não desempacota NSIS. Extrair com 7-Zip (`7z x`) numa subpasta vazia de `extracted/`. O `GBT_Cooler_*.exe` de dentro do GCC é um NSIS com campos de 64 bits que o 7-Zip não abre: usar `scripts/re/nsis64_blocks.py`. Os blocos ficam em `extracted/nsis-gbt-cooler-blocks/`, a visão com os nomes reais em `extracted/nsis-gbt-cooler-named/`, e o mapa bloco → nome → sha256 em `NSIS-GBT-COOLER-BLOCKS.tsv`.
  - `gigabyte-b550m-aorus-elite-bios/`: BIOS F20g, F21 e F22d (a F22d está instalada desde 2026-11-01), extraídos com o UEFIExtract NE A75 (`/home/bumblebee/.local/opt/uefiextract-ne-a75/uefiextract`) em `extracted/uefi-*/`. Os módulos SMM e as SSDT tiradas da imagem ficam em `analysis/`. Nunca gravar na placa.
  - `kingston-fury-ctrl/`: FURY CTRL 2.0.66.0, o software oficial da RAM (o site bloqueia download automático; o usuário baixa pelo navegador). O SDK é um Inno Setup 6.6.1, que só o `innoextract` do fork crazy-max abre (`/mnt/workspace/tools/repos/innoextract-crazy-max/build/innoextract`, compilado do código-fonte). Saídas em `extracted/` (`inno-sdk/app`, `ilspy-*`) e `analysis/decomp-FuryCTRL/`.
  - Arquivos-chave do Waterforce: `ucCooler.dll` (fan, bomba, LCD), `AorusLcdService.exe` (dados de CPU para o LCD) e `block-006.dll` = `GcLedLib.dll` (LED, nativa). Os snapshots do Ghidra ficam em `<produto>/analysis/`.
- REA (Reverse Engineer Anything): clone e build em `/mnt/workspace/tools/repos/rea`, registrado como MCP `rea` no perfil `ld` do Claude Code. Funciona para .NET, JavaScript/Electron e web. Para binários nativos usa o Ghidra 12.1.4 (`/home/bumblebee/.local/opt/ghidra_12.1.4_PUBLIC`) com o JDK 21 Temurin (`/home/bumblebee/.local/opt/jdk-21.0.12.1+1`), passados ao MCP por `GHIDRA_INSTALL_DIR` e `JAVA_HOME`. Na CLI: `node scripts/rea.mjs decompile <bin> <função> --provider ghidra --snapshot <arquivo>`.
- Decompilador .NET: .NET SDK 8.0 em `/home/bumblebee/.dotnet` e `ilspycmd` 9.1 em `/home/bumblebee/.dotnet/tools/ilspycmd` (exportar `DOTNET_ROOT=/home/bumblebee/.dotnet`). O REA não decompila C#. As saídas do GCC ficam em `extracted/ilspy-*/`, e o protocolo extraído está em `docs/protocols/castor3.md`.
