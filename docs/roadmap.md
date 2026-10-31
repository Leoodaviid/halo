# Roadmap

## Fase 0: análise (concluída em 2026-10-20)

Inventário de hardware, protocolos e permissões. Ver [hardware.md](hardware.md).

## Fase 1: placa-mãe (IT5702)

1. ~~Criar o workspace Rust e decidir a licença.~~ Feito em 2026-10-20: workspace `crates/halo-core` + `crates/halo-cli`, GPL-2.0-or-later.
2. ~~Ferramenta só de leitura (`CC 60`).~~ Feito em 2026-10-20: `halo probe it5702 --raw`. Valores lidos em [hardware.md](hardware.md).
3. Cor estática por zona (`D_LED1`, `D_LED2`, `LED_C1`, `LED_CPU`, `LED_C2`) e ligar/desligar.
4. Efeitos nativos do controlador.

## Fase 2: daemon e interface

1. ~~Daemon com D-Bus e perfis salvos.~~ Feito em 2026-10-21: `halod` (change `halo-daemon`), com LED, velocidade e telemetria do Waterforce pelo D-Bus da sessão, perfis em TOML que só gravam a velocidade quando ela muda, ticks que só leem o sysfs e acesso exclusivo ao hidraw (`flock`) para a CLI e o daemon. Em 2026-10-27, o `waterforce-cpu-feed` acrescentou o envio de `E0`/`E1` pelo worker, sob o watchdog da unidade.
2. ~~Protótipo visual do painel para aprovação antes de codar a extensão.~~ Aprovado em 2026-10-22: [`docs/ui/panel-prototype.html`](ui/panel-prototype.html) (change `gnome-panel-prototype`, spec `gnome-panel`), no visual do canvas "halo · Apple HIG" com o destaque do GNOME, a temperatura na barra, o carrossel guiado pelo daemon, os 9 efeitos de uma cor e a aplicação só no fim de cada gesto.
3. Extensão GNOME Shell com o ícone na barra e o painel, dividida em sete changes. Segue o protótipo e a spec `gnome-panel`. O `Devices` com o nome e as capacidades de cada aparelho (change `daemon-device-status`) já existe.
   - ~~`gnome-shell-extension`~~: feito em 2026-10-22. Indicador na barra com a temperatura do líquido (`--` quando inválida, só o ícone sem leitura, esmaecido com o daemon fora do ar), painel mínimo com o estado e a contagem de aparelhos, "Iniciar halod" e "Ver registro", sem subir o daemon sozinha. `packaging/install.sh` instala e habilita a extensão. Testes: `extension/tests/run.sh`, `scripts/dev/headless-smoke.sh` e `scripts/dev/nested-shell.sh`.
   - ~~`gnome-panel-devices`~~: feito em 2026-10-23. Carrossel dos aparelhos presentes (setas, indicador de página, roda e teclas, circular), desenhos locais em SVG com o anel do Waterforce na cor do `LastApplied`, o aviso de controle em desenvolvimento nos aparelhos sem `led` e o liga/desliga desabilitado. Só lê do daemon. O sysfs falso dos testes (`scripts/dev/fake-sysfs.sh`) usa `hidraw99`, que não existe nesta máquina.
   - ~~`gnome-panel-led`~~: feito em 2026-10-23. Controles do LED (cor, efeito, brilho, velocidade), liga/desliga habilitado, fila de `Apply` (um em voo, só o último valor pendente, nenhum igual ao aplicado) e religar (último ligado, LED do perfil ativo, `cycle`). O `nested-shell.sh --daemon` passa ao sysfs falso, e o `scripts/dev/watch-led-applies.sh` conta os `Apply` e os `B6` na conferência no aparelho.
   - `led-state-persist`: implementada em 2026-10-25, com conferência física aceita pelo usuário (task 4.2). O `halod` guarda o último LED aplicado em `~/.local/state/halo/waterforce-led.last` e o publica de novo ao iniciar, sem enviar nada ao aparelho. Os testes cobrem o primeiro `Get` sem abrir hidraw, o painel com a cor e o switch ligado após login/reboot, e desligar/religar com a mesma cor. O corte de energia de 2026-10-25 confirmou que o estático branco aplicado pelo painel com `B6` permanece no aparelho; o usuário também confirmou o fluxo após instalar a persistência, usando estático `fffbeb` (task 4.2; limites da observação registrados nas tasks).
   - ~~`gnome-panel-cpu-source`~~: feito e arquivado em 2026-10-27. Preferências da extensão (`prefs.js`, schema GSettings compilado na instalação) com a fonte da temperatura da barra, líquido (padrão) ou CPU, trocada na hora, e o item "Preferências do halo…" no rodapé do painel. A CPU vem do `CpuMilliCelsius`/`CpuValid` que a `waterforce-cpu-feed` publica.
   - ~~`gnome-panel-profiles`~~: feito e arquivado em 2026-10-31. Menu de perfis no cabeçalho (lido de novo a cada abertura, com o ativo marcado e o efeito de cada um), a ativação na mesma fila dos `Apply`, a faixa de erro por tipo com "Tentar de novo" e a linha de status no rodapé. Sem mudança no `halod`.
   - `gnome-panel-details`: pendente. A página de detalhes do aparelho, o item "Detalhes do aparelho…" e o comando da CLI.
4. ~~Serviço `systemd --user`.~~ Feito em 2026-10-22 (change `systemd-user-service`): `packaging/install.sh` instala no usuário os binários, a unidade `halod.service` e a ativação pelo D-Bus, e `--uninstall` remove mantendo perfis e snapshots. Em 2026-10-30, a change `halod-outside-session` (concluída e ensaiada no aparelho) tirou a unidade da sessão gráfica: com o linger e a regra udev com `GROUP="plugdev"` no Castor3, o `halod` sobe no boot e manda a CPU na tela de login; a `halod-session.service` é a barreira do logout quando o acesso depende só da ACL da sessão. Não reaplica o perfil no login: o Waterforce guarda a velocidade gravada com `B6`, e o GCC não reenvia nada no boot. O estático branco aplicado pelo painel com `B6` também sobreviveu ao corte de energia em 2026-10-25 (ver `docs/hardware.md`); não é preciso reaplicá-lo no boot.

## Fase 3: Waterforce

1. ~~Regra udev para liberar `1044:7a4d` ao usuário.~~ Feito em 2026-10-20: `packaging/udev/70-halo.rules`.
2. ~~Telemetria e leitura de firmware.~~ Feito em 2026-10-20: `halo telemetry waterforce` e `halo probe waterforce`. O firmware não confirmou a compatibilidade com o protocolo de anel do OpenRGB (ver [hardware.md](hardware.md)).
3. ~~Analisar o GIGABYTE Control Center com o REA.~~ Feito em 2026-10-20: velocidade, status e LCD no C#, e anel e fans na `GcLedLib.dll` com o Ghidra. Ver [protocols/castor3.md](protocols/castor3.md).
4. Velocidade, dividida em duas changes:
   - ~~`waterforce-speed-mode`~~: feito em 2026-10-20. `halo speed waterforce status|set|restore`, só com os presets do firmware (`E5` + `B6`) e com snapshots anterior e original. Confirmado no aparelho: o `E5` vale sem o `B6`, mas só até desligar, o `B6` persiste o modo, e o `D9` é o slot Customized.
   - ~~`waterforce-speed-curve`~~: feito em 2026-10-21. `halo speed waterforce curve <fan|pump> <rpm>`, uma curva Customized plana (`E6`). Confirmado no aparelho: o `E6` vale sem o `B6`, e a referência da curva não é o líquido; tudo indica a CPU, que só o Windows envia. Por isso a curva é plana.
   - ~~Curva que segue a temperatura.~~ Feito em 2026-10-27 (change `waterforce-cpu-feed`): o `halod` envia o Tctl com `E1`/`E0` a cada 2 s, como o `AorusLcdService`, protege com `E0` de 80 °C no SIGTERM e no `ExecStopPost`, e o D-Bus ganhou a temperatura da CPU e a curva da fan com degraus (`SetFanCurve`). Ensaios no aparelho em `docs/hardware.md`.
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
