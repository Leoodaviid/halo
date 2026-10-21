#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Captura passiva do status `99 DA` do Waterforce (Castor3, 1044:7a4d). Abre o
# hidraw só para leitura e não escreve nada no aparelho: quem pede o `DA` é o
# driver `gigabyte_waterforce` do kernel, quando o `temp1_input` do hwmon é
# lido, e o hidraw recebe uma cópia de cada input report. Para comparar,
# mostra também o Tctl do k10temp, se houver.
#
# Uso: waterforce_da_sniff.py [segundos=40] [relatórios=14]
import glob, os, select, sys, time


def hwmon(name):
    for d in glob.glob("/sys/class/hwmon/hwmon*"):
        if open(os.path.join(d, "name")).read().strip() == name:
            return d
    return None


def find_hidraw():
    for d in glob.glob("/sys/class/hidraw/hidraw*"):
        try:
            uevent = open(os.path.join(d, "device/uevent")).read()
        except OSError:
            continue
        if "HID_ID=0003:00001044:00007A4D" in uevent:
            return "/dev/" + os.path.basename(d)
    return None


seconds = float(sys.argv[1]) if len(sys.argv) > 1 else 40
wanted = int(sys.argv[2]) if len(sys.argv) > 2 else 14
dev, wf, cpu = find_hidraw(), hwmon("waterforce"), hwmon("k10temp")
if dev is None or wf is None:
    sys.exit("Waterforce não encontrado (hidraw ou hwmon waterforce)")

fd = os.open(dev, os.O_RDONLY | os.O_NONBLOCK)
print(f"# {dev} (só leitura), {wf}")
print("# hora     temp1   Tctl | b2..13 do DA")
seen, next_poke, temp = 0, 0.0, "?"
deadline = time.time() + seconds
try:
    while time.time() < deadline and seen < wanted:
        if time.time() >= next_poke:
            temp = open(os.path.join(wf, "temp1_input")).read().strip()
            next_poke = time.time() + 2.5
        if not select.select([fd], [], [], 0.5)[0]:
            continue
        data = os.read(fd, 6144)
        if len(data) < 14 or data[0] != 0x99 or data[1] != 0xDA:
            continue
        seen += 1
        tctl = f"{int(open(cpu + '/temp1_input').read()) / 1000:5.1f}" if cpu else "    ?"
        print(f"{time.strftime('%T')} {temp:>7} {tctl} | {data[2:14].hex(' ')}")
finally:
    os.close(fd)
