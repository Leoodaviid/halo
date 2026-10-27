# Protocolo do Castor3: AORUS Waterforce X (`1044:7a4d`)

Análise estática do GIGABYTE Control Center, sem executar nada e sem enviar nenhum byte ao aparelho. Só os comandos marcados como **confirmado** podem entrar em changes de implementação.

## Fontes

| Arquivo | Origem | sha256 |
|---|---|---|
| `ucCooler.dll` (.NET 4.7.2) | `GBT_Cooler_26.08.24.01.exe`, dentro do `GCC_26.09.10.01.zip` | `5d08e67f…fb36dd` |
| `AorusLcdService.exe` (.NET) | mesmo pacote | ver `NSIS-GBT-COOLER-BLOCKS.tsv` |
| `GvComApp.dll` (.NET, utilitários) | mesmo pacote | `3a7644f1…58b302` |
| `GcLedLib.dll` (nativo x64, `block-006`; PDB `GvLedLib_New_230704_WaterforceX2`) | mesmo pacote | `f2f36a21…acb9b3` |

Os caminhos e os hashes completos estão em `/mnt/workspace/tools/samples/gigabyte-control-center/` (`SHA256SUMS` e `NSIS-GBT-COOLER-BLOCKS.tsv`). Os assemblies .NET foram decompilados com o `ilspycmd` 9.1.0.7988 em `extracted/ilspy-uccooler/` e `extracted/ilspy-aoruslcdservice/`, e a triagem foi feita com o REA (`inspect_managed_artifact`, `inspect_managed_native_boundaries`). A `GcLedLib.dll` foi analisada com o Ghidra 12.1.4 pelo REA.

**Graus de confiança:**
- **confirmado**: os bytes estão escritos literalmente no código oficial, no caminho que o GCC usa para o 7A4D.
- **inferido**: deduzido do código, mas depende de um valor que o código não fixa (enum de interface, comportamento do firmware).
- **desconhecido**: não analisado.

## Identificação no GCC

- `Cooler_Controler.EnumDevice` procura `VID_1044&PID_7A4D`. O enum `DevicePId.DEVICE_X240_280_360 = 31309` (`0x7A4D`) corresponde à primeira geração. O X II é o `0414:7A5E` (`31326`).
- O ID do LED é `DeviceLedId.DEVICE_X240_280_360 = 0x4016` (16406). A interface do anel é `GLed_0x4016_240`.
- Os limites de fan usados pelo GCC (`DeviceApi.GetFanSpeedLimit`) vão de 900 a 2500 RPM. O máximo do gráfico (`MaxSpeed`) é 3200.

## Transporte

`CoolerApi.Send` e `CoolerApi.Receive` (`ucCooler.dll`):

- Cada comando é um output report que começa com o report ID `0x99` e é completado com zeros até `OutputReportLen`, o tamanho que o descritor declara (6144 bytes com o report ID, igual ao `MAX_REPORT_LENGTH` do kernel). O envio vai por `GvHidWriteByte(..., -1)` (`HidDriver.dll`).
- A resposta é um input report de 256 bytes. O `Receive` só aceita o report cujos bytes 0 e 1 sejam iguais aos do pedido (`99 <cmd>`), e para algumas listas também confere o byte 2 e o byte 3 (`nRecvCheckLength`). Os outros reports são descartados. É o mesmo filtro do `wait_response` do halo.
- Abaixo, `r[n]` é o byte `n` da resposta, contando o `0x99` como `r[0]`. A notação `99 XX …` mostra só os bytes iniciais, e o resto é zero.

## Identificação e status

| Comando | Bytes | Resposta | Fonte | Confiança |
|---|---|---|---|---|
| Modelo | `99 DE 00` | `r[2]`: 0 = X 240, 1 = X 280, 2 = X 360 | `DeviceApi.GetDeviceModelName` | confirmado |
| Firmware | `99 D6` | `r[2].r[3]` (maior.menor) | `CoolerDataApi.GetXiiFwVersion` (AorusLcdService, usado no X II) e o kernel | confirmado no X II e no kernel; no GCC o 7A4D lê o firmware por `GvLedGetDeviceFWVersion`, que é nativo |
| RPM | `99 DA` | fan = `r[2] \| r[3]<<8 \| r[4]<<16`; bomba = `r[5] \| r[6]<<8 \| r[7]<<16` | `DeviceApi.GetRPMSpeed` | confirmado, e confere com o kernel |
| Modo de velocidade | `99 DD` | `r[2]` modo da fan, `r[3]` modo da bomba (ver [enum](#speedmode)); o valor 3 é lido como 0 | `DeviceApi.GetMode` | confirmado |
| Curva | `99 D9 <tipo>` | 4 pontos: temperatura `r[4+3i]`, RPM `r[5+3i]<<8 \| r[6+3i]` (big-endian); é o slot Customized guardado, e não a curva ativa | `DeviceApi.GetSpeedCurve` (privado) | confirmado; slot Customized confirmado no aparelho |
| Quantidade de fans RGB | `99 C7` | `r[2]` | `DeviceApi.GetFanQty` | confirmado |
| Progresso de gravação | `99 C2` | `r[2]` em % | `LcdApi.GetFlashPercent`; só com firmware ≥ 2.0 (abaixo disso o GCC assume 100 %) | confirmado |

**Conferência com o aparelho** (respostas do `halo probe waterforce`, ver `docs/hardware.md`):
- `99 de 00` → `r[2] = 0`, que é **GP-AORUS WATERFORCE X 240**. O OpenRGB lê esse byte como versão. O GCC mostra que ele é o índice do modelo.
- `99 d6 01 06` → firmware **1.6**, igual ao "16" do kernel (`1 * 10 + 6`). Com firmware abaixo de 2.0, o GCC não consulta o `C2`.
- `99 DD`, `99 D9 01`, `99 D9 02`, `99 C7` e `99 E8` responderam com o próprio comando (change `waterforce-led`): fan em Performance, bomba em Turbo, curvas de 4 pontos até 80 °C, 2 fans RGB e LCD no modo 3. O `D9` repete o tipo em `r[2]` e `r[3]`. Os valores estão em `docs/hardware.md`.

## Velocidade da fan e da bomba

| Comando | Bytes | Fonte | Confiança |
|---|---|---|---|
| Definir modo | `99 E5 <tipo> <modo> 00` | `DeviceApi.SetSpeedMode` | confirmado no aparelho: vale sem o `B6`, mas só até desligar |
| Definir curva | `99 E6 00 <tipo>` e 4 pontos `<temp °C> <rpm_hi> <rpm_lo>` a partir do byte 4; pacote de 84 bytes | `DeviceApi.SetSpeedCurve` | confirmado no aparelho: o firmware segue a curva, que vale na hora sem o `B6` e aparece no `D9` |
| Salvar | `99 B6` | `DeviceApi.Save` e `LcdApi.Save` | confirmado no aparelho: persiste o modo depois de desligar com corte de energia |

`<tipo>` vem do `SpeedType`: `1` fan e `2` bomba.

<a id="speedmode"></a>`<modo>` vem do `SpeedMode`: `0` Balanced, `1` Customized, `2` Default, `3` FixedRPM, `4` Turbo, `5` Performance, `6` Quiet, `7` ZeroRPM. A interface da fan mostra Default, ZeroRPM, Quiet, Balanced, Performance, Turbo e Customized. A da bomba usa as curvas de `PumpApi`: 0, 1 e 4.

Sequência do botão Aplicar (`FanControl.OnClickApply`):
1. `99 E5 01 <modo da fan> 00`
2. `99 E5 02 <modo da bomba> 00`
3. Se a fan estiver em Customized, `99 E6 00 01 …`. Se a bomba estiver, `99 E6 00 02 …`.
4. `99 B6`

As curvas pré-definidas que o GCC mostra quando o modo não é Customized são **tabelas da própria interface** (`FanApi.dcX240` para o 240 e `PumpApi.dcX240_280_360`), com pontos em 0, 30, 50 e 65 °C. O aparelho não as informa. Para o X 240:

| Modo | Fan (RPM em 0/30/50/65 °C) | Bomba |
|---|---|---|
| 0 Balanced | 1500 / 1500 / 1800 / 1800 | 2600 / 2600 / 2600 / 3150 |
| 1 Customized | 2250 ×4 (valor inicial) | 2800 ×4 |
| 2 Default | 1200 / 1200 / 1400 / 1800 | — |
| 4 Turbo | 2500 ×4 | 3150 ×4 |
| 5 Performance | 2000 / 2000 / 2200 / 2500 | — |
| 6 Quiet | 900 / 900 / 1400 / 1600 | — |
| 7 ZeroRPM | 0 ×4 | — |

Para o X 280 e o X 360, o GCC usa o `FanApi.dcX280_360` na fan, com a mesma tabela da bomba. Ele só muda o Performance, que é 1800 / 1800 / 1800 / 2500. Os outros modos são iguais aos do 240. O GCC escolhe a tabela do 240 quando o nome do modelo contém "240". O `FanApi.dc240_280_360`, com Quiet em 950, é de outro produto e não é usado para o `0x7A4D`.

O `D9` lê o **slot Customized** guardado no aparelho, e não a curva ativa. Isso bate com o `DeviceApi.GetSpeedCurve`, que só chama o `D9` quando o modo é Customized. Foi **confirmado no aparelho** (change `waterforce-speed-mode`): o `D9` não mudou depois de nenhum preset, e o RPM seguiu o preset. As tabelas acima são só da interface: o firmware usa as dele, e o Default chegou a duty 255. O RPM medido em cada preset está em `docs/hardware.md`.

A tela da fan do GCC (`FanControl`) mostra a **temperatura da CPU** (`tb_CPUTemp`) ao lado da curva, em todos os modelos menos o 7A51. O valor vem do `DeviceApi.GetCPUTemperature`, que lê o `CpuInfo.ini` do `AorusLcdService` (ou `30`, se não houver). A tela não mostra a temperatura do líquido.

A temperatura de referência da curva **não é a do líquido**, o que foi **confirmado no aparelho** (change `waterforce-speed-curve`, `docs/hardware.md`). Com o líquido em 25–26 °C, a fan foi ao ponto de uma curva que só subia acima de 29 °C. A hipótese é a temperatura da CPU (`E0`/`E1`, enviados só pelo `AorusLcdService`), com um valor perto de 50 °C quando ninguém envia. Essa parte não está confirmada.

### Limites da curva Customized

A interface do GCC (`GFanCurve.Point_PointerMoved`) limita cada ponto arrastado:
- temperatura de 0 a 100 °C, estritamente crescente de um ponto para o seguinte;
- RPM sem queda de um ponto para o seguinte;
- RPM dentro da faixa do `DeviceApi.GetFanSpeedLimit` (900 a 2500 para o `0x7A4D`), aplicada à fan **e também à bomba**. O GCC não tem limite próprio para a bomba. O valor inicial da Customized da bomba (2800 ×4) só é gravado se o usuário não mexer no ponto.

O halo (`halo speed waterforce curve`) grava só curvas **planas**: o mesmo RPM em 0, 30, 50 e 80 °C, porque a referência da curva é a temperatura da CPU (ver acima). Esses pontos cumprem as regras de temperatura e de RPM sem queda. O halo usa a faixa do GCC só na fan. Na bomba, usa **de 2600 a 2800 RPM**, uma regra do halo, e não do GCC:
- 2600 é o menor RPM de qualquer modo pronto da bomba (Balanced, `PumpApi.dcX240_280_360`). Assim, a curva nunca deixa a bomba mais lenta do que os presets da Gigabyte.
- 2800 é o maior valor que se sabe que o GCC grava com o `E6` (o valor inicial da Customized, igual ao `D9 02` de fábrica). O Turbo (3150) é uma curva interna do firmware e nunca foi visto num `E6`.

A restauração de um snapshot não usa essas regras (nem exige curva plana): só a checagem de sanidade (4 pontos, temperatura crescente até 100 °C, RPM até o `MaxSpeed` de 3200), porque devolve um estado que já estava no aparelho.

Exemplo, a curva plana da fan de 1500 RPM com o `DD` em `05 04`: `99 E5 01 01 00`, `99 E5 02 04 00`, `99 E6 00 01 00 05 DC 1E 05 DC 32 05 DC 50 05 DC` e `99 B6`.

## LCD (320×320)

O LCD não recebe pixels crus. O GCC grava **arquivos** (JPEG, PNG ou MP4) na memória flash do aparelho, e o firmware os decodifica. O caminho do 7A4D é a classe `LcdApi` genérica. `LcdApi4XII`, `LcdApi4Elite` e `LcdApi4XIIElite` são de outros modelos.

### Modo e configuração

| Comando | Bytes | Fonte | Confiança |
|---|---|---|---|
| Ler modo | `99 E8` → `r[2] - 1` | `LcdApi.GetMode` | confirmado |
| Definir modo | `99 E7 <modo+1> <param>` (`param` é o logo no modo 4) | `LcdApi.SetMode` | confirmado |
| Itens exibidos | `99 E2 <b0>…<b8> <intervalo>`: nove bytes 0/1 (bits de `dwLcdDisplay`) e o intervalo em `[11]` | `LcdApi.SetDisplay` | confirmado |
| Ler itens exibidos | `99 DF` → `r[2..10]` | `LcdApi.GetDisplay` | confirmado |
| Cores das zonas | `99 E4 <modo+1> R1 G1 B1 R2 G2 B2` | `LcdApi.SetZoneColor`; cor `0x00RRGGBB` (`GvComApp.RGBUtil`) | confirmado |
| Ler cores | `99 EA <modo+1>` → `r[3..5]`, `r[6..8]` | `LcdApi.GetZoneColor` | confirmado |
| Ler logo | `99 E9` → `r[2]` | `LcdApi.GetGBTLogo` | confirmado |
| Carrossel de modos | `99 FB <intervalo> <modo+1>…` (27 bytes) | `LcdApi.SetLoop` | confirmado |
| Ler carrossel | `99 FC` → `r[2]` intervalo, `r[3…]` modos+1 até 0 | `LcdApi.GetLoop` | confirmado |
| Salvar | `99 B6` | `LcdApi.Save` | confirmado |

Números dos modos (índice de `GDisplayMode` na interface): de 0 a 2, informações de CPU; 3, temperatura; 4, logo; 5, funções; 6, texto; 7, imagem; 8, animação; 9, vídeo; 10, mascote; 11, carrossel. A correspondência vem das imagens de pré-visualização do `LcdControl.ShowMode` e é **inferida**.

### Dados de sistema (`AorusLcdService`)

Para o 7A4D, o serviço (`CoolerDataApi.GetCoolerData`) espera 20 s, manda o nome da CPU (`E1`) uma vez e depois manda o `E0` em laço, **sem `Sleep`**: o ritmo é o das leituras do `MakeCpuData` (WMI e `GbtCpuDll`). O intervalo de 1 s (`Sleep(1000)`) vale só para o 7A51, o X II Elite e os modelos com LCD de `IsLcdCmdCpuInfoPid`. Nesse ramo do 7A4D, o campo `LiqiudTemperature` nunca é preenchido, e o byte da temperatura do líquido vai como 0.

| Comando | Bytes | Confiança |
|---|---|---|
| Nome da CPU | `99 E1 <len> <ASCII…>` | confirmado |
| Dados | `99 E0 <fabricante: 1 Intel, 0 outro> <temp CPU> <threads> <GHz inteiro> <GHz décimo> <núcleos> <temp VRM> <temp líquido> <uso %> <W lo> <W hi>` | confirmado |

#### Matriz fonte/byte do E0 e do E1 (change `waterforce-cpu-feed`)

Conferida em 2026-10-27 contra `CoolerDataApi.cs` (`GetCoolerData`, linhas 230–257; `SendCpuName`, 318–344; `SendCoolerData`, 383–412; `GetCpuShortName`, 901–920) e `CoolerApi.cs` (`Send`, 42–56) em `extracted/ilspy-aoruslcdservice/AorusLcdService.API.Cooler/`, e contra os payloads de `experiment1.jsonl` e `experiment2.jsonl`. Nenhum envio novo ao aparelho.

| Byte | GCC (`SendCoolerData`, ramo 7A4D) | halo (design da change) | Enviado nos ensaios |
|---|---|---|---|
| 0 / 1 | `0x99` / `0xE0` | igual | `99 e0` |
| 2 | `CpuVendor`: 1 se o fabricante contém `Intel`, senão 0 | 1 para `GenuineIntel`, senão 0 | `00` |
| 3 | `Math.Round(CpuTemperature, 0)` (meio para o par, padrão do .NET), truncado em `byte` | Tctl arredondado para cima, saturado em 80; inválido = 80 | `50` / `32` / `1e` (80/50/30) |
| 4 | `CpuThreadCount` | threads online, até 255; sem dado = 0 | `0c` (12) |
| 5 / 6 | `CpuFrequency / 1000` e `/ 100 % 10` (MHz) | 0 / 0 | `00 00` |
| 7 | `CpuCoreCount` | núcleos únicos online, até 255; sem dado = 0 | `06` |
| 8 | `VRAMTemperature` (a tabela antiga chama de VRM) | 0, sem sensor atribuído | `00` |
| 9 | `LiqiudTemperature`, nunca preenchido nesse ramo = 0 | 0 | `00` |
| 10 | `CpuUsage` | 0 | `00` |
| 11 / 12 | `CpuPower % 256`, `CpuPower / 256` | 0 / 0 | `00 00` |
| 13… | zeros até `OutputReportLen` (`CoolerApi.Send`) | zeros até 6144 | zeros |

| Campo do E1 | GCC (`SendCpuName`) | halo (design da change) | Ensaio 1 |
|---|---|---|---|
| Bytes | `99 E1 <len> <nome>`; `len` = `string.Length` (caracteres), cada caractere truncado em `byte`, num buffer de 256 | igual, com `len` em bytes e caractere não ASCII trocado por `?` | `99 e1 0d` + `Ryzen 5 5600G` |
| Nome | `GetCpuShortName`: maiúsculas, sem `(R)`/`(TM)`, sem `@…`, `… GHz`, `…-Core`/`Threads`, `Processor…`; espaços colapsados; **até 30 caracteres** | nome de `/proc/cpuinfo`, até 30 bytes (sem as outras normalizações), fallback `CPU` | nome curto escrito à mão |
| Momento | uma vez, 20 s depois de iniciar o serviço, antes do laço de E0 | uma vez por sessão lógica, antes do primeiro E0 | 15 ms antes do primeiro E0 |

O que a matriz sustenta e o que não sustenta:

- **E1 obrigatório: não comprovado.** O GCC sempre manda E1 antes do laço, mas o código não mostra o firmware exigindo isso. O ensaio 2 não mandou E1 e a curva reagiu ao E0, só que o E1 do ensaio 1 continuava valendo, sem corte de energia entre os dois. A necessidade depois de inicialização limpa segue aberta.
- **Ausência de B6: confirmada no host.** `SendCpuName`, `SendCoolerData` e `CoolerApi.Send` não montam `B6`, e o `Send` só completa com zeros e chama `HidDriver.WriteByte(..., -1)`. Nenhum dos dois JSONL tem `b6`. Isso não prova que o firmware deixe de gravar o E0 na flash por conta própria. A leitura depois de um corte de energia pode limitar hipóteses, não provar a ausência de escrita interna.
- **Divergências deliberadas do halo:** arredondar para cima e saturar em 80 é mais conservador que o `Math.Round` do GCC. GHz, uso e W ficam em zero, o mesmo que os ensaios mandaram e que bastou para a curva reagir. O nome não passa pelas outras normalizações do `GetCpuShortName` (maiúsculas, cortes de sufixo), mas respeita o mesmo limite de 30 caracteres: o firmware nunca recebeu do GCC um nome maior.

### Arquivos na flash

| Comando | Bytes | Fonte | Confiança |
|---|---|---|---|
| Espaço livre | `99 FA <disco>` → `r[2]` = n, e o tamanho em `r[3..3+n]` little-endian, em KiB (o GCC divide por 1024 para mostrar MB) | `LcdApi.GetDiskFree`; disco `'B'` (0x42) para imagens e `'E'` (0x45) para vídeo no 7A4D (`LcdApi.GetDisk`) | confirmado; a unidade é inferida |
| Lista de mídia | `99 F3 <modo-6>`, espera 500 ms, `99 F4 <modo-6>` → `r[3]` = quantidade; por item, `99 F5 <modo-6> <i>` → `r[4]` em loop, nome UTF-8 a partir de `r[6]` | `LcdApi.GetMediaList` | confirmado |
| Ordem da lista | `99 F0 <modo+1> <índices…>` | `LcdApi.SetMedliaList` | confirmado |
| Itens em loop | `99 F6 <modo+1> <intervalo> <índices…>`; leitura `99 FD <modo+1>` | `LcdApi.SetMediaLoopIndex` e `GetMediaLoopIndex` | confirmado |
| Apagar arquivo | `99 FE <len> <nome UTF-8>` | `LcdApi.RemoveFile` | confirmado |

**Envio de arquivo** (`LcdApi.SendImage`), com o tamanho `S` em 32 bits big-endian (`ByteUtil.TopByte/High/Midd/Low`) e `N = OutputReportLen - 3` (6141 bytes por pedaço):

1. Início: `99 F1 01 S3 S2 S1 S0 <len(nome)+1> <nome UTF-8>`. Depois, 10 ms de espera.
2. Para cada pedaço completo: `99 F2 FD <N bytes do arquivo>`, com 1 ms entre os pedaços.
3. Se sobrar um resto `R = S mod N`: `99 F2 <R & 0xFF> <R bytes>`. O GCC trunca `R` em um byte, e o firmware deve usar o tamanho do início.
4. Fim: `99 F1 <tipo> S3 S2 S1 S0`, com tipo 2 para imagem ou animação, 3 para texto e 4 para vídeo.
5. Com firmware ≥ 2.0, o GCC lê o progresso com `99 C2` até 100 %. Depois manda `99 E7 …` com o modo e `99 B6`.

Preparação do arquivo no GCC (`ImageMaker`, `GCutImage`):
- Imagem: recorte e redimensionamento para **320×320** e gravação em **JPEG**.
- Texto: PNG de 320×320.
- GIF: convertido para MP4 com o ffmpeg (`-pix_fmt yuv420p -profile:v main -level 2.1 -b:v 1000K -bf 0`).
- Vídeo: H.264 (`libx264`), recortado em 320×320, sem áudio.
- O nome tem no máximo 20 caracteres, em minúsculas e com `_` no lugar de espaço, e o arquivo tem no máximo 10 MiB.

O formato que o firmware aceita é **inferido** dessa preparação. O código não mostra uma validação no aparelho.

## Anel e RGB das fans

No GCC, o LED do 7A4D passa pela DLL nativa `GcLedLib.dll`: `LedApi.LedSet` e `LedSave` chamam `dllexp_GvAIOV2LedSet(0x4016, Led_Cfg)`. As duas usam a mesma exportação, então toda aplicação termina com `B6`. Analisei a DLL com o Ghidra 12.1.4 pela CLI do REA (`rea decompile` e `rea instructions`, `--provider ghidra`). Os argumentos passados pela pilha foram conferidos no assembly.

### Entrada: `Led_Cfg` (C#)

`LedApi.LedSettingToConfig` monta a estrutura (todos os campos são `int`/`uint` de 4 bytes):

| Offset | Campo | Valor vindo da interface |
|---|---|---|
| 0x00 | `nType` | ID do efeito (ver tabela abaixo) |
| 0x04 | `nSpeed` | velocidade de 1 a 5 (padrão 5, `uc_Speed` no BAML do `LedControl`) |
| 0x08 | `nBright` | brilho de 1 a 10 × 10, ou seja, de 10 a 100 (padrão 100) |
| 0x0C | `nClrIndex` | 0 |
| 0x10 | `nGlobalClr` | 1 (o `SettingApply` só aplica com `GlobalClr`) |
| 0x14 | `nArea` | 1 |
| 0x18 | `lGlobalClr` | cor `0x00RRGGBB` |
| 0x28 | `lFan1Clr[8]` | paleta de até 8 cores `0x00RRGGBB` |

"Desligado" (`On = false`) vai como `nType = 1` e `nBright = 0`.

### Ajustes na DLL para `0x4016`

`dllexp_GvAIOV2LedSet` (`0x18001c410`) acha o dispositivo pelo ID e, para `0x4016`, faz o seguinte antes de chamar `FUN_1800402a0`:
- `nSpeed = nSpeed << 1` (e vira 1 se o resultado for 0), ou seja, de 2 a 10;
- `nClrIndex = 2`.

### Pacotes (todos com report ID `0x99`, 256 bytes completados até `OutputReportLen`)

| Pacote | Bytes | Função na DLL | Confiança |
|---|---|---|---|
| Efeito | `99 C9 <nType> <nBright> <nSpeed> <nClrIndex> <nArea>` | `FUN_180040fc0` | confirmado (assembly: `EDX`, `R8`, `R9`, `[RSP+0x20]`, `[RSP+0x28]` → bytes 2 a 6) e no aparelho |
| Cor única | `99 CD R G B` (de `lGlobalClr`) | `FUN_180041470` | confirmado e no aparelho |
| Paleta | `99 B0 <nType> R0 G0 B0 R1 G1 B1`, `99 B1 <nType> c2 c3`, `99 B2 <nType> c4 c5`, `99 B3 <nType> c6 c7` (de `lFan1Clr`), com 1 ms entre eles | `FUN_1800410d0` | confirmado e no aparelho |
| Aplicar | `99 B6` | `FUN_180040780` | confirmado e no aparelho |

O write (`FUN_18003a500`) copia os 256 bytes para um buffer do tamanho do output report, completado com zeros. É o mesmo transporte do C#.

### Sequência por efeito (`FUN_1800402a0`)

`<b>` = `nBright` (10 a 100), `<s>` = `nSpeed` já dobrado (2 a 10). Com o C# do GCC, `<idx>` = 2 e `<área>` = 1, salvo onde a DLL troca esses valores.

| `nType` | Efeito | Pacotes enviados |
|---|---|---|
| 1, 2, 4, 5 | Static, Pulse, Flash, Double Flash | `C9 t b s 02 01`, `CD R G B`, `B6` (com `nGlobalClr = 0`, a DLL manda `B0`–`B3` em vez do `CD`) |
| 3, 8, 10 | Cycle, Wave, Rainbow | `C9 t b s 02 01`, `B6` |
| 6 | Gradient | `C9 t b s 02 01`, `CD R G B`, `C9 t b s 0A 01`, `B6` |
| 7 | Color Shift | `C9 07 b s 08 02`, `B0`–`B3`, `B6` |
| 9 | (não aparece na interface) | `C9 09 b s 02 02`, `B0`–`B3` se `nGlobalClr`, `B6` |
| 11 a 15 | Tri-Color, Spin, Switch, Lego, Aurora | `C9 t b s 02 01`, `B0`–`B3`, `B6` |
| 0 ou > 15 | — | só `B6` |

A interface do X 240 (`GLed_0x4016_240.LedEffectId`) oferece 1, 2, 4, 5, 3, 6, 7, 8, 10, 11, 12, 13, 14, 15 e 0. Os nomes vêm de `LedEffectConvert.Cooler.ToName`.

### Comparação com o OpenRGB

O driver `GigabyteCastor3Controller.cpp` (`72262b8`, para o X II 360 `0414:7a5e`) usa os mesmos comandos, `C9`, `CD`, `B0`–`B3` e `B6`, com os mesmos números de estilo. Isso faz sentido: no GCC, o X e o X II compartilham o ID de LED `0x4016` e passam pela mesma função da DLL.

| Campo | OpenRGB | GCC (`GcLedLib`) |
|---|---|---|
| `C9` byte 2 | "velocidade × 20" (20 a 100) | brilho × 10 (10 a 100) |
| `C9` byte 3 | "brilho" (1 a 10) | velocidade × 2 (2 a 10) |
| `C9` bytes 4 e 5 | `02 01` fixos | `nClrIndex`, `nArea`: `02 01`, salvo nos efeitos 6, 7 e 9 |
| `B0`–`B3` | `bN <estilo> RGB RGB 00` | igual (o 9º byte é zero do preenchimento) |

O OpenRGB monta o `C9` a partir de uma captura de USB e **troca os nomes dos bytes 2 e 3**. Nos valores padrão (velocidade 5, brilho 10), os dois geram `100, 10`, por isso a troca não aparece. **O halo deve seguir o GCC**: byte 2 = brilho, byte 3 = velocidade.

### RGB das fans

O anel e as fans usam **o mesmo canal**. Para o `0x4016`, o GCC tem uma única região (`SettingIndex2Region = {0: 0}`) e manda uma configuração global com `nArea = 1`. Ele não manda nada separado para as fans, ao contrário do 7A51 (`0x4015`), que envia a área 1 (fan) e a área 2 (bomba). As fans ficam numa cadeia controlada pelo Castor3, e o número delas é definido à parte:

| Comando | Bytes | Fonte | Confiança |
|---|---|---|---|
| Quantidade de fans RGB | `99 C8 <n>` (de 1 a 8; padrão da interface 3), leitura com `99 C7` | `DeviceApi.SetFanQty` e `GetFanQty`; o controle só aparece no 7A4D | confirmado |

Que o anel e as fans acendam juntos com o mesmo `C9` era **inferido** do fato de o GCC não ter outro comando. Isso foi **confirmado no aparelho** na change `waterforce-led`: com `C9 01 64 0A 02 01`, `CD FF 66 00` e `B6`, o anel e as duas fans ficaram laranja juntos.

### Conferência no aparelho (2026-10-20)

Firmware 1.6, Waterforce X 240, com o `halo led waterforce` (`DE 00` antes de cada sequência):

| Efeito | Pacotes (sem o `DE 00`) | Resultado |
|---|---|---|
| `static ff6600` | `C9 01 64 0A 02 01`, `CD FF 66 00`, `B6` | anel e fans laranja, juntos |
| `cycle` | `C9 03 64 0A 02 01`, `B6` | igual ao modo de fábrica |
| `rainbow` | `C9 0A 64 0A 02 01`, `B6` | diferente do `cycle` |
| `tri-color ff0000 00ff00 0000ff` | `C9 0B 64 0A 02 01`, `B0 0B FF0000 00FF00`, `B1`–`B3 0B 0000FF 0000FF`, `B6` | vermelho, verde e azul |
| `pulse --speed 2` | `C9 02 64 04 02 01`, `CD FF 66 00`, `B6` | laranja pulsando devagar |

Isso confirma o layout do GCC: o byte 3 do `C9` (`04`) mudou o ritmo, e o byte 2 (`64`) ficou como brilho.

Em 2026-10-23, pelo painel do GNOME (`Led.Apply` do `halod`, mesmos pacotes), o `flash` (`04`), o `gradient` (`06`) e o desligado (estático com brilho 0 e `CD 00 00 00`) foram vistos no aparelho pela primeira vez e responderam como esperado, com um único `B6` cada (ver [hardware.md](../hardware.md), "LED pelo painel do GNOME"). Ainda não vistos: `05`, `07`, `08` e `0C` a `0F`.

### Não analisado

- `dllexp_GvSetRealTimeColor`, usado pela sincronização com outros dispositivos do GCC, despacha por um método virtual da classe do dispositivo. Não é necessário para cor fixa e efeitos.
- Se o `B6` grava na flash a cada chamada. O GCC manda `B6` em toda aplicação de LED, de velocidade e de LCD. Na velocidade, ele é o que faz o modo sobreviver ao desligamento (confirmado no aparelho), mas o desgaste por chamada não é conhecido. Por prudência, o halo não deve enviá-lo em laço.

## Reprodução da análise nativa

```sh
export GHIDRA_INSTALL_DIR=/home/bumblebee/.local/opt/ghidra_12.1.4_PUBLIC
export JAVA_HOME=/home/bumblebee/.local/opt/jdk-21.0.12.1+1
cd /mnt/workspace/tools/repos/rea
node scripts/rea.mjs decompile <GcLedLib.dll> dllexp_GvAIOV2LedSet --provider ghidra --snapshot <snapshot>
node scripts/rea.mjs instructions <GcLedLib.dll> FUN_1800402a0 --provider ghidra --snapshot <snapshot>
```

Funções analisadas: `dllexp_GvAIOV2LedSet`, `FUN_1800402a0`, `FUN_180040fc0`, `FUN_1800410d0`, `FUN_180041470`, `FUN_180040780`, `FUN_18003a500` e `dllexp_GvSetRealTimeColor` (parcial). O snapshot do Ghidra está em `tools/samples/gigabyte-control-center/analysis/gcledlib.snapshot`.
