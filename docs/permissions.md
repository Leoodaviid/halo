# Permissões

O halo não roda como root. Para falar com um controlador, o usuário da sessão precisa ter leitura e escrita no `/dev/hidrawN` dele. A regra [`packaging/udev/70-halo.rules`](../packaging/udev/70-halo.rules) dá a tag `uaccess` só aos hidraw que o halo usa:

| Dispositivo | VID:PID |
|---|---|
| Placa-mãe, ITE IT5702 | `048d:5702` |
| AORUS Waterforce, Castor3 | `1044:7a4d` |

Ela não inclui o teclado Redragon nem barramentos I2C/SMBus. A telemetria do Waterforce (`halo telemetry waterforce`) não precisa dessa regra, porque vem do hwmon do kernel, que qualquer usuário pode ler.

## Instalar

```sh
sudo install -m 644 packaging/udev/70-halo.rules /etc/udev/rules.d/
sudo udevadm control --reload
sudo udevadm trigger --subsystem-match=hidraw --action=change
```

## Verificar

```sh
for d in /sys/class/hidraw/*; do
  grep -qE 'HID_ID=0003:0000(048D:00005702|1044:00007A4D)' "$d/device/uevent" && getfacl -p "/dev/$(basename "$d")"
done
```

Cada hidraw listado deve ter uma linha `user:<seu usuário>:rw-`.

## Remover

```sh
sudo rm /etc/udev/rules.d/70-halo.rules
sudo udevadm control --reload
sudo udevadm trigger --subsystem-match=hidraw --action=change
```

## Regras do OpenRGB

O `/etc/udev/rules.d/60-openrgb.rules`, instalado com o OpenRGB, também libera o IT5702 e o mouse, além de todos os `/dev/i2c-*`. O halo não depende dele. Removê-lo é uma decisão à parte: o mouse (`046d:c08b`) perde o acesso até o halo ter uma regra para ele.
