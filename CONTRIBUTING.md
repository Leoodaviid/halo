# Contribuindo com o halo

## Fluxo

- A `main` é protegida: nada entra sem pull request aprovado por @Leoodaviid. Não há push direto, force push nem exclusão da `main`.
- Trabalhe num branch (`feat/...`, `fix/...`, `docs/...`) e abra o PR contra a `main`.
- Um PR por assunto. Descreva o que mudou, em que hardware e sistema foi testado e como reproduzir o teste.

## Regras de hardware (valem para todo mundo)

Um comando errado pode inutilizar um aparelho. As regras do [`CLAUDE.md`](CLAUDE.md) valem para qualquer contribuição:

- Nunca enviar ao hardware um comando que não esteja documentado em `docs/hardware.md`, no código do OpenRGB ou numa análise do software oficial. **Todo dispositivo novo começa com uma ferramenta que só lê.**
- Teclado Redragon (`258a:013b`): não usar os comandos Sino Wealth do OpenRGB. Só agir com protocolo confirmado pelo software oficial da Redragon.
- Não mexer na RAM/SMBus nem em BIOS ou parâmetros de boot sem combinar antes.
- Encontrar dispositivos por VID:PID e usage page, nunca por caminho fixo (`/dev/hidrawN` no Linux, caminho de dispositivo no Windows).
- O OpenRGB é só referência de protocolo, não dependência. Código portado dele mantém o projeto em GPL-2.0-or-later.

## Hardware diferente e Windows

- Um modelo novo (outro Waterforce, outra revisão de placa) entra primeiro como **leitura**: VID:PID, descritor HID, respostas observadas. Só depois vem a escrita.
- No Windows, capturas do software oficial (GCC/RGB Fusion) com USBPcap/Wireshark são a melhor fonte de protocolo. Anexe ao PR o trecho relevante e o modelo exato do aparelho.
- Toda descoberta sobre hardware ou protocolo vai para `docs/hardware.md`, com a data e o aparelho em que foi vista.
- Código específico de sistema fica atrás de `cfg(target_os = ...)`. A montagem dos pacotes de protocolo não deve depender do sistema.

## Antes de abrir o PR

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

Se não der para testar no aparelho, diga isso no PR.

## Licença

Ao contribuir, você concorda em licenciar sua contribuição sob GPL-2.0-or-later.
