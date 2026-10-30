"""Turns scripts/i18n/<code>.txt (menu bar names, then `English|Translation` lines) into
crates/picasso-ui/src/i18n/<code>.rs, and checks the fit rules the menus need:

- a menu bar name is at most MENU_MAX characters (the title row has a fixed slot for each);
- an entry is at most 6 characters or a third longer than the English one (20 characters always fit: a
  menu is that wide before it has a label), and never over 48.
"""
import sys, pathlib
from PIL import ImageFont

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = ROOT / 'scripts' / 'i18n'
OUT = ROOT / 'crates' / 'picasso-ui' / 'src' / 'i18n'
ENGLISH = (HERE / 'labels.txt').read_text(encoding='utf-8').split('\n')
MENU_MAX = [5, 5, 5, 9, 5, 5, 5, 9, 7, 5, 8, 11, 8]
MENU_NAMES = ['File', 'Edit', 'Trim', 'Timeline', 'Clip', 'Mark', 'View', 'Playback',
              'Effects', 'Color', 'Audio', 'Workspace', 'Help']


FONT = ImageFont.truetype(str(ROOT / 'crates' / 'picasso-ui' / 'assets' / 'fonts' / 'NotoSans-Regular.ttf'), 14)


def width(text):
    return FONT.getlength(text)


# Words that have room where the pages draw them (the page tabs along the bottom, the buttons of the
# toolbars, the rows of a list): a translation may be up to twice as wide there.
ROOMY = {
    'Cut', 'Edit', 'Deliver', 'Color', 'Effects', 'Audio', 'Media', 'Timeline', 'Mixer', 'Metadata',
    'Media Pool', 'Inspector', 'Fusion', 'Photo', 'Fairlight', 'Nodes', 'Titles', 'Lightbox',
    'Sound Library', 'Color Bars', 'Half', 'Eighth', 'Teal', 'Default', 'Off',
}


def fits_page(english, shown):
    # Text on a page is made smaller down to 3/4 when it is wider than the English; wider than that
    # (a third more) would run into its neighbour.
    if english in ROOMY:
        return width(shown) <= width(english) * 2.0 + 10
    return width(shown) <= width(english) * 1.15 + 4


def entry_max(english):
    return min(max(len(english) + 6, -(-len(english) * 135 // 100), 20), 48)


def build(code):
    lines = (HERE / f'{code}.txt').read_text(encoding='utf-8').split('\n')
    assert lines[0] == '#menus', code
    i = lines.index('#entries')
    u = lines.index('#ui') if '#ui' in lines else len(lines)
    menus = [l for l in lines[1:i] if l]
    assert len(menus) == 13, (code, len(menus))
    problems = []
    for name, short, most in zip(MENU_NAMES, menus, MENU_MAX):
        if len(short) > most:
            problems.append(f'menu {name} -> {short!r} is {len(short)} > {most}')
    pairs = []
    seen = set()
    entry_lines = list(lines[i + 1:u])
    for extra in sorted(HERE.glob(f'{code}.entries*.txt')):
        entry_lines += extra.read_text(encoding='utf-8').split('\n')
    latest = {}
    for l in entry_lines:
        if '|' in l:
            en, tr = l.split('|', 1)
            latest[en] = tr
    for en, tr in latest.items():
        assert en in ENGLISH, (code, en)
        seen.add(en)
        if tr != en and len(tr) > entry_max(en):
            problems.append(f'{en!r} -> {tr!r} is {len(tr)} > {entry_max(en)}')
        pairs.append((en, tr))
    missing = [e for e in ENGLISH if e not in seen]
    ui = []
    ui_lines = []
    for extra in sorted(HERE.glob(f'{code}.ui*.txt')):
        ui_lines += extra.read_text(encoding='utf-8').split('\n')
    if not ui_lines:
        ui_lines = lines[u + 1:]
    latest_ui = {}
    for l in ui_lines:
        if '|' in l:
            en, tr = l.split('|', 1)
            latest_ui[en] = tr
    ui_lines = [f'{en}|{tr}' for en, tr in latest_ui.items()]
    collected = set((HERE / 'ui_labels.txt').read_text(encoding='utf-8').split('\n'))
    collected |= set((HERE / 'ui_more.txt').read_text(encoding='utf-8').split('\n'))
    on_pages = set(collected)
    collected |= set(ENGLISH)
    for l in ui_lines:
        if not l.strip():
            continue
        en, tr = l.split('|', 1)
        if not fits_page(en, tr):
            problems.append(f'ui text {en!r} -> {tr!r} is {width(tr):.0f}px, the English {width(en):.0f}px')
        if en not in collected:
            problems.append(f'ui text {en!r} is not shown anywhere (scripts/i18n/ui_labels.txt)')
        ui.append((en, tr))
    # A menu word that is also text on a page (Cut, Edit, Media...) is drawn there too, so it has to fit
    # there, unless the page text has a short form of its own.
    own = {en for en, _ in ui}
    for en, tr in latest.items():
        if en in on_pages and en not in own and not fits_page(en, tr):
            problems.append(f'page text {en!r} -> {tr!r} is {width(tr):.0f}px, the English {width(en):.0f}px (from the menus)')
    return menus, pairs, ui, problems, missing


def rust(code, menus, pairs, ui):
    esc = lambda s: s.replace(chr(92), chr(92) * 2).replace(chr(34), chr(92) + chr(34))
    out = [f'//! The menus in {code}. Made from scripts/i18n/{code}.txt by scripts/i18n/build.py.', '']
    out.append('pub(super) const MENUS: [&str; 13] = [')
    out += [f'    "{esc(m)}",' for m in menus]
    out.append('];\n')
    out.append('pub(super) const ENTRIES: &[(&str, &str)] = &[')
    out += [f'    ("{esc(e)}", "{esc(t)}"),' for e, t in pairs]
    out.append('];\n')
    out.append('pub(super) const UI: &[(&str, &str)] = &[')
    out += [f'    ("{esc(e)}", "{esc(t)}"),' for e, t in ui]
    out.append('];')
    return '\n'.join(out) + '\n'


if __name__ == '__main__':
    bad = False
    OUT.mkdir(parents=True, exist_ok=True)
    for code in sys.argv[1:]:
        menus, pairs, ui, problems, missing = build(code)
        print(code, len(pairs), 'entries,', len(missing), 'not translated,', len(ui), 'texts of the pages')
        for p in problems:
            print('  TOO LONG:', p)
            bad = True
        (OUT / f'{code}.rs').write_text(rust(code, menus, pairs, ui), encoding='utf-8', newline='\n')
    sys.exit(1 if bad else 0)
