# Proposal

## Why

Com a change `waterforce-speed-mode`, o halo passa a escolher os modos prontos da fan e da bomba, mas não define a curva Customized, que o GCC oferece. A curva é a primeira escrita do halo em que o usuário escolhe os RPM, então pode deixar o resfriamento abaixo do que o fabricante usa.

O experimento no aparelho (2026-10-20, `docs/hardware.md`) mostrou que a temperatura de referência da curva **não é a do líquido**. Com o líquido a 26 °C, a fan foi ao ponto de uma curva que só subia acima de 29 °C. O código do GCC aponta para a temperatura da CPU: a tela da fan mostra a temperatura da CPU ao lado da curva (`FanControl.tb_CPUTemp`), e só o `AorusLcdService` a envia ao aparelho (`99 E0`). No Linux, ninguém envia o `E0`, e o firmware se comporta como se a CPU estivesse perto de 50 °C. Uma curva com degraus ficaria presa num ponto que o usuário não escolheu. Uma curva plana (o mesmo RPM nos 4 pontos) não depende da referência, e o aparelho a seguiu: 1500 RPM deram duty 153 (1500/2500 × 255).

Por isso, esta change grava só curvas planas, com limites de segurança por tipo.

## What Changes

- Novo subcomando `halo speed waterforce curve <fan|pump> <rpm>`, que grava a curva Customized plana de um lado: 4 pontos em 0, 30, 50 e 80 °C (as temperaturas do slot de fábrica), todos com o RPM informado. Ele usa a mesma sequência do botão Aplicar do GCC (`FanControl.OnClickApply`): os dois `E5` (`99 E5 01 …` e `99 E5 02 …`, com o lado da curva em Customized `01` e o outro repetindo o modo cru lido no `DD`), `99 E6 00 <tipo>` com os pontos e `99 B6`. Também tem um modo de simulação que mostra os pacotes sem abrir o aparelho, e uma opção experimental `--no-save` que envia os `E5` e o `E6` sem o `B6`, como o `--no-save` do `set`.
- Validação do RPM antes de abrir o aparelho:
  - Fan: de 900 a 2500 RPM (`DeviceApi.GetFanSpeedLimit`, PID `0x7A4D`).
  - Bomba: de 2600 a 2800 RPM. É uma regra do halo, porque o GCC aplica à bomba a faixa da fan.
  - As temperaturas fixas já cumprem as regras da interface do GCC (`GFanCurve`): de 0 a 100 °C, estritamente crescentes, e RPM sem queda.
- Antes de gravar, o snapshot do estado atual (`DD`, `D9 01`, `D9 02`). Depois, o "voltar ao anterior", que volta os modos e reenvia a curva só do lado que estava em Customized no snapshot. O slot Customized de um lado que estava num preset continua com a última curva gravada, sem estar ativo. Os dois são reaproveitados da `waterforce-speed-mode`. A restauração não passa pelas faixas da gravação, nem exige curva plana, porque devolve um estado que já estava no aparelho.
- O experimento da temperatura de referência já foi feito. O resultado vai para `docs/hardware.md` e `docs/protocols/castor3.md`.
- Fora do escopo: curvas com degraus, o envio de `E0`/`E1` (exige daemon, Fase 2, e fica para uma change futura), ZeroRPM (modo 7) e FixedRPM (modo 3).

**Dependência:** esta change estende a capability `waterforce-speed`, criada pela `waterforce-speed-mode`, que já está aplicada e arquivada (PR #6). Ela fornece o envio do `E5`, o snapshot e o "voltar ao anterior", e esta change não duplica essas partes. A speed-mode confirmou que o `E5` vale sem o `B6`, mas só até desligar, e que o `B6` persiste o modo. O experimento desta change confirmou que o `E6` também vale na hora sem o `B6` e aparece no `D9`. Ainda não se sabe se ele continua depois de desligar.

## Capabilities

### New Capabilities
- Nenhuma.

### Modified Capabilities
- `waterforce-speed` (criada pela `waterforce-speed-mode`): ganha a gravação da curva Customized plana da fan e da bomba, com a faixa de RPM de cada tipo. Muda dois requisitos existentes: "Comandos permitidos" passa a aceitar o `E6` no subcomando `curve`, e "Snapshot validado antes de restaurar" deixa explícito que a restauração não aplica as regras da gravação de curva. O modelo conferido, o snapshot, a aplicação única e o aparelho ausente já valem para qualquer escrita e não são repetidos.

## Impact

- `crates/halo-core/src/waterforce/`: o módulo `curve.rs` com o tipo da curva plana, a validação do RPM e o alvo `E5`/`E6`. Usa o `CurvePoint` e o `Target::packets` existentes, e o envio e o snapshot da `waterforce-speed-mode`.
- `crates/halo-cli`: subcomando `curve` sob o `halo speed waterforce`, com testes de integração.
- `docs/hardware.md` e `docs/protocols/castor3.md`: o resultado do experimento, o `E6` confirmado no aparelho e os achados do GCC sobre a temperatura da CPU.
- `README.md`: exemplos do comando e o aviso sobre a referência.
- Hardware: muda o RPM da fan ou da bomba e, sem o `--no-save`, grava na flash do Castor3 (`B6`). Toda execução no aparelho depende do aval do usuário.
