#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Captura passiva do status `99 DA` do Waterforce (Castor3, 1044:7a4d). Abre o
# hidraw só para leitura e não escreve nada no aparelho: quem pede o `DA` é o
# driver `gigabyte_waterforce` do kernel, quando o `temp1_input` do hwmon é
# lido, e o hidraw recebe uma cópia de cada input report. Para comparar,
# mostra também o Tctl do k10temp, achado pelo rótulo `Tctl` (sem índice fixo).
#
# Uso: waterforce_da_sniff.py [segundos=40] [relatórios=14] [--jsonl ARQUIVO]
#
# `relatórios` 0 é sem limite. Com --jsonl, cada DA vira uma linha com o
# relógio monotônico (CLOCK_MONOTONIC) e o de parede (para cruzar com o
# `journalctl -o short-iso-precise`), o Tctl em m°C, os bytes 2..13 brutos e
# os campos decodificados (docs/protocols/castor3.md): RPM da fan em
# r[2..4], RPM da bomba em r[5..7] (little-endian) e duty da fan em r[8].
import glob, json, os, select, sys, time


def hwmon(name):
    for d in glob.glob("/sys/class/hwmon/hwmon*"):
        if open(os.path.join(d, "name")).read().strip() == name:
            return d
    return None


def find_tctl():
    found = []
    for d in glob.glob("/sys/class/hwmon/hwmon*"):
        if open(os.path.join(d, "name")).read().strip() != "k10temp":
            continue
        for label in glob.glob(os.path.join(d, "temp*_label")):
            if open(label).read().strip() == "Tctl":
                found.append(label[: -len("_label")] + "_input")
    return found[0] if len(found) == 1 else None


def find_hidraw():
    for d in glob.glob("/sys/class/hidraw/hidraw*"):
        try:
            uevent = open(os.path.join(d, "device/uevent")).read()
        except OSError:
            continue
        if "HID_ID=0003:00001044:00007A4D" in uevent:
            return "/dev/" + os.path.basename(d)
    return None


def read_tctl(path):
    try:
        return int(open(path).read())
    except (OSError, ValueError, TypeError):
        return None


args = sys.argv[1:]
jsonl = None
if "--jsonl" in args:
    i = args.index("--jsonl")
    jsonl = args[i + 1]
    del args[i : i + 2]
seconds = float(args[0]) if len(args) > 0 else 40
wanted = int(args[1]) if len(args) > 1 else 14
dev, wf, tctl_path = find_hidraw(), hwmon("waterforce"), find_tctl()
if dev is None or wf is None:
    sys.exit("Waterforce não encontrado (hidraw ou hwmon waterforce)")

fd = os.open(dev, os.O_RDONLY | os.O_NONBLOCK)
out = open(jsonl, "a", buffering=1) if jsonl else None
print(f"# {dev} (só leitura), {wf}, Tctl: {tctl_path or 'não achado'}")
print("# hora     temp1   Tctl | b2..13 do DA | fan rpm duty")
seen, next_poke, temp = 0, 0.0, "?"
deadline = time.time() + seconds
try:
    if out:
        out.write(json.dumps({"event": "opened", "mono": time.monotonic(), "wall": time.time(),
                              "node": dev, "tctl_path": tctl_path}) + "\n")
    while time.time() < deadline and (wanted == 0 or seen < wanted):
        if time.time() >= next_poke:
            temp = open(os.path.join(wf, "temp1_input")).read().strip()
            next_poke = time.time() + 2.5
        if not select.select([fd], [], [], 0.5)[0]:
            continue
        data = os.read(fd, 6144)
        if len(data) < 14 or data[0] != 0x99 or data[1] != 0xDA:
            continue
        seen += 1
        mono, wall = time.monotonic(), time.time()
        tctl = read_tctl(tctl_path)
        fan_rpm = data[2] | data[3] << 8 | data[4] << 16
        pump_rpm = data[5] | data[6] << 8 | data[7] << 16
        duty = data[8]
        shown = f"{tctl / 1000:5.1f}" if tctl is not None else "    ?"
        print(f"{time.strftime('%T')} {temp:>7} {shown} | {data[2:14].hex(' ')} | {fan_rpm} {duty}")
        if out:
            out.write(json.dumps({"event": "DA", "mono": mono, "wall": wall, "tctl": tctl,
                                  "coolant_raw": temp, "raw": data[2:14].hex(" "),
                                  "fan_rpm": fan_rpm, "pump_rpm": pump_rpm,
                                  "fan_duty": duty}) + "\n")
finally:
    os.close(fd)
    if out:
        out.close()
