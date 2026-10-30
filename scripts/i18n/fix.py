"""fix.py <file> <fixes>: replaces the `English|Translation` lines of <file> with those of <fixes>;
a fix with nothing after the bar removes the line (the English is kept)."""
import sys
path, fixes = sys.argv[1], sys.argv[2]
lines = open(path, encoding='utf-8').read().split('\n')
new = dict(l.split('|', 1) for l in open(fixes, encoding='utf-8').read().split('\n') if '|' in l)
out, done = [], 0
for l in lines:
    en = l.split('|', 1)[0] if '|' in l else None
    if en in new:
        done += 1
        if new[en]:
            out.append(f'{en}|{new.pop(en)}')
        else:
            new.pop(en)
        continue
    out.append(l)
open(path, 'w', encoding='utf-8', newline='\n').write('\n'.join(out))
print('fixed', done, 'unknown', list(new))
