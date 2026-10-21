#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Extrai os blocos de dados de um instalador NSIS não sólido com campos de
# 64 bits (variante usada pelo GBT_Cooler do GIGABYTE Control Center), que o
# 7-Zip não abre. Uso: nsis64_blocks.py <overlay [0] do 7z> <pasta nova>
import os, struct, sys, zlib
src, out = sys.argv[1], sys.argv[2]
d = open(src, 'rb').read()
os.makedirs(out)
csize, = struct.unpack_from('<Q', d, 36)
pos = 44 + (csize & ~(1 << 63))
i = 0
while pos + 8 <= len(d):
    raw, = struct.unpack_from('<Q', d, pos)
    size = raw & ~(1 << 63)
    if size == 0 or pos + 8 + size > len(d):
        break
    blk = d[pos + 8: pos + 8 + size]
    data = zlib.decompressobj(-15).decompress(blk) if raw >> 63 else blk
    open(os.path.join(out, f'block-{i:03d}.bin'), 'wb').write(data)
    pos += 8 + size
    i += 1
print(f'{i} blocos, parou em {pos} de {len(d)}')
