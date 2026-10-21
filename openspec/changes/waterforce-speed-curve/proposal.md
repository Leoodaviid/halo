# Proposal

## Why

Com a change `waterforce-speed-mode`, o halo passa a escolher os modos prontos da fan e da bomba, mas não define a curva Customized, que o GCC oferece. A curva é a primeira escrita do halo em que o usuário escolhe os RPM, então pode deixar o resfriamento abaixo do que o fabricante usa. Por isso, a change precisa de limites de segurança e de uma confirmação, no aparelho, da temperatura que o firmware usa como referência. Hoje ela é só inferida como a do líquido.

## What Changes

- Novo subcomando `halo speed waterforce curve` para gravar a curva Customized de 4 pontos (temperatura °C → RPM) da fan ou da bomba. Ele usa a mesma sequência do botão Aplicar do GCC (`FanControl.OnClickApply`): os dois `E5` (`99 E5 01 …` e `99 E5 02 …`, com o lado da curva em Customized `01` e o outro repetindo o modo cru lido no `DD`), `99 E6 00 <tipo>` com os pontos e `99 B6`. Também tem um modo de simulação que mostra os pacotes sem abrir o aparelho, e uma opção experimental `--no-save` que envia os `E5` e o `E6` sem o `B6`, como o `--no-save` do `set`.
- Validação da curva antes de abrir o aparelho, com as regras da interface do GCC (`GFanCurve`): temperatura de 0 a 100 °C, estritamente crescente; RPM que nunca diminui de um ponto para o seguinte. Faixas por tipo:
  - Fan: de 900 a 2500 RPM (`DeviceApi.GetFanSpeedLimit`, PID `0x7A4D`).
  - Bomba: de 2600 a 2800 RPM. É uma regra do halo, porque o GCC não tem limite próprio para a bomba.
- Antes de gravar, o snapshot do estado atual (`DD`, `D9 01`, `D9 02`). Depois, o "voltar ao anterior", que volta os modos e reenvia a curva só do lado que estava em Customized no snapshot. O slot Customized de um lado que estava num preset continua com a última curva gravada, sem estar ativo. Os dois são reaproveitados da `waterforce-speed-mode`. A restauração não passa pelas faixas da gravação de curva, porque devolve um estado que já estava no aparelho.
- Experimento no aparelho para confirmar a temperatura de referência da curva: líquido (`temp1_input` do hwmon) ou CPU (`E0`/`E1`, que só o `AorusLcdService` do Windows envia). Esse experimento vem antes de fechar as regras de segurança. Se a referência for a CPU, a change é pausada e reescopada.
- Fora do escopo: ZeroRPM (modo 7) e FixedRPM (modo 3).

**Dependência:** esta change estende a capability `waterforce-speed`, criada pela `waterforce-speed-mode`, que já está aplicada e arquivada (PR #6). Ela fornece o envio do `E5`, o snapshot e o "voltar ao anterior", e esta change não duplica essas partes. A speed-mode confirmou no aparelho que o `E5` vale sem o `B6`, mas só até desligar, e que o `B6` persiste o modo. Ainda não se sabe se o `E6` também vale sem o `B6`.

## Capabilities

### New Capabilities
- Nenhuma.

### Modified Capabilities
- `waterforce-speed` (criada pela `waterforce-speed-mode`): ganha a gravação e a validação da curva Customized de 4 pontos da fan e da bomba. Muda dois requisitos existentes: "Comandos permitidos" passa a aceitar o `E6` no subcomando `curve`, e "Snapshot validado antes de restaurar" deixa explícito que a restauração não aplica as faixas da curva. O modelo conferido, o snapshot, a aplicação única e o aparelho ausente já valem para qualquer escrita e não são repetidos.

## Impact

- `crates/halo-core/src/waterforce/`: novo módulo com o tipo da curva, a validação e o pacote `E6`. Usa o `packet()` do `probe.rs`, o `CurvePoint` existente e o envio e o snapshot da `waterforce-speed-mode`.
- `crates/halo-cli`: subcomando `curve` sob o `halo speed waterforce` criado pela `waterforce-speed-mode`, com testes de integração.
- `docs/hardware.md` e `docs/protocols/castor3.md`: resultado da temperatura de referência, o `E6` confirmado no aparelho e se ele vale sem o `B6`. O `D9` como slot Customized já foi confirmado pela `waterforce-speed-mode`.
- `README.md`: exemplos do comando.
- Hardware: muda o RPM da fan ou da bomba e, sem o `--no-save`, grava na flash do Castor3 (`B6`). Toda execução no aparelho depende do aval do usuário.
