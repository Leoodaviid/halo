#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Cor estática na fita ARGB do D_LED1 do IT5702 (048d:5702).

Pacotes copiados do OpenRGB (commit 72262b8,
Controllers/GigabyteRGBFusion2USBController), caminho de efeito de hardware
de uma fita: SetStripBuiltinEffectState(HDR_D_LED1, true) -> CC 32,
SetLEDEffect(HDR_D_LED1=5, EFFECT_STATIC) -> CC 25, ApplyEffect() -> CC 28.
Não grava na flash (sem CC 47/CC 5E): a cor se perde ao desligar.

Uso: dled1-color.py RRGGBB [--brightness 0-255] [--dry-run]
"""
import argparse
import fcntl
import glob
import os
import struct
import sys

VID, PID = 0x048D, 0x5702
HDR_D_LED1 = 5
EFFECT_STATIC = 1


def hidiocsfeature(length):
    # _IOC(_IOC_WRITE | _IOC_READ, 'H', 0x06, length)
    return (3 << 30) | (length << 16) | (ord("H") << 8) | 0x06


def find_device():
    want = f"HID_ID=0003:{VID:08X}:{PID:08X}"
    for node in sorted(glob.glob("/sys/class/hidraw/hidraw*")):
        with open(os.path.join(node, "device/uevent")) as f:
            if want not in f.read().upper():
                continue
        with open(os.path.join(node, "device/report_descriptor"), "rb") as f:
            desc = f.read()
        if desc[:3] == bytes([0x06, 0x89, 0xFF]):  # usage page 0xFF89
            return "/dev/" + os.path.basename(node)
    sys.exit("IT5702 (048d:5702, usage page 0xFF89) não encontrado")


def packet(*head):
    buf = bytearray(64)
    buf[: len(head)] = bytes(head)
    return buf


def build(r, g, b, brightness):
    zone = 1 << HDR_D_LED1  # 0x20
    # CC 32 <mask>: máscara de efeitos desligados; 0 = efeitos ligados (bit 0x01 = D_LED1)
    enable = packet(0xCC, 0x32, 0x00)
    # PktEffect: report, header 0x20+led, zone0, zone1, reserved0, tipo, max, min, color0 (B G R 0)
    effect = bytearray(64)
    struct.pack_into("<BBIIBBBB", effect, 0, 0xCC, 0x20 + HDR_D_LED1, zone, 0, 0,
                     EFFECT_STATIC, brightness, 0)
    effect[14:18] = bytes([b, g, r, 0])
    # PktEffectApply: CC 28, zone_sel0, zone_sel1
    apply_ = bytearray(64)
    struct.pack_into("<BBII", apply_, 0, 0xCC, 0x28, zone, 0)
    return [("CC 32", enable), ("CC 25", effect), ("CC 28", apply_)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("color", help="RRGGBB")
    ap.add_argument("--brightness", type=int, default=255)
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()
    if len(a.color) != 6 or not 0 <= a.brightness <= 255:
        sys.exit("cor deve ser RRGGBB e brilho 0-255")
    v = int(a.color, 16)
    pkts = build(v >> 16, (v >> 8) & 0xFF, v & 0xFF, a.brightness)
    for name, p in pkts:
        print(f"{name}: {p[:20].hex(' ')} ...")
    if a.dry_run:
        return
    dev = find_device()
    fd = os.open(dev, os.O_RDWR)
    try:
        for name, p in pkts:
            fcntl.ioctl(fd, hidiocsfeature(len(p)), bytes(p))
    finally:
        os.close(fd)
    print(f"enviado para {dev}")


if __name__ == "__main__":
    main()
