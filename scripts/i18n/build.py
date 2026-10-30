"""Turns scripts/i18n/<code>.txt (menu bar names, then `English|Translation` lines) into
crates/picasso-ui/src/i18n/<code>.rs, and checks the fit rules the menus need:

- a menu bar name is at most MENU_MAX characters (the title row has a fixed slot for each);
- an entry is at most 6 characters or a third longer than the English one (20 characters always fit: a
  menu is that wide before it has a label), and never over 48.
"""
import sys, pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = ROOT / 'scripts' / 'i18n'
OUT = ROOT / 'crates' / 'picasso-ui' / 'src' / 'i18n'
ENGLISH = (HERE / 'labels.txt').read_text(encoding='utf-8').split('\n')
MENU_MAX = [5, 5, 5, 9, 5, 5, 5, 9, 7, 5, 8, 11, 8]
MENU_NAMES = ['File', 'Edit', 'Trim', 'Timeline', 'Clip', 'Mark', 'View', 'Playback',
              'Effects', 'Color', 'Audio', 'Workspace', 'Help']


def entry_max(english):
    return min(max(len(english) + 6, -(-len(english) * 135 // 100), 20), 48)


def build(code):
    lines = (HERE / f'{code}.txt').read_text(encoding='utf-8').split('\n')
    assert lines[0] == '#menus', code
    i = lines.index('#entries')
    menus = [l for l in lines[1:i] if l]
    assert len(menus) == 13, (code, len(menus))
    problems = []
    for name, short, most in zip(MENU_NAMES, menus, MENU_MAX):
        if len(short) > most:
            problems.append(f'menu {name} -> {short!r} is {len(short)} > {most}')
    pairs = []
    seen = set()
    for l in lines[i + 1:]:
        if not l.strip():
            continue
        en, tr = l.split('|', 1)
        assert en in ENGLISH, (code, en)
        assert en not in seen, (code, 'twice', en)
        seen.add(en)
        if tr != en and len(tr) > entry_max(en):
            problems.append(f'{en!r} -> {tr!r} is {len(tr)} > {entry_max(en)}')
        pairs.append((en, tr))
    missing = [e for e in ENGLISH if e not in seen]
    return menus, pairs, problems, missing


def rust(code, menus, pairs):
    esc = lambda s: s.replace(chr(92), chr(92) * 2).replace(chr(34), chr(92) + chr(34))
    out = [f'//! The menus in {code}. Made from scripts/i18n/{code}.txt by scripts/i18n/build.py.', '']
    out.append('pub(super) const MENUS: [&str; 13] = [')
    out += [f'    "{esc(m)}",' for m in menus]
    out.append('];\n')
    out.append('pub(super) const ENTRIES: &[(&str, &str)] = &[')
    out += [f'    ("{esc(e)}", "{esc(t)}"),' for e, t in pairs]
    out.append('];')
    return '\n'.join(out) + '\n'


if __name__ == '__main__':
    bad = False
    OUT.mkdir(parents=True, exist_ok=True)
    for code in sys.argv[1:]:
        menus, pairs, problems, missing = build(code)
        print(code, len(pairs), 'entries,', len(missing), 'not translated')
        for p in problems:
            print('  TOO LONG:', p)
            bad = True
        (OUT / f'{code}.rs').write_text(rust(code, menus, pairs), encoding='utf-8', newline='\n')
    sys.exit(1 if bad else 0)
