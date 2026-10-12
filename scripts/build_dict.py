#!/usr/bin/env python3
"""Build data/wubi86.tsv from upstream decomposition data.

Inputs (download first, see README):
  wubi86.js   - https://github.com/gmgi1234/wubi/blob/main/data/wubi86.js
  roots.js    - https://github.com/gmgi1234/wubi/blob/main/data/wubi_roots_all.js
  iwubi.json  - https://github.com/iHTCboy/iWuBi (iWuBi_code.json, for short codes)

Output columns (tab separated):
  char  full_code  short_codes(comma)  kind  roots  ident
    kind:  key = 键名字根, radical = 成字字根, char = 普通合体字
    roots: space separated `root:KEY` (a bracketed pseudo root like [牛前] keeps its brackets)
    ident: 识别码 letter when the code uses one, else empty
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

rows, stats = [], Counter()
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
    roots_out = ('' if ok else '!') + ' '.join(f'{x}:{k}' for x, k in zip(roots, keys))
    shorts = sorted({c for c in iw if len(c) < len(full)}, key=lambda c: (len(c), c))
    rows.append('\t'.join([ch, full, ','.join(shorts), kind, roots_out, ident]))

out.write_text('\n'.join(rows) + '\n', encoding='utf-8')
print(dict(stats), file=sys.stderr)
