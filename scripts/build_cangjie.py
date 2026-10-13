#!/usr/bin/env python3
"""Build data/cangjie5.tsv: 倉頡第五代的編碼和字首／字身拆分。

Inputs (download first, see README):
  cangjie5.base.dict.yaml - https://github.com/rime/rime-cangjie (編碼的權威來源)
  cjk-decomp.txt          - https://github.com/amake/cjk-decomp (字形結構，用來找字首、字身)

Usage: build_cangjie.py <src_dir> <out.tsv>

Output columns (tab separated):
  char  code  kind  parts
    kind:  whole = 整體字（或字根本身），split = 字首 + 字身，unknown = 結構資料對不上
    parts: 空格分隔的 `部件:碼[:單獨打的碼]`。第一組是字首，其餘是字身（字身可再分成次字首、次字身）；
           部件字形未知時留空（`:碼`）。`單獨打的碼` 只在部件在字中的取碼和它單獨成字時不同才寫。
Also writes cangjie5_keys.tsv next to the output: 每個鍵上常見的部件（按出現次數排序）。
"""
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

src_dir = Path(sys.argv[1])
out = Path(sys.argv[2])


def is_cjk(ch):
    o = ord(ch)
    return 0x3400 <= o <= 0x4DBF or 0x4E00 <= o <= 0x9FFF or 0xF900 <= o <= 0xFAFF or 0x20000 <= o <= 0x3134F


# ---- 編碼 ----
all_codes = defaultdict(list)
in_body = False
for line in (src_dir / 'cangjie5.base.dict.yaml').read_text(encoding='utf-8').splitlines():
    if line == '...':
        in_body = True
        continue
    if not in_body or not line or line.startswith('#'):
        continue
    cols = line.split('\t')
    if len(cols) < 2 or len(cols[0]) != 1 or not is_cjk(cols[0]):
        continue
    code = cols[1].strip()
    if re.fullmatch('[a-y]{1,5}', code):
        all_codes[cols[0]].append(code)


def pick(codes):
    # x 開頭多是容錯碼（如「个」xol），單字母的 x/z 是詞頻簡碼；都只在沒有別的碼時才用。
    real = [c for c in codes if not c.startswith('x')]
    return (real or codes)[0]


code_of = {ch: pick(cs) for ch, cs in all_codes.items()}

# ---- 結構 ----
decomp = {}
for line in (src_dir / 'cjk-decomp.txt').read_text(encoding='utf-8').splitlines():
    m = re.fullmatch(r'([^:]+):([^(]+)\((.*)\)', line)
    if m:
        decomp[m.group(1)] = (m.group(2), [p for p in m.group(3).split(',') if p])

# 能分出字首（左、上、外）和字身的結構；w（交疊）之類算整體字。
SPLIT_TYPES = {'a', 'd', 'a/m', 'd/m', 'd/t', 'd/s', 's', 'st', 'sb', 'sl', 'str', 'stl', 'sbl', 'sbr', 'ra', 'rd'}


def abbr(code, n):
    """字首取首尾兩碼（n=2），字身取首、次、尾三碼（n=3）。"""
    if len(code) <= n:
        return code
    return code[0] + (code[1] if n == 3 else '') + code[-1]


def is_stroke(part):
    return len(part) == 1 and 0x31C0 <= ord(part) <= 0x31EF


def glyph(part):
    return part if len(part) == 1 and not part.isdigit() else ''


def std_code(part, n):
    c = code_of.get(part)
    return abbr(c, n) if c else None


# 部件在字中常見的取碼（第一遍從互相印證的拆分裏統計，如 亻→O、氵→E）。
in_char = defaultdict(Counter)


def matches(part, code, n):
    return std_code(part, n) == code or in_char[part][code] >= 5


def split(ch, code, depth=0):
    """[(部件, 碼, 單獨打的碼或 None)]；分不出時回傳 None。"""
    d = decomp.get(ch)
    if not d or d[0] not in SPLIT_TYPES or len(d[1]) < 2 or depth > 4:
        return None
    # 拆成筆畫的（如 匕、五）在倉頡裏是整體字。
    if any(is_stroke(x) for x in d[1]):
        return None
    head, body = d[1][0], d[1][1:]
    best = None
    for k in (1, 2):
        if len(code) <= k:
            break
        h, rest = code[:k], code[k:]
        if len(rest) > 3:
            continue
        score = matches(head, h, 2) * 2 + (len(body) == 1 and matches(body[0], rest, 3)) * 2
        # 兩碼的字只能一分為二。
        score += len(code) == 2
        if score and (best is None or score > best[0]):
            best = (score, h, rest)
    if best is None:
        return None
    score, h, rest = best

    def note(part, c, n):
        # 部件在字中的碼和它單獨成字時不同（如 化 的 匕 是 P，單獨打 UH）；常見的輔助字形不算。
        s = std_code(part, n)
        return s if s and s != c and in_char[part][c] < 5 else None

    parts = [(glyph(head), h, note(head, h, 2))]
    # 字身沒被截短時可以再拆成次字首、次字身。
    sub = None
    if len(body) == 1 and code_of.get(body[0]) == rest and len(rest) >= 2:
        sub = split(body[0], rest, depth + 1)
    # 次字首、次字身也要互相印證，免得把整體字（如 毛）硬拆開。
    if sub and sub[1] >= 4:
        parts.extend(sub[0])
    else:
        body_glyph = glyph(body[0]) if len(body) == 1 else ''
        parts.append((body_glyph, rest, note(body[0], rest, 3) if body_glyph else None))
    return parts, score


def build():
    rows, stats = [], Counter()
    for ch, code in code_of.items():
        d = decomp.get(ch)
        res = split(ch, code)
        if res:
            kind, parts = 'split', res[0]
        elif d and d[0] in SPLIT_TYPES and len(d[1]) >= 2 and not any(is_stroke(x) for x in d[1]):
            kind, parts = 'unknown', [('', code, None)]
        else:
            kind, parts = 'whole', [(ch, code, None)]
        stats[kind] += 1
        rows.append((ch, code, kind, parts, res[1] if res else 0))
    return rows, stats


# 第一遍：只用單獨成字的碼互相印證，統計部件在字中的取碼。
for ch, code, kind, parts, score in build()[0]:
    if score >= 2:
        for g, c, _ in parts:
            if g:
                in_char[g][c] += 1
rows, stats = build()
part_votes = defaultdict(Counter)
for ch, code, kind, parts, score in rows:
    for g, c, _ in parts:
        if g and len(c) == 1 and kind == 'split' and not is_stroke(g):
            part_votes[c][g] += 1
rows = ['\t'.join([ch, code.upper(), kind,
                   ' '.join(f'{g}:{c.upper()}' + (f':{s.upper()}' if s else '') for g, c, s in parts)])
        for ch, code, kind, parts, _ in rows]

out.write_text('\n'.join(rows) + '\n', encoding='utf-8')
print(dict(stats), file=sys.stderr)

# ---- 鍵盤：每個鍵上單碼部件的常見字形 ----
ROOTS = dict(zip('abcdefghijklmnopqrstuvwy', '日月金木水火土竹戈十大中一弓人心手口尸廿山女田卜'))
key_rows = []
for k, name in ROOTS.items():
    # 只放主要歸在這個鍵上的部件。
    gs = [g for g, n in part_votes[k].most_common()
          if n >= 3 and g != name and n * 5 >= sum(c[g] for c in part_votes.values()) * 3]
    key_rows.append(f'{k.upper()}\t{name}\t{"".join(gs[:16])}')
(out.parent / 'cangjie5_keys.tsv').write_text('\n'.join(key_rows) + '\n', encoding='utf-8')
