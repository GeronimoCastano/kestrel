#!/usr/bin/env python3
"""Regenerate the pinned Unicode tables. Only regeneration needs the network."""
from pathlib import Path
import hashlib
import urllib.request

VERSION = "17.0.0"
ROOT = Path(__file__).resolve().parents[1]
BASE = f"https://www.unicode.org/Public/{VERSION}/ucd/"
FILES = ["UnicodeData.txt", "SpecialCasing.txt", "DerivedCoreProperties.txt", "PropList.txt", "CaseFolding.txt"]
data = {name: urllib.request.urlopen(BASE + name).read() for name in FILES}
lower, upper, titlecase, special_title = {}, {}, [], {}
for line in data["UnicodeData.txt"].decode().splitlines():
    fields = line.split(";")
    code = int(fields[0], 16)
    if fields[2] == "Lt":
        titlecase.append([code, code])
    for table, index in [(upper, 12), (lower, 13)]:
        if fields[index]:
            table[code] = [int(fields[index], 16)]
for line in data["SpecialCasing.txt"].decode().splitlines():
    fields = [part.strip() for part in line.split("#")[0].split(";")]
    if len(fields) < 5 or fields[4]:
        continue
    code = int(fields[0], 16)
    lower[code] = [int(value, 16) for value in fields[1].split()]
    upper[code] = [int(value, 16) for value in fields[3].split()]
    special_title[code] = [int(value, 16) for value in fields[2].split()]

def ranges(filename, property_name):
    result = []
    for line in data[filename].decode().splitlines():
        fields = [part.strip() for part in line.split("#")[0].split(";")]
        if len(fields) < 2 or fields[1] != property_name:
            continue
        interval = fields[0].split("..")
        start, end = int(interval[0], 16), int(interval[-1], 16)
        if result and result[-1][1] + 1 == start:
            result[-1][1] = end
        else:
            result.append([start, end])
    return result

def category_ranges(predicate):
    """Code point ranges whose UnicodeData fields satisfy predicate. Range rows (First>/Last>) are expanded."""
    result, pending = [], None
    for line in data["UnicodeData.txt"].decode().splitlines():
        fields = line.split(";")
        code, name = int(fields[0], 16), fields[1]
        if name.endswith("First>"):
            pending = code
            continue
        start = pending if name.endswith("Last>") and pending is not None else code
        pending = None
        if predicate(fields):
            if result and result[-1][1] + 1 == start:
                result[-1][1] = code
            else:
                result.append([start, code])
    return result

def complement(ranges_in):
    """The code points from 0 to 0x10FFFF that the sorted ranges do not cover."""
    result, next_free = [], 0
    for start, end in ranges_in:
        if start > next_free:
            result.append([next_free, start - 1])
        next_free = end + 1
    if next_free <= 0x10FFFF:
        result.append([next_free, 0x10FFFF])
    return result

def simple_mapping(index):
    """Code points whose UnicodeData field at index maps them to another code point."""
    table = {}
    for line in data["UnicodeData.txt"].decode().splitlines():
        fields = line.split(";")
        if fields[index]:
            code, mapped = int(fields[0], 16), int(fields[index], 16)
            if mapped != code:
                table[code] = [mapped]
    return table

def folding_table():
    """Full case folding (statuses C and F), which casefold uses."""
    table = {}
    for line in data["CaseFolding.txt"].decode().splitlines():
        body = line.split("#")[0].strip()
        if not body:
            continue
        fields = [part.strip() for part in body.split(";")]
        if fields[1] in ("C", "F"):
            table[int(fields[0], 16)] = [int(value, 16) for value in fields[2].split()]
    return table

lines = [f"/* Generated from Unicode {VERSION}; see licenses/UNICODE.txt. */"]
for filename, content in data.items():
    lines.append(f"/* {filename} SHA256 {hashlib.sha256(content).hexdigest()} */")
lines += ["typedef struct { uint32_t code, mapped[3]; uint8_t length; } KCaseMap;", "typedef struct { uint32_t start, end; } KCodeRange;"]
for name, table in [("lower", lower), ("upper", upper)]:
    lines.append(f"static const KCaseMap k_unicode_{name}[] = {{")
    for code, mapped in sorted(table.items()):
        if mapped == [code]:
            continue
        assert 0 < len(mapped) <= 3
        values = ",".join(hex(value) for value in mapped + [0] * (3 - len(mapped)))
        lines.append(f"    {{{hex(code)},{{{values}}},{len(mapped)}}},")
    lines.append("};")
for name, filename, prop in [("cased", "DerivedCoreProperties.txt", "Cased"), ("ignorable", "DerivedCoreProperties.txt", "Case_Ignorable"), ("space", "PropList.txt", "White_Space"), ("upper_property", "DerivedCoreProperties.txt", "Uppercase"), ("lower_property", "DerivedCoreProperties.txt", "Lowercase"), ("xid_start", "DerivedCoreProperties.txt", "XID_Start"), ("xid_continue", "DerivedCoreProperties.txt", "XID_Continue")]:
    lines.append(f"static const KCodeRange k_unicode_{name}[] = {{")
    lines.extend(f"    {{{hex(start)},{hex(end)}}}," for start, end in ranges(filename, prop))
    lines.append("};")
# Python's str predicates: letters (L*), decimal digits, digits and numerics from the numeric fields.
for name, predicate in [
    ("letter", lambda f: f[2] in ("Lu", "Ll", "Lt", "Lm", "Lo")),
    ("decimal", lambda f: bool(f[6])),
    ("digit", lambda f: bool(f[7])),
    ("numeric", lambda f: bool(f[8])),
    ("titlecase", lambda f: f[2] == "Lt"),
]:
    lines.append(f"static const KCodeRange k_unicode_{name}[] = {{")
    lines.extend(f"    {{{hex(start)},{hex(end)}}}," for start, end in category_ranges(predicate))
    lines.append("};")
# Characters that str.isprintable rejects: control, format, surrogate, private use, separator and unassigned code points.
assigned = category_ranges(lambda f: True)
unprintable = sorted(category_ranges(lambda f: f[2] in ("Cc", "Cf", "Cs", "Co", "Zl", "Zp", "Zs")) + complement(assigned))
lines.append("static const KCodeRange k_unicode_unprintable[] = {")
lines.extend(f"    {{{hex(start)},{hex(end)}}}," for start, end in unprintable)
lines.append("};")
title = simple_mapping(14)
title.update({code: mapped for code, mapped in special_title.items() if mapped != [code]})
for name, table in [("title", title), ("fold", folding_table())]:
    lines.append(f"static const KCaseMap k_unicode_{name}[] = {{")
    for code, mapped in sorted(table.items()):
        values = ",".join(hex(value) for value in mapped + [0] * (3 - len(mapped)))
        lines.append(f"    {{{hex(code)},{{{values}}},{len(mapped)}}},")
    lines.append("};")
(ROOT / "src/unicode_data.c").write_text("\n".join(lines) + "\n")
lines = [f"// Generated from Unicode {VERSION}; see licenses/UNICODE.txt.",
         "// Regenerate with python3 tools/generate_unicode.py."]
for filename in ["UnicodeData.txt", "DerivedCoreProperties.txt"]:
    lines.append(f"// {filename} SHA256 {hashlib.sha256(data[filename]).hexdigest()}")
for name, prop in [("START", "XID_Start"), ("CONTINUE", "XID_Continue"),
                   ("LOWER", "Lowercase"), ("UPPER", "Uppercase"),
                   ("IGNORABLE", "Default_Ignorable_Code_Point"), ("TITLE", None)]:
    lines.append(f"pub(super) const {name}: &[(u32, u32)] = &[")
    values = ranges("DerivedCoreProperties.txt", prop) if prop else titlecase
    lines.extend(f"    ({hex(start)}, {hex(end)})," for start, end in values)
    lines.append("];")
(ROOT / "src/identifier_data.rs").write_text("\n".join(lines) + "\n")
(ROOT / "licenses").mkdir(exist_ok=True)
(ROOT / "licenses/UNICODE.txt").write_bytes(urllib.request.urlopen("https://www.unicode.org/license.txt").read())
print(f"Generated Unicode {VERSION} mappings and properties")
