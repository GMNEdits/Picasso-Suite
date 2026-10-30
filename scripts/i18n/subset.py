"""Subsets the Noto fonts of the downloads to what the app can show: the characters of the translations plus
the common ones of the language (so file names in it show too). Writes to crates/picasso-ui/assets/fonts."""
import os, glob, sys, re
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools import subset

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..')).replace(chr(92), '/')
DL = sys.argv[1] if len(sys.argv) > 1 else os.environ['TEMP'].replace(chr(92), '/') + '/fonts_dl'
OUT = ROOT + '/crates/picasso-ui/assets/fonts'


def decode_rows(codec, first_lead, last_lead, lo=0xA1, hi=0xFE):
    out = set()
    for a in range(first_lead, last_lead + 1):
        for b in range(lo, hi + 1):
            try:
                out.add(bytes([a, b]).decode(codec))
            except Exception:
                pass
    return out


def used(code):
    s = open(f'{ROOT}/crates/picasso-ui/src/i18n/{code}.rs', encoding='utf-8').read() if os.path.exists(f'{ROOT}/crates/picasso-ui/src/i18n/{code}.rs') else ''
    return set(s)


def sjis_level1():
    out = set()
    for a in range(0x88, 0x99):
        for b in range(0x40, 0xFD):
            try:
                out.add(bytes([a, b]).decode('shift_jis'))
            except Exception:
                pass
    return out


def kana_and_punct():
    s = set()
    for lo, hi in [(0x3000, 0x303F), (0x3040, 0x30FF), (0xFF00, 0xFFEF), (0x2010, 0x2027), (0x2190, 0x2199), (0x25A0, 0x25CF)]:
        s |= {chr(c) for c in range(lo, hi + 1)}
    return s


def arabic():
    s = set()
    for lo, hi in [(0x0600, 0x06FF), (0x0750, 0x077F), (0xFB50, 0xFDFF), (0xFE70, 0xFEFF), (0x200C, 0x200F), (0x0660, 0x0669)]:
        s |= {chr(c) for c in range(lo, hi + 1)}
    return s


JOBS = {
    'NotoSansSC': ('NotoSansSC-Regular.subset.ttf', lambda: decode_rows('gb2312', 0xA1, 0xA9) | decode_rows('gb2312', 0xB0, 0xD7) | kana_and_punct() | used('zh') | set('日本語')),
    'NotoSansJP': ('NotoSansJP-Regular.subset.ttf', lambda: sjis_level1() | kana_and_punct() | used('ja') | set('中文')),
    'NotoSansKR': ('NotoSansKR-Regular.subset.ttf', lambda: decode_rows('euc_kr', 0xB0, 0xC8) | decode_rows('euc_kr', 0xA1, 0xAC) | kana_and_punct() | {chr(c) for c in range(0x3131, 0x318F)} | used('ko')),
    'NotoSansArabic': ('NotoSansArabic-Regular.subset.ttf', lambda: arabic() | used('ar')),
}

ONLY = os.environ.get('SUBSET_ONLY', '').split(',')
for name, (out, chars) in JOBS.items():
    if ONLY != [''] and name not in ONLY:
        continue
    font = TTFont(f'{DL}/{name}.ttf')
    axes = {a.axisTag: a.defaultValue for a in font['fvar'].axes}
    axes['wght'] = 400
    if 'wdth' in axes:
        axes['wdth'] = 100
    font = instancer.instantiateVariableFont(font, axes)
    opts = subset.Options()
    opts.layout_features = ['*']
    opts.name_IDs = ['*']
    opts.notdef_outline = True
    opts.glyph_names = False
    opts.hinting = False
    opts.drop_tables += ['DSIG']
    sub = subset.Subsetter(opts)
    wanted = {ord(c) for c in chars() if ord(c) > 0x7F or True}
    cmap = font.getBestCmap()
    sub.populate(unicodes=[u for u in wanted if u in cmap])
    sub.subset(font)
    font.save(f'{OUT}/{out}')
    print(name, os.path.getsize(f'{OUT}/{out}') // 1024, 'KB', len(wanted & set(cmap)), 'chars')
