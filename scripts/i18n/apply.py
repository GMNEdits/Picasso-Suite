"""apply.py <code> <file>: replaces `English|Translation` lines of scripts/i18n/<code>.txt
with those of <file> (a fix-up list)."""
import sys
code, fixes = sys.argv[1], sys.argv[2]
p = f'scripts/i18n/{code}.txt'
lines = open(p, encoding='utf-8').read().split('\n')
new = dict(l.split('|', 1) for l in open(fixes, encoding='utf-8').read().split('\n') if '|' in l)
done = 0
for i, l in enumerate(lines):
    if '|' in l:
        en = l.split('|', 1)[0]
        if en in new:
            lines[i] = f'{en}|{new.pop(en)}'
            done += 1
open(p, 'w', encoding='utf-8', newline='\n').write('\n'.join(lines))
print('replaced', done, 'unknown', list(new))
