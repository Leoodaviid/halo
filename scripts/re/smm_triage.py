#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Triagem estática dos módulos SMM de uma imagem UEFI extraída pelo
# UEFIExtract (pasta <imagem>.dump, com o <imagem>.report.txt ao lado).
# Só leitura. Para cada módulo SMM, lista os protocolos de despacho cujo GUID
# aparece no binário (quem registra tratadores periódicos, de Sx, de SW SMI,
# de GPI ou do botão de energia), se ele usa o protocolo de SMBus do AMI e se
# tem "mov edx/dx, 0xB00" (acesso direto à base do SMBus).
#
# Uso: smm_triage.py <imagem>.dump
#
# Limites: o GUID ser citado não prova que o tratador é registrado (confirmar
# no Ghidra). Quem monta o GUID com imediatos (como o próprio SmBusSmm ao
# instalar o protocolo) não aparece, e quem calcula a base do SMBus em tempo
# de execução também não.
import pathlib
import re
import sys
import uuid

GUIDS = {
    'PeriodicTimer2': '4cec368e-8e8e-4d71-8be1-958c45fc8a53',
    'SxDispatch2': '456d2859-a84b-4e47-a2ee-3276d886997d',
    'SwDispatch2': '18a3c6dc-5eea-48c8-a1c1-b53389f98999',
    'GpiDispatch2': '25566b03-b577-4cbf-958c-ed663ea24380',
    'PowerButton2': '1b1183fa-1823-46a7-8872-9c578755409d',
    # Instalado pelo SmBusSmm (FUN_00001688 no F21); Execute(this, addr, cmd, op, pec, len, buf).
    'SmBusSmm': '72e40094-2ee1-497a-8f33-4c934a9e9c0c',
}
PATTERNS = {k: uuid.UUID(v).bytes_le for k, v in GUIDS.items()}
IO = {'mov dx,0xB00': b'\x66\xba\x00\x0b', 'mov edx,0xB00': b'\xba\x00\x0b\x00\x00'}

root = pathlib.Path(sys.argv[1])
report = root.parent / (root.name.removesuffix('.dump') + '.report.txt')
smm = {line.rsplit('|', 1)[-1].strip()
       for line in report.read_text(errors='replace').splitlines()
       if re.search(r'\| SMM module +\|', line)}

rows = set()
for d in root.rglob('*'):
    m = re.match(r'\d+ (.+)$', d.name)
    if not d.is_dir() or not m or m.group(1) not in smm:
        continue
    for f in d.rglob('body.bin'):
        if 'PE32' not in str(f):
            continue
        b = f.read_bytes()
        hits = [k for k, v in PATTERNS.items() if v in b]
        io = [k for k, v in IO.items() if v in b]
        if hits or io:
            rows.add('%-26s %s %s' % (m.group(1), ','.join(hits), 'IO:' + ','.join(io) if io else ''))
print('\n'.join(sorted(rows)))
