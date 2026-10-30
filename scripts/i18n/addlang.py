"""Adds languages to the program: python scripts/i18n/addlang.py Danish:da:Dansk Finnish:fi:Suomi ...
(Variant:code:own name). The tables (crates/picasso-ui/src/i18n/<code>.rs) come from build.py first.
Each language goes before Chinese in the lists, so the Help > Language menu keeps Latin and Cyrillic scripts
together and the languages with a font of their own at the end."""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
MOD = ROOT / 'crates' / 'picasso-ui' / 'src' / 'i18n' / 'mod.rs'
MENU = ROOT / 'crates' / 'picasso-ui' / 'src' / 'menu.rs'


def edit(path, f):
    s = path.read_text(encoding='utf-8', newline='')
    crlf = '\r\n' in s
    s = f(s.replace('\r\n', '\n'))
    path.write_text(s.replace('\n', '\r\n') if crlf else s, encoding='utf-8', newline='')


def main(specs):
    langs = [x.split(':') for x in specs]
    k = len(langs)

    def mod(s):
        for variant, code, name in langs:
            if f'mod {code};' in s:
                continue
            s = s.replace('mod de;\n', f'mod de;\nmod {code};\n', 1) if False else s.replace('mod ar;\n', f'mod ar;\nmod {code};\n', 1)
            s = s.replace('    Chinese,\n    Japanese,', f'    {variant},\n    Chinese,\n    Japanese,', 1)
            s = s.replace('        Language::Chinese,\n        Language::Japanese,', f'        Language::{variant},\n        Language::Chinese,\n        Language::Japanese,', 1)
            s = s.replace('            Self::Chinese => "zh",', f'            Self::{variant} => "{code}",\n            Self::Chinese => "zh",', 1)
            s = s.replace('            Self::Chinese => "中文",', f'            Self::{variant} => "{name}",\n            Self::Chinese => "中文",', 1)
            s = s.replace('            Self::Chinese => (&zh::MENUS', f'            Self::{variant} => (&{code}::MENUS, {code}::ENTRIES, {code}::UI),\n            Self::Chinese => (&zh::MENUS', 1)
        n = int(re.search(r'ALL: \[Language; (\d+)\]', s).group(1))
        s = s.replace(f'ALL: [Language; {n}]', f'ALL: [Language; {n + k}]')
        s = s.replace(f'; {n}] =\n        [const {{ OnceLock::new() }}; {n}];', f'; {n + k}] =\n        [const {{ OnceLock::new() }}; {n + k}];')
        return s

    def menu(s):
        for variant, _, _ in langs:
            line = f'    language(crate::i18n::Language::{variant}),\n'
            if line not in s:
                s = s.replace('    language(crate::i18n::Language::Chinese),\n', line + '    language(crate::i18n::Language::Chinese),\n', 1)
        return s

    edit(MOD, mod)
    edit(MENU, menu)


if __name__ == '__main__':
    main(sys.argv[1:])
