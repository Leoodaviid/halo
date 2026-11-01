#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Grafo de chamadas estático das tabelas ACPI descompiladas pelo iasl (.dsl),
# só leitura. Lista todo método que lê ou escreve os campos dados, direta ou
# indiretamente, com quem o chama e, para cada raiz (método sem chamador no
# AML), um caminho até um método que toca os campos.
#
# Uso: aml_field_callers.py <pasta com .dsl> <CAMPO,CAMPO,...> [roots]
#   sem "roots": TSV método, campos tocados direto, chamadores
#   com "roots": TSV raiz, caminho até os campos
#
# Limites: casa nomes curtos (4 caracteres), então dois métodos com o mesmo
# nome em escopos diferentes se misturam, e campos com o nome de outro objeto
# (ex.: HSTS do TPM) aparecem como falso positivo. Não vê chamadas dinâmicas
# (RefOf/DerefOf de método) nem o que roda em SMM depois de um SMI.
import collections
import pathlib
import re
import sys

CALL = re.compile(r'\b([A-Z_][A-Z0-9_]{3})\s*\(')
DEF = re.compile(r'^\s*(Method|Device|Scope|ThermalZone|Processor|PowerResource)'
                 r'\s*\(\s*([\\^A-Z0-9_.]+)')
LOCAL = re.compile(r'Create\w+Field\s*\([^)]*?,\s*([A-Z0-9_]{4})\s*\)')

src, fields = pathlib.Path(sys.argv[1]), set(sys.argv[2].split(','))
mode = sys.argv[3] if len(sys.argv) > 3 else 'table'

bodies = {}  # caminho -> corpo
for f in sorted(src.glob('*.dsl')):
    stack, pending = [], None
    for line in f.read_text(errors='replace').splitlines():
        code = line.split('//')[0]
        m = DEF.match(code)
        if m:
            pending = (m.group(1), m.group(2).split('.')[-1].lstrip('\\^'))
        for kind, _, lines in stack:
            if kind == 'Method':
                lines.append(code)
        s = code.strip()
        if s == '{':
            kind, name = pending or (None, None)
            stack.append((kind, name, []))
            pending = None
        elif s.startswith('}'):
            kind, name, lines = stack.pop()
            if kind == 'Method':
                path = '.'.join(n for _, n, _ in stack if n) + '.' + name
                bodies[path] = '\n'.join(lines)

by_short = collections.defaultdict(set)
for p in bodies:
    by_short[p.rsplit('.', 1)[-1]].add(p)

calls, touch = {}, {}
for p, body in bodies.items():
    # Só a recursão sai: _SB.MPTS chama \GSA1.MPTS, que tem o mesmo nome curto.
    calls[p] = {q for n in set(CALL.findall(body)) for q in by_short.get(n, ())} - {p}
    local = set(LOCAL.findall(body))
    hit = sorted(x for x in fields if x not in local and re.search(r'\b%s\b' % x, body))
    if hit:
        touch[p] = hit

reach = set(touch)
while True:
    new = {p for p, cs in calls.items() if p not in reach and cs & reach}
    if not new:
        break
    reach |= new

callers = collections.defaultdict(set)
for p, cs in calls.items():
    for q in cs:
        callers[q].add(p)


def path_to_fields(p, seen=()):
    if p in touch:
        return [p]
    for q in sorted(calls[p] & reach):
        if q not in seen:
            rest = path_to_fields(q, seen + (p,))
            if rest:
                return [p] + rest
    return None


for p in sorted(reach):
    short = lambda x: x.rsplit('.', 1)[-1]
    if mode == 'roots':
        if not callers[p]:
            print('%s\t%s' % (p, ' -> '.join(short(x) for x in path_to_fields(p))))
    else:
        print('%s\t%s\t%s' % (p, ','.join(touch.get(p, [])) or '-',
                              ','.join(sorted(short(x) for x in callers[p])) or '-'))
