#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Monta em <dir> um sysfs falso para o halod de teste: o Castor3 do Waterforce
# em class/hidraw/hidraw99 e o hwmon dele, com o líquido a 31 °C.
#
# O halod transforma o nome do hidraw em /dev/<nome>, então o nome falso não
# pode existir em /dev: com hidraw3, por exemplo, ele abriria o teclado
# Redragon desta máquina. Com hidraw99, o halod acha o Waterforce, mas
# qualquer abertura falha com NotFound antes de qualquer escrita. O script
# recusa rodar se /dev/<nome> existir.
#
# Uso: sh scripts/dev/fake-sysfs.sh <dir>
#   HALO_FAKE_HIDRAW troca o nome do nó (só para testar a recusa).

set -eu

[ $# -eq 1 ] || { echo "uso: sh scripts/dev/fake-sysfs.sh <dir>" >&2; exit 2; }
SYS=$1
NODE=${HALO_FAKE_HIDRAW:-hidraw99}

if [ -e "/dev/$NODE" ]; then
    echo "halo: /dev/$NODE existe nesta máquina; o sysfs falso levaria o halod a um aparelho real" >&2
    exit 1
fi

HID=$SYS/class/hidraw/$NODE/device
mkdir -p "$HID"
printf 'HID_ID=0003:00001044:00007A4D\nHID_NAME=ITE Tech. Castor3\n' > "$HID/uevent"
# Usage page 0x0000, coleção de aplicação: o descritor do Castor3.
printf '\005\000\011\002\241\001\300' > "$HID/report_descriptor"

HW=$SYS/class/hwmon/hwmon4
mkdir -p "$HW/device"
echo waterforce > "$HW/name"
printf 'DRIVER=gigabyte_waterforce\nHID_ID=0003:00001044:00007A4D\n' > "$HW/device/uevent"
for kv in fan1_input:1200 fan2_input:2600 pwm1:120 pwm2:200 temp1_input:31000; do
    echo "${kv#*:}" > "$HW/${kv%%:*}"
done
