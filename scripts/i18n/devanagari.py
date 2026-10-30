"""Hindi (Devanagari) for a text drawing that sets characters one by one.

Devanagari needs shaping: conjuncts are joined, a vowel sign is put before the letter it is read after,
marks sit above and below. The toolkit does none of it, so it is done here, once, when the tables are made:

- every word of the Devanagari in a table (a run of Devanagari characters up to a space) is shaped with
  HarfBuzz and becomes ONE glyph of a font made for the purpose, called by a character of the Unicode
  private use area (U+E000 on);
- the tables hold that character in the word's place, so the toolkit draws the word as it is to be seen;
- `hi_glyphs.json` keeps which word has which character (a word is never given another one, so the
  Help > Language name in `i18n/mod.rs` stays right when words are added);
- `write_font` makes `NotoSansDevanagari-Regular.subset.ttf`: each such glyph is a composite of the
  glyphs of Noto Sans Devanagari, put where HarfBuzz put them.
"""
import io
import json
import os
import pathlib
import re
import sys

import uharfbuzz as hb
from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._g_l_y_f import Glyph, GlyphComponent
from fontTools.varLib import instancer

HERE = pathlib.Path(__file__).resolve().parent
# The scripts that are shaped here, by language code: font, characters of a word, HarfBuzz script and
# language, and where in the private use area their words begin.
SCRIPTS = {
    'hi': dict(font='NotoSansDevanagari.ttf', run='[ऀ-ॿ‌‍]+', script='Deva', lang='hi', first=0xE000,
               out='NotoSansDevanagari-Regular.subset.ttf'),
    'bn': dict(font='NotoSansBengali.ttf', run='[ঀ-৿‌‍]+', script='Beng', lang='bn', first=0xE800,
               out='NotoSansBengali-Regular.subset.ttf'),
    'th': dict(font='NotoSansThai.ttf', run='[฀-๿]+', script='Thai', lang='th', first=0xF000,
               out='NotoSansThai-Regular.subset.ttf'),
}
CODE = 'hi'
REGISTRY = HERE / 'hi_glyphs.json'
FIRST = 0xE000
RUN = re.compile(SCRIPTS['hi']['run'])

_font_bytes = None
_hb_font = None
_upem = 1000


def select(code):
    """Works on the script of this language from now on."""
    global CODE, REGISTRY, FIRST, RUN, _font_bytes, _hb_font
    if code != CODE:
        CODE = code
        REGISTRY = HERE / f'{code}_glyphs.json'
        FIRST = SCRIPTS[code]['first']
        RUN = re.compile(SCRIPTS[code]['run'])
        _font_bytes = None
        _hb_font = None


def download_dir():
    return pathlib.Path(sys.argv[1] if len(sys.argv) > 1 and os.path.isdir(sys.argv[1]) else os.environ.get('TEMP', '.')) / 'fonts_dl'


def source():
    """The Noto Sans Devanagari at regular weight and width, as bytes of a font file."""
    global _font_bytes
    if _font_bytes is None:
        path = str(pathlib.Path(os.environ.get('TEMP', '.')) / 'fonts_dl' / SCRIPTS[CODE]['font'])
        font = TTFont(path)
        axes = {a.axisTag: a.defaultValue for a in font['fvar'].axes}
        axes['wght'] = 400
        if 'wdth' in axes:
            axes['wdth'] = 100
        font = instancer.instantiateVariableFont(font, axes)
        out = io.BytesIO()
        font.save(out)
        _font_bytes = out.getvalue()
    return _font_bytes


def shaper():
    global _hb_font, _upem
    if _hb_font is None:
        face = hb.Face(source())
        _upem = face.upem
        _hb_font = hb.Font(face)
    return _hb_font


def shape_run(run):
    """(glyph id, x offset, y offset, advance) of each glyph of a word, as HarfBuzz sets it."""
    font = shaper()
    buf = hb.Buffer()
    buf.add_str(run)
    buf.script = SCRIPTS[CODE]['script']
    buf.language = SCRIPTS[CODE]['lang']
    buf.direction = 'ltr'
    hb.shape(font, buf, {'kern': True, 'liga': True, 'ccmp': True})
    return [(i.codepoint, p.x_offset, p.y_offset, p.x_advance) for i, p in zip(buf.glyph_infos, buf.glyph_positions)]


def run_width(run, size=14.0):
    shaper()
    return sum(g[3] for g in shape_run(run)) * size / _upem


def load():
    return json.loads(REGISTRY.read_text(encoding='utf-8')) if REGISTRY.exists() else {}


def to_pua(text):
    """The text with each word of Devanagari as its character of the private use area."""
    registry = load()
    changed = False

    def one(m):
        nonlocal changed
        run = m.group(0)
        if run not in registry:
            registry[run] = FIRST + len(registry)
            changed = True
        return chr(registry[run])

    out = RUN.sub(one, text)
    if changed:
        REGISTRY.write_text(json.dumps(registry, ensure_ascii=False, indent=0, sort_keys=False), encoding='utf-8', newline='\n')
    return out


def write_font(path):
    registry = load()
    font = TTFont(io.BytesIO(source()))
    shaper()
    order = font.getGlyphOrder()
    glyf, hmtx = font['glyf'], font['hmtx']
    new_names = []
    for run, cp in registry.items():
        name = f'word{cp:04X}'
        glyph = Glyph()
        glyph.numberOfContours = -1
        glyph.components = []
        pen = 0
        for gid, dx, dy, advance in shape_run(run):
            comp = GlyphComponent()
            comp.glyphName = order[gid]
            comp.x = pen + dx
            comp.y = dy
            comp.flags = 0x0002
            glyph.components.append(comp)
            pen += advance
        glyf.glyphs[name] = glyph
        hmtx.metrics[name] = (max(pen, 0), 0)
        new_names.append((name, cp))
    font.setGlyphOrder(order + [n for n, _ in new_names])
    for table in font['cmap'].tables:
        if table.isUnicode():
            for name, cp in new_names:
                table.cmap[cp] = name
    options = subset.Options()
    options.layout_features = []
    options.drop_tables += ['GSUB', 'GPOS', 'GDEF', 'DSIG']
    options.notdef_outline = True
    options.hinting = False
    options.glyph_names = False
    sub = subset.Subsetter(options)
    sub.populate(unicodes=[cp for _, cp in new_names])
    sub.subset(font)
    # The rows of text are as high as those of the Latin font, whatever the glyphs of a script reach:
    # the toolkit takes the height of a row from these (the Devanagari ones are a quarter taller).
    latin = TTFont(str(HERE.parent.parent / 'crates' / 'picasso-ui' / 'assets' / 'fonts' / 'NotoSans-Regular.ttf'))
    for table, names in (('hhea', ('ascent', 'descent', 'lineGap')), ('OS/2', ('sTypoAscender', 'sTypoDescender', 'sTypoLineGap', 'usWinAscent', 'usWinDescent'))):
        for name in names:
            setattr(font[table], name, getattr(latin[table], name))
    font.save(path)
    return len(new_names)
