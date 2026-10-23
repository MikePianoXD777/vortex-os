#!/usr/bin/env python3
"""Busca manejadores on<Señal> repetidos en un mismo objeto QML (QML no lo admite)."""
import re, sys, pathlib
def limpiar(t):
    t = re.sub(r'/\*.*?\*/', lambda m: '\n'*m.group(0).count('\n'), t, flags=re.S)
    t = re.sub(r'//[^\n]*', '', t)
    t = re.sub(r'"(?:\\.|[^"\\\n])*"', '""', t)
    t = re.sub(r"'(?:\\.|[^'\\\n])*'", "''", t)
    t = re.sub(r'`(?:\\.|[^`\\])*`', '``', t, flags=re.S)
    return t
mal = 0
for f in sorted(pathlib.Path(sys.argv[1]).rglob('*.qml')):
    pila = [dict()]
    for n, linea in enumerate(limpiar(f.read_text(errors='replace')).split('\n'), 1):
        m = re.match(r'\s*(on[A-Z]\w*)\s*:', linea)
        if m:
            if m.group(1) in pila[-1]:
                print(f"{f}:{n}: {m.group(1)} repetido (antes en la línea {pila[-1][m.group(1)]})"); mal += 1
            else:
                pila[-1][m.group(1)] = n
        for c in linea:
            if c == '{': pila.append(dict())
            elif c == '}' and len(pila) > 1: pila.pop()
print("duplicados:", mal)
sys.exit(1 if mal else 0)
