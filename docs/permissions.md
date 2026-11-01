# Permissões

O halo não roda como root. Para falar com um controlador, o usuário da sessão precisa ter leitura e escrita no `/dev/hidrawN` dele. A regra [`packaging/udev/70-halo.rules`](../packaging/udev/70-halo.rules) dá a tag `uaccess` só aos hidraw que o halo usa, e o Castor3 ganha também o grupo `plugdev`:

| Dispositivo | VID:PID | Acesso |
|---|---|---|
| Placa-mãe, ITE IT5702 | `048d:5702` | `uaccess` (ACL da sessão ativa) |
| AORUS Waterforce, Castor3 | `1044:7a4d` | `uaccess` e `GROUP="plugdev"`, `MODE="0660"` |

O `uaccess` faz o logind pôr na ACL do nó o usuário da sessão ativa do assento. Na tela de login, a sessão ativa é a do greeter, e essa entrada sai. O grupo `plugdev` continua valendo sem sessão, e é por ele que o `halod`, que roda com o linger (ver o README), continua mandando a CPU ao X 240 na tela de login e no boot. Só o Castor3 tem o grupo: o halo ainda não escreve no IT5702. Com a regra nova, qualquer processo do grupo `plugdev` abre o Castor3, mesmo fora da sessão; num PC de um usuário, isso equivale ao acesso de antes, e o `flock` do halo continua serializando quem fala com o aparelho.

Ela não inclui o teclado Redragon nem barramentos I2C/SMBus. A telemetria do Waterforce (`halo telemetry waterforce`) não precisa dessa regra, porque vem do hwmon do kernel, que qualquer usuário pode ler.

## Instalar

```sh
sudo install -m 644 packaging/udev/70-halo.rules /etc/udev/rules.d/
sudo udevadm control --reload
sudo udevadm trigger --subsystem-match=hidraw --action=change
```

Depois, **reinicie o PC**. O `trigger` aplica o `uaccess` na hora, mas o grupo do Castor3 só vale num nó criado de novo (boot ou replug). O nó já tem a ACL estendida que o logind põe pela sessão, e, num arquivo com ACL, o `MODE="0660"` vira um `chmod` que muda só a máscara (`mask::rw-`): a entrada do grupo continua `group::---`, e o `halod` continua com acesso só pela ACL. Visto em 2026-10-29: depois do `trigger`, o nó era `crw-rw----+ root plugdev` com `group::---`.

O `packaging/install.sh` (ver o README) não instala a regra, porque precisa de sudo. Ele só compara a regra instalada com a do repositório e, se ela faltar ou estiver diferente (uma regra antiga, só com `uaccess`, aparece como desatualizada), mostra estes mesmos comandos.

O grupo e o linger também são só conferidos pelo script. Se faltarem, ele mostra os comandos:

```sh
sudo usermod -aG plugdev "$(id -un)"   # vale depois de sair e entrar na sessão
loginctl enable-linger "$(id -un)"
```

Sem o grupo, o `halod` não alcança o Castor3 fora da sessão e, no logout, manda o `E0` de 80 °C e pausa a CPU até o próximo login (ver "Guarda da sessão gráfica" em [architecture.md](architecture.md)). Sem o linger, o `halod` para no logout, como antes.

Sem o grupo, a **troca de usuário** ("Trocar de usuário" com a sessão aberta) deixa o Castor3 sem proteção. O logind passa a ACL do nó para a outra sessão, mas a sessão gráfica do primeiro usuário continua, então não há `End()`. Os envios do `halod` passam a falhar, o watchdog o mata e o `halod --waterforce-cpu-safe` também recebe acesso negado: o aparelho fica no último Tctl, que pode ser frio. Reagir à troca de sessão não resolve, porque o envio pode chegar depois de a ACL sair (a mesma corrida que levou à guarda). Com o grupo `plugdev` e a regra atual, o acesso não depende de qual sessão está ativa, e a troca de usuário não muda nada.

## Verificar

```sh
for d in /sys/class/hidraw/*; do
  grep -qE 'HID_ID=0003:0000(048D:00005702|1044:00007A4D)' "$d/device/uevent" &&
    ls -l "/dev/$(basename "$d")" && getfacl -p "/dev/$(basename "$d")"
done
```

Com a sessão ativa, cada hidraw listado tem uma linha `user:<seu usuário>:rw-`. O do Castor3 é `crw-rw----+ root plugdev`, com `group::rw-` e `mask::rw-`. Com `group::---`, o nó é anterior à regra: reinicie o PC (ver "Instalar"). Sem sessão ativa (rode por SSH com a tela de login aberta, ou leia o journal do `halod` depois de entrar), a linha do usuário some, e o grupo `plugdev` continua com `rw-`. O `halod` registra no journal qual acesso encontrou: "acesso pelo grupo plugdev" ou "acesso só pela ACL da sessão" (`journalctl --user -u halod.service`).

## Remover

```sh
sudo rm /etc/udev/rules.d/70-halo.rules
sudo udevadm control --reload
sudo udevadm trigger --subsystem-match=hidraw --action=change
```

## Voltar à versão que segue a sessão gráfica

Antes da change `halod-outside-session`, o `halod.service` subia e parava com o `graphical-session.target`. Para voltar a ela:

1. Rode o `packaging/install.sh --uninstall` **desta** versão. Ele para e desabilita o `halod-session.service` e o `halod.service` e remove os dois.
2. Confira que os links sumiram. Os dois comandos devem dizer que o arquivo não existe:

   ```sh
   ls ~/.config/systemd/user/default.target.wants/halod.service
   ls ~/.config/systemd/user/graphical-session.target.wants/halod-session.service
   ```

   O instalador antigo só faz `enable`, e não removeria esses links.
3. Só então instale a versão anterior com o `packaging/install.sh` dela, que volta a habilitar pelo `graphical-session.target`.

A regra antiga, só com `uaccess`, volta com os comandos de "Instalar" a partir da versão anterior do arquivo. O grupo `plugdev` e o linger podem ficar como estão.

## SMBus da RAM: `sudo`, não o grupo `i2c`

O `halo probe ram-controller`, que só lê os controladores RGB dos pentes em `0x58` + slot, abre o `/dev/i2c-N` do SMBus da placa. Esse nó é `root:i2c 660`, e a regra `/etc/udev/rules.d/72-halo-smbus-no-uaccess.rules` tira dele o `uaccess` da sessão (ver "Primeiro boot com o lax" em [hardware.md](hardware.md)). Por isso o comando roda com `sudo`, no terminal, a cada uso:

```sh
sudo target/debug/halo probe ram-controller
```

O halo não instala regra udev para esse nó e não pede para pôr o usuário no grupo `i2c`. Os dois caminhos dariam a qualquer processo do usuário, inclusive a um navegador ou a um AppImage, acesso ao barramento inteiro, onde também estão o VRM do processador (`0x60`) e os DACs de tensão (`0x10` e `0x15`, este com a tensão da DRAM). Com o `sudo`, o acesso vale só para aquele comando, e o halo só endereça os controladores dos pentes confirmados pelo SPD. Um acesso sem root para uso diário, com um helper que só aceite os comandos do controlador, fica para uma change própria.

O `halo probe ram`, que só lê o SPD pelo sysfs, não precisa de `sudo`.

### Reaplicar a RAM no boot e ao acordar: a cópia do root

O pente não guarda o que recebe depois de um corte de energia, e numa suspensão S3 fica aceso. Duas unidades do systemd de sistema, `halo-ram-restore.service` (no boot) e `halo-ram-sleep.service` (presa ao `sleep.target`), rodam como root o `halo led ram restore` e o `halo led ram suspend`. Elas executam só `/usr/local/libexec/halo/halo`, uma cópia do `halo` que pertence ao root, e **nunca** o `~/.local/bin/halo`. Aquele arquivo é do usuário: qualquer processo dele (um navegador, um AppImage) poderia trocá-lo e ganharia root no boot seguinte.

O estado que elas reaplicam fica em `/var/lib/halo/ram-led`, gravado pelo próprio `sudo halo led ram …` a cada comando que dá certo (arquivo `0644`, diretório `0755`, gravação atômica). Como ele é lido por root, o `restore` e o `suspend` recusam o arquivo, sem tocar no barramento, se ele ou o diretório não forem do usuário efetivo (o root, nas unidades), se o grupo ou outros puderem gravar neles, se ele for um link ou se uma linha não passar na validação.

O `packaging/install.sh` nunca usa `sudo`: ele compara a cópia do root com o `halo` compilado e as unidades com as do repositório, e mostra os comandos que faltam:

```sh
sudo install -D -m 755 target/release/halo /usr/local/libexec/halo/halo
sudo install -m 644 packaging/systemd/system/halo-ram-restore.service /etc/systemd/system/
sudo install -m 644 packaging/systemd/system/halo-ram-sleep.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable halo-ram-restore.service halo-ram-sleep.service
```

Depois de atualizar o halo, a cópia do root precisa ser trocada de novo, e o script avisa quando ela está diferente. O `--uninstall` só mostra os comandos para desabilitar e remover as três peças; o estado em `/var/lib/halo` fica.

## Regras do OpenRGB

O `/etc/udev/rules.d/60-openrgb.rules`, instalado com o OpenRGB, também libera o IT5702 e o mouse, além de todos os `/dev/i2c-*`. O halo não depende dele. Removê-lo é uma decisão à parte: o mouse (`046d:c08b`) perde o acesso até o halo ter uma regra para ele.
