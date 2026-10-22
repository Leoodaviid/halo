# Design

## Context

Ver `proposal.md` (Why). Estado atual no `halod`:

- `bus.rs`: `Manager` guarda `devices: Vec<DeviceEntry>`, com `DeviceEntry = (String, bool, String)`. `Devices()` devolve essa lista, e o sinal `DevicesChanged` a carrega.
- `ticks.rs`: `scan_devices(sys_root)` percorre uma tabela fixa `DEVICES` (`it5702`, `waterforce`) com `find_devices` e um `DeviceMatch` cada. O `run_once` compara a lista com a anterior e só emite o sinal quando ela muda.
- `worker.rs`: o `CoreBackend` chama o `halo-core`. Todas as operações do Waterforce já leem o modelo com `99 DE 00` antes de escrever (`led::apply_to`, `speed::read_state`) e o devolvem: `led::Applied.model`, `SpeedStatus.state.model`, `speed::Applied.written.before.model` e o `Snapshot` de `StateOutcome`.
- `main.rs`: um canal `refresh` (capacidade 1) acorda os ticks fora do ritmo. Hoje ele é usado no fim de uma suspensão.

O G502 tem duas interfaces no sysfs: `hidraw0`, usage page `0x0001` (mouse comum), e `hidraw1`, com a coleção de topo na usage page `0xFF00` (lido do `report_descriptor`, sem abrir o hidraw). O OpenRGB registra o G502 HERO nessa interface: `Controllers/LogitechController/LogitechControllerDetect.cpp`, `REGISTER_HID_DETECTOR_IPU("Logitech G502 HERO Gaming Mouse", DetectLogitechWired, LOGITECH_VID, LOGITECH_G502_HERO_PID, 1, 0xFF00, 2)` (commit `72262b8`). Esta change não envia nenhum byte ao mouse: só a combinação VID:PID e usage page vem do OpenRGB.

## Goals / Non-Goals

**Goals:**
- Uma lista de aparelhos que a extensão possa desenhar sem catálogo próprio e que não precise mudar de forma quando um aparelho ganhar uma capacidade.
- O nome exato do Waterforce sem nenhum tráfego novo no Castor3.

**Non-Goals:**
- Guardar o modelo entre execuções do daemon. Depois de um restart, o nome volta a `AORUS Waterforce X` até a primeira operação.
- Objetos D-Bus por aparelho ou `ObjectManager`.
- Abrir o hidraw do G502 ou da placa.

## Decisions

### 1. `aa{sv}` com chaves fixas

Cada aparelho é um `a{sv}` com estas chaves, sempre presentes e sempre com o mesmo tipo:

| Chave | Tipo | Valor |
|---|---|---|
| `id` | `s` | `waterforce`, `it5702` ou `g502` |
| `name` | `s` | nome para mostrar (decisões 3 e 4) |
| `present` | `b` | encontrado no sysfs |
| `devnode` | `s` | `/dev/hidrawN`, ou `""` quando ausente |
| `capabilities` | `as` | as interfaces D-Bus que o daemon expõe para o aparelho |

Dentro do daemon, a lista é um `Vec<Device>` com um `struct Device` comum, `PartialEq` para a comparação do tick, convertido para `Vec<HashMap<String, OwnedValue>>` só na borda do D-Bus, como o `LastApplied` já faz. A ordem da lista é a da tabela (decisão 2): `waterforce`, `it5702`, `g502`. É a ordem em que o painel mostra os aparelhos.

Alternativas: uma tupla maior `a(ssbsas)`, que muda de assinatura a cada campo novo, e um objeto por aparelho com `ObjectManager`, mais idiomático, mas com muito mais código para três aparelhos fixos. As duas foram descartadas na exploração.

### 2. Tabela de aparelhos com capacidades e origem do nome

A tabela fixa de `ticks.rs` ganha o que cada aparelho precisa:

```text
id          DeviceMatch                       capabilities                 nome
waterforce  1044:7a4d, page 0x0000            led, speed, telemetry        modelo (decisão 4)
it5702      048d:5702, page 0xFF89            (nenhuma)                    DMI (decisão 3)
g502        046d:c08b, page 0xFF00            (nenhuma)                    "Logitech G502 SE HERO"
```

As capacidades são constantes por aparelho: dizem o que o daemon sabe fazer, e não se o aparelho está presente nesse instante. Elas usam os nomes das interfaces D-Bus (`Waterforce.Led` vira `led`, e assim por diante). A `it5702-static-color` só acrescenta `led` na linha da placa.

O `DeviceMatch` do G502 fica no `halo-core`, num módulo `g502` só com as constantes (`VENDOR_ID`, `PRODUCT_ID`, `USAGE_PAGE` e `DEVICE`), como os de `it5702` e `waterforce`. A `g502-read-probe` vai construir em cima dele.

### 3. Nome da placa pelo DMI

O tick lê `<sys_root>/class/dmi/id/board_name`, sem o `\n` do fim. É a mesma raiz de sysfs dos testes, então um sysfs falso controla o valor. Se o arquivo faltar ou vier vazio, o nome é `Placa-mãe (IT5702)`. A leitura é de um arquivo pequeno a cada 2 s, no mesmo ritmo da presença.

### 4. Modelo do Waterforce pelo worker, sinal pelo canal `refresh`

```text
 worker (thread)                         executor do zbus
 ---------------                         -----------------
 backend.apply_led()  -> Applied.model
 backend.speed_*()    -> Snapshot.model
        |
        v
 KnownModel (Arc<Mutex<Option<u8>>>) ---> tick lê ao montar a lista
        |
        +-- refresh.try_send(()) -------> run_once agora: nome mudou ->
                                          DevicesChanged
```

- O `WaterforceBackend` passa a devolver o modelo nas operações que o leem: `apply_led` devolve o `u8`, e `speed_status`, `set_presets`, `set_curve`, `restore` e `apply_state` já devolvem ou passam a devolver o `Snapshot` de onde ele sai. A troca é interna ao daemon, e o `halo-core` não muda.
- O worker grava o modelo num `KnownModel` compartilhado, só quando a operação deu certo e o modelo está na tabela de `model_name`. Um modelo desconhecido não muda o nome, e a operação falha como hoje.
- Depois de gravar, o worker manda `try_send(())` no canal `refresh` que os ticks já escutam. Com o canal cheio, o tick já vai rodar, então perder o envio não é problema. O `run_once` monta o nome com `AORUS ` mais o `model_name(m)` e, como o nome mudou, emite `DevicesChanged`. Assim o sinal sai do executor do zbus, onde ele já sai hoje, e não da thread do worker.
- Sem modelo conhecido, o nome é `AORUS Waterforce X`.

Alternativa: o worker atualizar o `Manager` e emitir o sinal ele mesmo. Foi descartada porque o worker é uma thread bloqueante, sem acesso ao executor. Reaproveitar o `refresh` mantém um só lugar que monta a lista e emite o sinal.

### 5. O que fica na CLI

A CLI não usa o `Devices()`. Só o exemplo de `busctl` no README muda, para a saída nova. Os testes de integração que leem a lista (`bus.rs`) passam a ler o dicionário.

## Risks / Trade-offs

- [Quebra de assinatura do `Devices()` e do `DevicesChanged`] → Não há cliente além dos testes e do README. A extensão nasce com o formato novo, e chaves novas no futuro não quebram nada.
- [O nome do Waterforce muda depois da primeira ação] → Acontece uma vez por execução do daemon e vem com `DevicesChanged`. O painel mantém a página e só troca o texto (cenário "Nome muda" na `gnome-panel`).
- [Outro mouse Logitech com a mesma usage page] → A busca é por VID:PID e usage page, então só o `046d:c08b` combina. Um receptor sem fio tem outro PID.
- [Ler o `board_name` a cada tick] → É um arquivo de sysfs pequeno, que não muda. Se o custo aparecer, dá para ler só uma vez na inicialização, sem mudar o comportamento.
- [Capacidades que não refletem o aparelho ausente] → É de propósito: `present` diz se está conectado, e `capabilities` diz o que o daemon sabe fazer com ele.

## Migration Plan

Nada a migrar: não há dados persistidos, e os clientes são os testes e o README. Para voltar atrás, basta reverter o commit.
