#!/usr/bin/env python3
"""给「黑体字根.ttf」换上有效的字体名。

原字体的 PostScript 名是 "-"，Mac 平台的名字记录是空的，而且 GPUI 会跳过没有字母 m 的字体。
这里重写两张表：
- name：族名 "Wubi Roots"，PostScript 名 "WubiRoots-Regular"；
- cmap：把 m 映射到不间断空格的字形（字体只用私用区，m 只是为了通过检查）。

Usage: patch_root_font.py <in.ttf> <out.ttf>
"""
import struct
import sys

FAMILY = 'Wubi Roots'
NAMES = {
    1: FAMILY,
    2: 'Regular',
    3: 'Wubi Roots Regular 1.00',
    4: 'Wubi Roots Regular',
    5: 'Version 1.00',
    6: 'WubiRoots-Regular',
}


def checksum(data):
    data += b'\0' * (-len(data) % 4)
    return sum(struct.unpack(f'>{len(data) // 4}I', data)) & 0xFFFFFFFF


def build_name_table():
    records, strings = [], b''
    for platform, encoding, language, encode in [
        (1, 0, 0, lambda s: s.encode('ascii')),
        (3, 1, 0x409, lambda s: s.encode('utf-16-be')),
    ]:
        for name_id, text in sorted(NAMES.items()):
            raw = encode(text)
            records.append(struct.pack('>6H', platform, encoding, language, name_id, len(raw), len(strings)))
            strings += raw
    header = struct.pack('>3H', 0, len(records), 6 + 12 * len(records))
    return header + b''.join(records) + strings


def read_cmap(font, offset):
    """读出 format 4 和 format 12 子表的全部映射。"""
    count = struct.unpack('>H', font[offset + 2:offset + 4])[0]
    mapping = {}
    for i in range(count):
        _, _, sub = struct.unpack('>HHI', font[offset + 4 + 8 * i:offset + 12 + 8 * i])
        start = offset + sub
        fmt = struct.unpack('>H', font[start:start + 2])[0]
        if fmt == 12:
            groups = struct.unpack('>I', font[start + 12:start + 16])[0]
            for g in range(groups):
                first, last, glyph = struct.unpack('>III', font[start + 16 + 12 * g:start + 28 + 12 * g])
                for c in range(first, last + 1):
                    mapping[c] = glyph + c - first
        elif fmt == 4:
            seg = struct.unpack('>H', font[start + 6:start + 8])[0] // 2
            arr = lambda k: struct.unpack(f'>{seg}H', font[start + 14 + k:start + 14 + k + 2 * seg])
            ends, starts = arr(0), arr(2 * seg + 2)
            deltas, ranges = arr(4 * seg + 2), arr(6 * seg + 2)
            for s_i in range(seg):
                for c in range(starts[s_i], ends[s_i] + 1):
                    if c == 0xFFFF:
                        continue
                    if ranges[s_i] == 0:
                        g = (c + deltas[s_i]) & 0xFFFF
                    else:
                        at = start + 14 + 6 * seg + 2 + 2 * s_i + ranges[s_i] + 2 * (c - starts[s_i])
                        g = struct.unpack('>H', font[at:at + 2])[0]
                        g = (g + deltas[s_i]) & 0xFFFF if g else 0
                    if g:
                        mapping[c] = g
    return mapping


def build_cmap(mapping):
    bmp = sorted(c for c in mapping if c <= 0xFFFF) + [0xFFFF]
    seg = len(bmp)
    ends, starts, deltas = bmp, bmp, [(mapping.get(c, 0) - c) & 0xFFFF if c != 0xFFFF else 1 for c in bmp]
    search = 2 ** (seg.bit_length() - 1) * 2
    body = struct.pack('>4H', seg * 2, search, (search // 2).bit_length() - 1, seg * 2 - search)
    body += struct.pack(f'>{seg}H', *ends) + b'\0\0' + struct.pack(f'>{seg}H', *starts)
    body += struct.pack(f'>{seg}H', *deltas) + struct.pack(f'>{seg}H', *([0] * seg))
    fmt4 = struct.pack('>3H', 4, 6 + len(body), 0) + body
    groups = sorted(mapping.items())
    fmt12 = struct.pack('>HHIII', 12, 0, 16 + 12 * len(groups), 0, len(groups))
    fmt12 += b''.join(struct.pack('>III', c, c, g) for c, g in groups)
    header = struct.pack('>HH', 0, 4)
    first = 4 + 8 * 4
    records = [(0, 3, first), (0, 4, first + len(fmt4)), (3, 1, first), (3, 10, first + len(fmt4))]
    return header + b''.join(struct.pack('>HHI', *r) for r in records) + fmt4 + fmt12


def replace_table(font, num_tables, tag, data):
    offset = len(font) + (-len(font) % 4)
    font += b'\0' * (offset - len(font)) + data
    for i in range(num_tables):
        entry = 12 + 16 * i
        if font[entry:entry + 4] == tag:
            struct.pack_into('>III', font, entry + 4, checksum(data), offset, len(data))


def main(src, dst):
    font = bytearray(open(src, 'rb').read())
    num_tables = struct.unpack('>H', font[4:6])[0]
    tables = {bytes(font[12 + 16 * i:16 + 16 * i]): struct.unpack('>I', font[20 + 16 * i:24 + 16 * i])[0]
              for i in range(num_tables)}
    mapping = read_cmap(font, tables[b'cmap'])
    mapping[ord('m')] = mapping[0xA0]
    replace_table(font, num_tables, b'cmap', build_cmap(mapping))
    replace_table(font, num_tables, b'name', build_name_table())
    # head.checkSumAdjustment
    for i in range(num_tables):
        entry = 12 + 16 * i
        if font[entry:entry + 4] == b'head':
            head = struct.unpack('>I', font[entry + 8:entry + 12])[0]
            struct.pack_into('>I', font, head + 8, 0)
            struct.pack_into('>I', font, head + 8, (0xB1B0AFBA - checksum(bytes(font))) & 0xFFFFFFFF)
    open(dst, 'wb').write(font)


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
