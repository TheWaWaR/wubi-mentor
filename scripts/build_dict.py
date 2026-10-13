#!/usr/bin/env python3
"""Build data/wubi86.tsv from upstream decomposition data.

Inputs (download first, see README):
  wubi86.js   - https://github.com/gmgi1234/wubi/blob/main/data/wubi86.js
  roots.js    - https://github.com/gmgi1234/wubi/blob/main/data/wubi_roots_all.js
  iwubi.json  - https://github.com/iHTCboy/iWuBi (iWuBi_code.json, for short codes)
  wb_spelling86.yaml - https://github.com/myshiqiqi/rime-wubi (tables/86/wb_spelling.dict.yaml),
                 每一码对应「黑体字根.ttf」私用区里的一个字形

Usage: build_dict.py <src_dir> <out.tsv> [traditional root glyphs to drop from the keyboard]

Output columns (tab separated):
  char  full_code  short_codes(comma)  kind  roots  ident
    kind:  key = 键名字根, radical = 成字字根, char = 普通合体字
    roots: space separated `root:KEY` (a bracketed pseudo root like [牛前] keeps its brackets)
    ident: 识别码 letter when the code uses one, else empty
    glyphs: 每一码在字根字体里的字形（私用区字符），可能为空
  roots 里每个字根写成 `字形:键位[:字根字体字形]`。
Also writes key_roots.tsv next to the output: 每个键在字根字体里的全部字根字形。
"""
import json, re, sys
from pathlib import Path

src_dir = Path(sys.argv[1])
out = Path(sys.argv[2])

js = (src_dir / 'wubi86.js').read_text(encoding='utf-8')
body = js[js.index('{'): js.rindex('}') + 1]
dic = json.loads(re.sub(r',\s*}', '}', body))

roots_js = (src_dir / 'roots.js').read_text(encoding='utf-8')
table_src = re.search(r'WUBI_ROOT_TABLE_86 = \{(.*?)\n\}', roots_js, re.S).group(1)
root_key = {}
for key, val in re.findall(r"'([A-Z])':\s*'(.*?)'", table_src):
    for tok in re.findall(r'\[[^\]]+\]|.', val):
        root_key.setdefault(tok.strip('[]'), key)
radical_src = re.search(r'RADICAL_CHARS_86 = new Set\(\[(.*?)\]\)', roots_js, re.S).group(1)
radicals = {r for r in json.loads('[' + radical_src + ']') if r}
# 且 is listed as a root upstream, but in 86 it is 月 + 一; let it expand.
root_key.pop('且', None)
radicals.discard('且')
# Roots upstream gives no key for (variant code points and pseudo roots).
root_key.update({'\ufa5d': 'A', '㇆': 'N', '互框': 'X', '光头': 'I', '尤前': 'D',
                 '艮底': 'E', '骨头': 'M', '𠄐': 'B'})

KEY_NAMES = dict(zip('GFDSAHJKLMTREWQYUIOPNBVCX', '王土大木工目日口田山禾白月人金言立水火之已子女又纟'))

def key_of(root):
    if root in root_key:
        return root_key[root]
    e = dic.get(root)
    if e and e['c']:
        return e['c'][0]
    return None

def is_root(x):
    return x in root_key or x in radicals

def expand(x, depth=0):
    if depth > 8:
        return [x]
    e = dic.get(x)
    if is_root(x) or not e or not e['r'] or e['r'] == [x]:
        return [x]
    out = []
    for part in e['r']:
        if part == x:
            out.append(part)
        else:
            out.extend(expand(part, depth + 1) if depth == 0 or not is_root(part) else [part])
    return out

codes = json.loads((src_dir / 'iwubi.json').read_text(encoding='utf-8'))['86']


def compute(ch, e):
    r = e['r']
    c = e['c'].upper()
    if r == [ch] and c == (key_of(ch) or '') * 4 and KEY_NAMES.get(c[0]) == ch:
        return 'key', [ch], [c[0]], c, ''
    if r and r[0] == ch:
        # 成字字根: 报户口 + 首、次、末笔
        return 'radical', r, [key_of(x) or '?' for x in r], None, ''
    roots = []
    for part in r:
        roots.extend(expand(part, 1) if not is_root(part) else [part])
    keys = [key_of(x) or '?' for x in roots]
    return 'char', roots, keys, None, ''

def expected(kind, keys, full):
    """Code the roots give, with the 识别码 taken from `full` when needed."""
    if kind == 'key':
        return full, ''
    if kind == 'radical':
        return keys[0] + full[1:], ''
    if len(keys) >= 4:
        return keys[0] + keys[1] + keys[2] + keys[-1], ''
    base = ''.join(keys)
    if len(full) > len(base):
        return base + full[len(base)], full[len(base)]
    return base, ''

def known_codes(ch, e):
    iw = sorted({c.upper() for c in codes.get(ch, [])}, key=len)
    cands = set(iw)
    c = e['c'].upper()
    if re.fullmatch('[A-Y]{1,4}', c):
        cands.add(c)
    return iw, cands

# Pass 1: infer keys for roots upstream leaves unmapped (one unknown per char).
from collections import Counter, defaultdict
votes = defaultdict(Counter)
for ch, e in dic.items():
    kind, roots, keys, _, _ = compute(ch, e)
    if kind != 'char' or keys.count('?') != 1:
        continue
    i = keys.index('?')
    pos = i if len(keys) < 4 or i < 3 else (3 if i == len(keys) - 1 else None)
    if pos is None:
        continue
    for code in known_codes(ch, e)[1]:
        if len(code) == 4 and (len(keys) >= 4 or len(code) > len(keys)):
            votes[roots[i]][code[pos]] += 1
for root, cnt in votes.items():
    k, n = cnt.most_common(1)[0]
    if n >= 2 and root not in root_key:
        root_key[root] = k

records, stats = [], Counter()
for ch, e in dic.items():
    iw, cands = known_codes(ch, e)
    if not cands:
        continue
    longest = max(len(c) for c in cands)
    fulls = [c for c in cands if len(c) == longest]
    kind, roots, keys, _, _ = compute(ch, e)
    full, ident, ok = None, '', False
    for f in sorted(fulls, key=lambda f: (f != e['c'].upper(), f)):
        exp, idt = expected(kind, keys, f)
        if exp == f and '?' not in keys:
            full, ident, ok = f, idt, True
            break
    if not ok:
        full = e['c'].upper() if e['c'].upper() in fulls else sorted(fulls)[0]
        ident = ''
    stats[(kind, ok)] += 1
    shorts = sorted({c for c in iw if len(c) < len(full)}, key=lambda c: (len(c), c))
    records.append(dict(ch=ch, full=full, shorts=shorts, kind=kind, roots=roots, keys=keys, ident=ident, ok=ok))


# ---- 字根字体（黑体字根.ttf）的字形 ----
# spelling: 每个字每一码对应一个私用区字形（字根、识别码或笔画）。
def is_single_stroke(ch):
    return ch in '一丨丿丶乙'

def coded_slots(r):
    """[(code slot, root index)]：编码的每一码来自哪个字根。"""
    n, kind, full = len(r['roots']), r['kind'], r['full']
    if kind == 'key':
        return [(0, 0)]
    if kind == 'radical':
        if is_single_stroke(r['ch']):
            return [(0, 0)]
        slots = [(0, 0), (1, 1)]
        if len(full) == 4:
            slots += [(2, 2), (3, n - 1)]
        elif len(full) == 3:
            slots += [(2, n - 1)]
        return [(s, i) for s, i in slots if i < n]
    if n >= 4:
        return [(0, 0), (1, 1), (2, 2), (3, n - 1)]
    return [(i, i) for i in range(n)]

code_glyphs = {}
spelling = src_dir / 'wb_spelling86.yaml'
if spelling.exists():
    for line in spelling.read_text(encoding='utf-8').splitlines():
        if '\t[' not in line:
            continue
        ch, rest = line.split('\t', 1)
        if len(ch) != 1 or ch in code_glyphs:
            continue
        fields = rest.strip('[]').split(',')
        glyphs = ''.join(c for c in fields[0] if 0xE000 <= ord(c) <= 0xF8FF)
        code = fields[1].strip('※').upper()
        if len(glyphs) == len(code):
            code_glyphs[ch] = (code, glyphs)

root_glyph_votes = defaultdict(Counter)
for r in records:
    cg = code_glyphs.get(r['ch'])
    if not r['ok'] or not cg or cg[0] != r['full']:
        continue
    for slot, i in coded_slots(r):
        root_glyph_votes[r['roots'][i]][cg[1][slot]] += 1
root_glyph = {g: c.most_common(1)[0][0] for g, c in root_glyph_votes.items()}

rows = []
for r in records:
    cg = code_glyphs.get(r['ch'])
    glyphs = cg[1] if cg and cg[0] == r['full'] else ''
    exact = dict(coded_slots(r)) if glyphs and r['ok'] else {}
    by_index = {i: glyphs[s] for s, i in exact.items()}
    tokens = []
    for i, (x, k) in enumerate(zip(r['roots'], r['keys'])):
        g = by_index.get(i) or root_glyph.get(x, '')
        tokens.append(f'{x}:{k}:{g}' if g else f'{x}:{k}')
    roots_out = ('' if r['ok'] else '!') + ' '.join(tokens)
    rows.append('\t'.join([r['ch'], r['full'], ','.join(r['shorts']), r['kind'], roots_out, r['ident'], glyphs]))

out.write_text('\n'.join(rows) + '\n', encoding='utf-8')
print(dict(stats), f'code glyphs: {sum(1 for r in rows if r.split(chr(9))[6])}', file=sys.stderr)

# ---- 键盘字根表：字体里每个键的字形，按字体顺序；去掉识别码、繁体和极少用的笔画变形 ----
key_votes = defaultdict(Counter)
for code, glyphs in code_glyphs.values():
    for k, g in zip(code, glyphs):
        key_votes[g][k] += 1
ident_glyphs = set()
for r in records:
    cg = code_glyphs.get(r['ch'])
    if r['ident'] and cg and cg[0] == r['full']:
        ident_glyphs.add(cg[1][-1])
# 繁体字根（字形对照截图人工挑出）。
TRADITIONAL = set(sys.argv[3]) if len(sys.argv) > 3 else set()
key_rows = []
for k in 'GFDSAHJKLMTREWQYUIOPNBVCX':
    gs = sorted((g for g, c in key_votes.items()
                if c.most_common(1)[0][0] == k and g not in ident_glyphs and g not in TRADITIONAL
                and sum(c.values()) >= 3))
    # 键名字根排第一（界面上单独显示在键帽右上角）。
    name = code_glyphs.get(KEY_NAMES[k], ('', ''))[1][:1]
    if name:
        if name in gs:
            gs.remove(name)
        gs.insert(0, name)
    # 练习页小键盘只放最常用的 15 个（不含键名），保持字根表顺序。
    frequent = set(sorted(gs[1:], key=lambda g: -sum(key_votes[g].values()))[:15])
    compact = ''.join(g for g in gs[1:] if g in frequent)
    key_rows.append(f'{k}\t{"".join(gs)}\t{compact}')
(out.parent / 'key_roots.tsv').write_text('\n'.join(key_rows) + '\n', encoding='utf-8')
