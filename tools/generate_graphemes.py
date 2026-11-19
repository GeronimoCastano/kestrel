#!/usr/bin/env python3
"""Generate Unicode 17 extended-grapheme properties and the conformance fixture."""
from pathlib import Path
import hashlib
import urllib.request
ROOT = Path(__file__).resolve().parents[1]
BASE = 'https://www.unicode.org/Public/17.0.0/ucd/'
files = {name: urllib.request.urlopen(BASE + name).read() for name in [
    'auxiliary/GraphemeBreakProperty.txt', 'emoji/emoji-data.txt',
    'DerivedCoreProperties.txt', 'auxiliary/GraphemeBreakTest.txt']}
props = ['CR', 'LF', 'Control', 'Extend', 'ZWJ', 'Regional_Indicator', 'Prepend', 'SpacingMark', 'L', 'V', 'T', 'LV', 'LVT']
lines = ['/* Generated from Unicode 17.0.0; see licenses/UNICODE.txt. */',
         '/* Regenerate: python3 tools/generate_graphemes.py */']
for name, data in files.items():
    lines.append(f'/* {name} SHA256 {hashlib.sha256(data).hexdigest()} */')
lines += ['enum { G_OTHER, ' + ', '.join('G_' + p.upper() for p in props) + ' };',
          'typedef struct { uint32_t first, last; unsigned char value; } KGraphemeRange;']
for table, name, mode in [('gcb', 'auxiliary/GraphemeBreakProperty.txt', 0), ('pictographic', 'emoji/emoji-data.txt', 1), ('incb', 'DerivedCoreProperties.txt', 2)]:
    rows = []
    for line in files[name].decode().splitlines():
        fields = [part.strip() for part in line.split('#')[0].split(';')]
        if len(fields) < 2: continue
        if mode == 0: value = 'G_' + fields[1].upper()
        elif mode == 1:
            if fields[1] != 'Extended_Pictographic': continue
            value = '1'
        else:
            if fields[1] != 'InCB' or len(fields) < 3: continue
            value = str({'Consonant': 1, 'Extend': 2, 'Linker': 3}[fields[2]])
        interval = fields[0].split('..')
        rows.append((int(interval[0], 16), int(interval[-1], 16), value))
    lines += [f'static const KGraphemeRange k_grapheme_{table}[] = {{']
    lines += [f'    {{{a:#x}, {b:#x}, {v}}},' for a,b,v in sorted(rows)] + ['};']
(ROOT / 'src/grapheme_data.c').write_text('\n'.join(lines) + '\n')
(ROOT / 'tests/fixtures').mkdir(exist_ok=True)
(ROOT / 'tests/fixtures/GraphemeBreakTest-17.0.0.txt').write_bytes(files['auxiliary/GraphemeBreakTest.txt'])
