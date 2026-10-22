# Arquitetura proposta

```text
Barra do GNOME:  [ícone halo]   extensão GNOME Shell (GJS)
                      │          painel: cores, zonas, brilho, efeitos, perfis
                      │ D-Bus (sessão)
                      ▼
               haloD (Rust)      daemon do usuário (systemd --user)
                      │          um módulo por dispositivo, perfis, reaplica no boot
          ┌───────────┼──────────────┬──────────────┐
          ▼           ▼              ▼              ▼
     IT5702 (HID)  Castor3 (HID)  G502 (HID)   RAM (SMBus, depois)
```

## Por que uma extensão do GNOME

O estilo desejado é de ícones discretos na barra superior que abrem um painel pequeno ao clicar. No GNOME com Wayland, um app comum (Tauri, GTK) não consegue isso: o ícone de bandeja (AppIndicator) só abre um menu de texto simples, e o Wayland não deixa o app posicionar uma janela abaixo do ícone. A extensão GNOME Shell é o caminho nativo e herda o tema do usuário.

Se for preciso uma tela de edição maior (desenhar efeitos por LED, por exemplo), ela pode vir depois como um app GTK4/libadwaita que usa o mesmo D-Bus.

## Daemon (`halod`, change `halo-daemon`)

- Crate `crates/halo-daemon`, binário `halod`. Fala com os aparelhos só pelo `halo-core`, sem lógica de protocolo própria, e encontra cada um pelo VID:PID e pela usage page a cada operação.
- D-Bus da sessão com o `zbus` 5 (executor próprio, sem tokio). Nome `io.github.leoodaviid.Halo1`, pedido sem `AllowReplacement` e sem `ReplaceExisting`: uma segunda instância recusa e sai, e não toma o nome da primeira.
- Não precisa de root. O Waterforce precisa da regra udev do halo, como na CLI.

```text
/io/github/leoodaviid/Halo1
  .Manager    prop Version; Devices() -> a(sbs); signal DevicesChanged
  .Profiles   List, Get, Save, Delete, Activate; prop ActiveProfile
/io/github/leoodaviid/Halo1/waterforce
  .Waterforce.Led        Apply(s, as, y, y); prop LastApplied
  .Waterforce.Speed      Status, SetPresets(s, s), SetFlatCurve(s, q), Restore(s)
  .Waterforce.Telemetry  props Available, FanRpm, PumpRpm, FanDuty, PumpDuty,
                         CoolantMilliCelsius, CoolantValid
```

- **Um worker por aparelho.** Uma thread é dona das chamadas bloqueantes ao core do Waterforce e atende um pedido de cada vez, na ordem de chegada. Os métodos D-Bus esperam a resposta por um canal, então o executor do zbus não bloqueia com as esperas de resposta do Castor3, e lista, perfis e telemetria continuam respondendo.
- **Ticks.** A cada 2 s, o daemon lê só o sysfs: a presença dos aparelhos (com `DevicesChanged` quando muda) e a telemetria do hwmon (com `PropertiesChanged` só nas propriedades que mudaram). No fim de uma suspensão (`PrepareForSleep(false)` do logind, no D-Bus do sistema), roda os ticks na hora. Nenhum tick abre hidraw: o `waterforce-cpu-feed` vai acrescentar um que envia o `E0` pelo worker, e o `systemd-user-service` vai decidir o que reaplicar no login e no resume.
- **Perfis.** TOML em `$XDG_CONFIG_HOME/halo/profiles/`, validados pelos tipos do core e gravados de forma atômica. A ativação da velocidade é um `DesiredState` do core: lê o estado e só escreve (com `B6`) se ele for diferente, com a mesma sequência do `set` e do `curve`.

## Acesso exclusivo ao hidraw

O Castor3 responde por pedido e resposta: o halo envia `99 <cmd>` e espera um input report que começa com o mesmo comando. Se dois processos conversarem com ele ao mesmo tempo, um pode pegar a resposta do outro. Por isso, `Hidraw::open` (`halo-core`) toma um `flock(LOCK_EX)` no próprio nó `/dev/hidrawN` e espera até 5 s por ele.

- O lock fica no nó do aparelho, e não num arquivo à parte: dois processos que abrem o mesmo `/dev/hidrawN` usam o mesmo inode, então não é preciso um nome de lock por aparelho, e o número do hidraw pode mudar entre boots sem problema, porque o lock é tomado depois da descoberta por VID:PID.
- Ficar em `Hidraw::open` cobre todo comando que abre o hidraw (probes, LED e velocidade), na CLI e no daemon.
- O lock é solto quando o descritor fecha, inclusive se o processo morrer.
- É um lock advisory: só vale entre processos do halo. O driver `gigabyte_waterforce` do kernel não usa o hidraw.

## Regras de segurança

- Só enviar comandos que já estejam documentados (OpenRGB ou análise com o REA). Nada de testar bytes desconhecidos no hardware.
- Começar todo dispositivo novo com uma ferramenta que só lê.
- RAM/SMBus fica fora até haver uma decisão explícita sobre `acpi_enforce_resources=lax`.
