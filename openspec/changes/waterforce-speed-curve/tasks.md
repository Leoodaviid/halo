# Tasks

## 1. Pré-requisito e montagem da curva (sem aparelho)

- [ ] 1.1 Conferir que a `waterforce-speed-mode` está aplicada. Anotar na seção 3 do `design.md` a API que ela expõe para o envio do `E5`, o snapshot e o restore, e a resposta sobre o `E5` valer sem o `B6`. Conferir que os requisitos "Comandos permitidos" e "Snapshot validado antes de restaurar" da spec `waterforce-speed` têm os nomes usados no MODIFIED desta change. Rever as Decisões 3 e 5 se a resposta contrariar o que está escrito. Verificar que `cargo test` passa na `main` com a change aplicada
- [ ] 1.2 Criar `waterforce/curve.rs` com o tipo `Curve` (4 `CurvePoint`, tipo fan ou bomba), a validação da spec e a função pura que, a partir do modo cru do outro lado, gera os dois `E5` (`DeviceApi.SetSpeedMode`, lado da curva em `01`, recusa com 3 ou maior que 7), `E6 00 <tipo> …` (`DeviceApi.SetSpeedCurve`) e `B6` (`DeviceApi.Save`). A validação cobre 4 pontos, temperatura de 0 a 100 °C estritamente crescente, RPM sem queda, fan de 900 a 2500 e bomba de 2600 a 2800. Verificar com testes unitários que comparam byte a byte os pacotes dos cenários "Curva da fan", "Curva da bomba" e "RPM em big-endian" da spec, e cobrem cada caso de recusa, inclusive "Outro lado em FixedRPM"
- [ ] 1.3 Documentar no `docs/protocols/castor3.md` a faixa do halo para a bomba (2600 a 2800, regra do halo, e não do GCC) e o limite que o `GFanCurve` aplica aos dois tipos. Verificar que a tabela "Velocidade da fan e da bomba" continua batendo com os pacotes dos testes da 1.2

## 2. Envio e CLI

- [ ] 2.1 Implementar a gravação: `DE 00` com `r[2] ∈ {0, 1, 2}`, o snapshot da `waterforce-speed-mode` (`DD`, `D9 01`, `D9 02`) e só então os dois `E5`, o `E6` e um `B6`, usando o envio da `waterforce-speed-mode`. Verificar com testes que usam um falso para mostrar cinco coisas: com modelo desconhecido, falha no snapshot ou o outro lado em FixedRPM, nenhum `E5`/`E6`/`B6` sai; com tudo certo, sai a sequência exata; com uma curva da bomba, nenhum `E6` de tipo `01` sai; há um único `B6` por execução; e a restauração de um snapshot com a fan em Customized a 800 RPM não é recusada pelas faixas da curva
- [ ] 2.2 Adicionar o subcomando `halo speed waterforce curve <fan|pump> <temp:rpm> ×4 [--dry-run]`, com a validação antes de procurar o aparelho e os códigos de saída do comando de LED. Verificar com testes de integração: `--dry-run` sem sysfs, curva inválida sem sysfs, aparelho não encontrado e sem permissão com sysfs falso
- [ ] 2.3 Documentar o comando no `README.md`, com a curva da fan, a curva da bomba, o `--dry-run` e o "voltar ao anterior", e explicar a faixa da bomba. Verificar que cada exemplo roda com `--dry-run`

## 3. Temperatura de referência (bloqueia o grupo 4)

- [ ] 3.1 Com o aval do usuário, ler `T` no `halo telemetry waterforce` e gravar na fan a curva do experimento `0:1000, T+1:1000, T+3:2500, 100:2500`. Comandos enviados: `99 DE 00`; `99 DD`, `99 D9 01` e `99 D9 02` (snapshot); `99 E5 01 01 00` e `99 E5 02 <modo lido da bomba> 00` (`DeviceApi.SetSpeedMode`); `99 E6 00 01 …` (`DeviceApi.SetSpeedCurve`); e `99 B6` (`DeviceApi.Save`), este só se a 1.1 mostrar que o `B6` é necessário. Todos vêm do `ucCooler.dll` do GCC 26.09.10.01. Verificar com `halo probe waterforce --raw` que o `DD` mostra a fan em Customized (1) e que o `D9 01` devolve a curva enviada
- [ ] 3.2 Com o `halo telemetry waterforce --watch` aberto, verificar com o usuário se a fan fica perto de 1000 RPM em repouso e sobe para perto de 2500 quando o `temp1_input` passa de `T+3` sob carga da CPU. Registrar o resultado: líquido ou CPU, e o comportamento acima do último ponto
- [ ] 3.3 Com o aval do usuário, usar o "voltar ao anterior" da `waterforce-speed-mode` (comandos da própria change: `E5`, `E6` com as curvas do snapshot e `B6`). Verificar com `halo probe waterforce` que o modo e as duas curvas voltaram ao snapshot e que o RPM voltou ao de antes no `--watch`
- [ ] 3.4 Atualizar o `docs/hardware.md` com o resultado do experimento: a temperatura de referência, o `D9` confirmado (ou não) como slot Customized e o `E6` aceito pelo firmware. Marcar no `docs/protocols/castor3.md` o `E6` e a referência como confirmados no aparelho. **Se a referência for a CPU, parar aqui** e reescopar com o usuário, conforme a Decisão 1 do `design.md`. Verificar que nenhum trecho dos dois documentos continua chamando a referência de "inferida"

## 4. Bomba e regras finais

- [ ] 4.1 Com o aval do usuário, gravar na bomba a curva plana `0:2600, 30:2600, 50:2600, 80:2600`. Comandos enviados: `99 DE 00`, snapshot (`DD`, `D9 01`, `D9 02`), `99 E5 01 <modo lido da fan> 00`, `99 E5 02 01 00`, `99 E6 00 02 00 0A 28 1E 0A 28 32 0A 28 50 0A 28` e `99 B6` (`DeviceApi`, `ucCooler.dll`). Verificar no `--watch` que a bomba se estabiliza perto de 2600 RPM e com o `D9 02` que a curva foi guardada. Depois, usar o "voltar ao anterior" e conferir o Turbo de volta
- [ ] 4.2 Decidir com o usuário as regras extras contra uma "curva perigosa" (Open Questions do `design.md`), com os dados das tasks 3.2 e 4.1. Se alguma for adotada, adicionar o requisito à spec da change, a validação na `curve.rs` e o teste unitário correspondente. Se nenhuma for adotada, registrar a decisão no `design.md`. Verificar com `cargo test`
- [ ] 4.3 Atualizar o `docs/hardware.md` com o RPM real da bomba na Customized e o roadmap com a curva concluída. Verificar com `cargo fmt --check`, `cargo clippy` e `cargo test` sem erros

## Workflow follow-up

- Sincronizar a spec `waterforce-speed` (depois do archive da `waterforce-speed-mode`), arquivar a change e commitar, quando o usuário pedir.
