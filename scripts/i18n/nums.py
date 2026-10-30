"""Turns scripts/i18n/<code>.nums into <code>.txt and <code>.ui.txt, the files build.py reads.

A .nums file has the 13 menu names under `#menus`, then `N|translation` lines under `#entries` (N is the
line of keys_entries.txt, the menu labels) and under `#ui` (N is the line of keys_ui.txt, the text of the
pages). The numbers are checked: each one is used once, and none is out of range.
"""
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent


def convert(code):
    ent = (HERE / 'keys_entries.txt').read_text(encoding='utf-8').split('\n')
    ui = (HERE / 'keys_ui.txt').read_text(encoding='utf-8').split('\n')
    lines = (HERE / f'{code}.nums').read_text(encoding='utf-8').split('\n')
    section = None
    menus, entries, uis = [], {}, {}
    for line in lines:
        if line in ('#menus', '#entries', '#ui'):
            section = line
            continue
        if not line.strip():
            continue
        if section == '#menus':
            menus.append(line.strip())
        else:
            n, text = line.split('|', 1)
            target = entries if section == '#entries' else uis
            n = int(n)
            assert n not in target, (code, section, n, 'twice')
            assert 1 <= n <= (len(ent) if section == '#entries' else len(ui)), (code, section, n)
            target[n] = text
    assert len(menus) == 13, (code, len(menus))
    out = ['#menus'] + menus + ['#entries'] + [f'{ent[n - 1]}|{t}' for n, t in sorted(entries.items())]
    (HERE / f'{code}.txt').write_text('\n'.join(out) + '\n', encoding='utf-8', newline='\n')
    (HERE / f'{code}.ui.txt').write_text('\n'.join(f'{ui[n - 1]}|{t}' for n, t in sorted(uis.items())) + '\n', encoding='utf-8', newline='\n')
    return len(entries), len(uis)


def dialogs(code):
    dlg = HERE / f'{code}.dlg'
    if not dlg.exists():
        return
    nl = chr(10)
    keys = (HERE / 'keys_dialogs.txt').read_text(encoding='utf-8').split(nl)
    done = {}
    for line in dlg.read_text(encoding='utf-8').split(nl):
        if '|' in line:
            n, text = line.split('|', 1)
            assert int(n) not in done and 1 <= int(n) <= len(keys), (code, n)
            assert text.count('{}') == keys[int(n) - 1].count('{}'), (code, n, 'pieces')
            done[int(n)] = text
    out = nl.join(f'{keys[n - 1]}|{t}' for n, t in sorted(done.items())) + nl
    (HERE / f'{code}.ui9.txt').write_text(out, encoding='utf-8', newline=nl)


if __name__ == '__main__':
    for code in sys.argv[1:]:
        if (HERE / f'{code}.nums').exists():
            print(code, convert(code))
        dialogs(code)
