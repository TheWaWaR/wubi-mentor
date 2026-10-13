#!/usr/bin/env python3
"""Build data/texts/*.txt: 内置的《大学》《中庸》《论语》，朱熹《四书章句集注》本，连同朱注。

来源是维基文库的《四书章句集註》（大學章句、中庸章句、論語集注卷一至卷十），
朱注（{{annotate|…}}）放进正文后的括号里，维基文库编者的校记（①…）去掉，再用 OpenCC 转成简体。

练习界面按空行分页，所以最后按章插空行（见 `paginate`）：《大学》《中庸》一章到「右…章」为止，
《论语》一章一页，篇名和篇首的朱注跟着第一章；太长的章在朱注后面断开。

Usage: build_classics.py <out_dir>            (needs `pip install opencc`)
       build_classics.py --repage <out_dir>   只对已有文件重新分页，不联网
"""
import re
import sys
import time
import urllib.parse
import urllib.request


BOOKS = {
    "daxue.txt": ["大學章句"],
    "zhongyong.txt": ["中庸章句"],
    "lunyu.txt": ["論語集注卷" + n for n in "一二三四五六七八九十"],
}

CIRCLED = "①②③④⑤⑥⑦⑧⑨⑩⑪⑫⑬⑭⑮⑯⑰⑱⑲⑳"


def split_template(body):
    """按顶层 | 切开模板参数。"""
    parts, depth, cur, i = [], 0, "", 0
    while i < len(body):
        two = body[i:i + 2]
        if two in ("{{", "[["):
            depth += 1; cur += two; i += 2; continue
        if two in ("}}", "]]"):
            depth -= 1; cur += two; i += 2; continue
        if body[i] == "|" and depth == 0:
            parts.append(cur); cur = ""; i += 1; continue
        cur += body[i]; i += 1
    parts.append(cur)
    return parts


def render(text):
    out, i = "", 0
    while i < len(text):
        if text.startswith("{{", i):
            depth, j = 0, i
            while j < len(text):
                if text.startswith("{{", j):
                    depth += 1; j += 2
                elif text.startswith("}}", j):
                    depth -= 1; j += 2
                    if depth == 0:
                        break
                else:
                    j += 1
            parts = split_template(text[i + 2:j - 2])
            name = parts[0].strip()
            if name == "annotate":
                note = render("|".join(parts[1:])).strip()
                # 维基文库编者的校记（①…据某本改），不是朱注。
                if note and note[0] not in CIRCLED:
                    out += "（" + note + "）"
            elif name == "ProperNoun" or name == "properNoun" or name == "wikipedia":
                out += render(parts[-1])
            elif name in ("gap", "header", "header2", "PD-old", "textquality", "left-right") or name.startswith("PD"):
                pass
            else:
                out += render(parts[-1]) if len(parts) > 1 else ""
                print("unknown template:", name, file=sys.stderr)
            i = j
        elif text.startswith("[[", i):
            j = text.index("]]", i)
            out += split_template(text[i + 2:j])[-1]
            i = j + 2
        else:
            out += text[i]; i += 1
    return out


def convert(raw, title_of=None):
    raw = raw.split("<onlyinclude>", 1)[-1].split("</onlyinclude>", 1)[0]
    raw = re.sub(r"\{\{header.*?\n\}\}", "", raw, flags=re.S)
    raw = re.sub(r"==\s*全覽\s*==.*?(?===)", "", raw, flags=re.S)
    raw = re.sub(r"-\{(.*?)\}-", r"\1", raw)
    raw = re.sub(r"<[^>]+>", "", raw)
    raw = raw.replace("''", "")
    text = render(raw)
    text = re.sub("[" + CIRCLED + "]", "", text)
    lines = []
    for line in text.split("\n"):
        line = line.strip().lstrip(":*#").strip().strip("　").strip()
        if line.lower().startswith("category:"):
            continue
        m = re.fullmatch(r"=+\s*(.*?)\s*=+", line)
        if m:
            line = m.group(1)
        line = re.sub(r"[ 　]+", "", line) if re.search(r"[一-鿿]", line) else line.strip()
        if line:
            lines.append(line)
    return "\n".join(lines)


# 一页超过这么多字就在朱注后面断开，每页尽量不超过 PAGE_MAX、不少于 PAGE_MIN。
PAGE_LONG = 1200
PAGE_MAX = 900
PAGE_MIN = 300
PIAN_TITLE = re.compile(r"\S{1,4}第[一二三四五六七八九十]+")


def depth(text):
    return text.count("（") - text.count("）")


def chapters(name, lines):
    """把行按章归组。"""
    groups, cur = [], []
    for line in lines:
        # 有的朱注在源文件里断成了两行，括号没合上就接着放。
        if cur and depth("".join(cur)) <= 0:
            if name == "lunyu.txt":
                # 篇名、篇首的朱注都和下一章放一起。
                done = not (PIAN_TITLE.fullmatch(cur[-1]) or cur[-1].startswith("（"))
            else:
                # 「右经一章」「右第一章」之类收尾，序（子程子曰）单独一页。
                done = cur[-1].startswith("右") or cur[-1].startswith("子程子曰")
            if done:
                groups.append(cur); cur = []
        cur.append(line)
    if cur:
        groups.append(cur)
    return groups


def split_long(text):
    """在换行或朱注收尾、接着是正文的地方断开，再凑成不太长的几页。"""
    pieces, cur, level = [], "", 0
    for i, ch in enumerate(text):
        cur += ch
        level += {"（": 1, "）": -1}.get(ch, 0)
        nxt = text[i + 1:i + 2]
        if level == 0 and nxt and (ch == "\n" or ch == "）" and nxt not in "）」』。，；：！？\n"):
            pieces.append(cur); cur = ""
    pieces.append(cur)
    pages, cur = [], ""
    for i, piece in enumerate(pieces):
        rest = sum(len(p) for p in pieces[i:])
        if len(cur) >= PAGE_MIN and len(cur) + len(piece) > PAGE_MAX and rest >= PAGE_MIN:
            pages.append(cur); cur = ""
        cur += piece
    if cur:
        pages.append(cur)
    return [p.strip("\n") for p in pages]


def paginate(name, text):
    """按章分页，页与页之间空一行。"""
    lines = [line for line in text.split("\n") if line.strip()]
    # 维基文库页尾的导航（「四书章句集注」「中庸」）。
    if "四书章句集注" in lines[-2:]:
        lines = lines[:lines.index("四书章句集注", len(lines) - 2)]
    pages = []
    for group in chapters(name, lines):
        page = "\n".join(group)
        pages.extend(split_long(page) if len(page) > PAGE_LONG else [page])
    return "\n\n".join(pages) + "\n"


def fetch(page):
    title = "四書章句集註/" + page
    url = "https://zh.wikisource.org/w/index.php?title=%s&action=raw" % urllib.parse.quote(title)
    req = urllib.request.Request(url, headers={"User-Agent": "zigen-build/0.1"})
    for attempt in range(5):
        try:
            return urllib.request.urlopen(req).read().decode("utf8")
        except OSError:
            time.sleep(5 * (attempt + 1))
    raise SystemExit("fetch failed: " + title)


if __name__ == "__main__":
    if sys.argv[1] == "--repage":
        out_dir = sys.argv[2]
        for name in BOOKS:
            path = f"{out_dir}/{name}"
            with open(path, encoding="utf8") as f:
                text = f.read()
            with open(path, "w", encoding="utf8") as f:
                f.write(paginate(name, text))
            print(name)
        sys.exit()
    import opencc

    cc = opencc.OpenCC("t2s")
    out_dir = sys.argv[1]
    for name, pages in BOOKS.items():
        parts = []
        for page in pages:
            parts.append(convert(fetch(page)))
            time.sleep(2)  # 维基百科的接口限流
        with open(f"{out_dir}/{name}", "w", encoding="utf8") as f:
            f.write(paginate(name, cc.convert("\n".join(parts))))
        print(name)
