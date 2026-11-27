#!/usr/bin/env python3
"""Проверка переводов ShatMat без носителей языка.

Что ловит автоматически:
  1. строку из lang/msgids.txt, которой нет в каком-то языке;
  2. пустой перевод;
  3. расхождение подстановок `{name}` между русской строкой и переводом
     (иначе в интерфейсе покажется «{total}» вместо числа);
  4. русский текст, оставшийся в переводе;
  5. подозрительно длинный перевод (может не влезть в кнопку).

Чего не ловит: стиль и точность формулировок — это только вычитка человеком.

Запуск (Windows, Python из поставки DSH):
    python tools/check_i18n.py
Код возврата 1, если есть ошибки (пункты 1–3).
"""

import io
import os
import re
import sys

LANGS = ["en", "de", "es", "fr", "tr"]
# Строки журнала партии пишутся по-английски для всех не-русских локализаций,
# поэтому для de/es/fr/tr они не обязательны.
LOG_ONLY_PREFIXES = (
    "Партия «ShatMat»",
    "Версия:",
    "Начало:",
    "Окончание:",
    "Режим:",
    "Код партии",
    "Смена фаз:",
    "Автосмена сумерек",
    "Время фаз",
    "Обязательное взятие",
    "Коронация не-пешек",
    "Король уходит от боя",
    "Ничья без взятий",
    "Результат:",
    "Всего ходов",
    "Полных ходов",
    "№\tСторона",
)

CYR = re.compile(r"[А-Яа-яЁё]")
PH = re.compile(r"\{[a-z_]+\}")


def root_dir():
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def read(path):
    return io.open(path, encoding="utf-8", newline="").read().replace("\r\n", "\n")


def unescape(s):
    return s.replace("\\n", "\n").replace("\\t", "\t").replace('\\"', '"')


def parse_po(path):
    text = read(path)
    out = {}
    for m in re.finditer(r'msgid "(.*?)"\nmsgstr "(.*?)"', text, re.S):
        out[unescape(m.group(1))] = unescape(m.group(2))
    return out


def placeholders(s):
    return sorted(PH.findall(s))


def main():
    root = root_dir()
    expected = [unescape(l) for l in read(os.path.join(root, "lang", "msgids.txt")).splitlines() if l.strip()]
    po = {lang: parse_po(os.path.join(root, "lang", lang + ".po")) for lang in LANGS}

    errors = 0
    warnings = 0
    print(f"строк в msgids.txt: {len(expected)}\n")

    for lang in LANGS:
        data = po[lang]
        missing, empty, bad_ph, cyr_left, too_long = [], [], [], [], []
        for msgid in expected:
            if msgid not in data:
                if lang != "en" and msgid.startswith(LOG_ONLY_PREFIXES):
                    continue
                missing.append(msgid)
            elif not data[msgid].strip():
                empty.append(msgid)
        for msgid, translated in data.items():
            if not translated.strip():
                continue
            if placeholders(msgid) != placeholders(translated):
                bad_ph.append((msgid, translated))
            if CYR.search(translated) and CYR.search(msgid):
                cyr_left.append((msgid, translated))
            if len(msgid) > 8 and len(translated) > len(msgid) * 2.2:
                too_long.append((msgid, translated))

        print(f"=== {lang}: переводов {len(data)} ===")
        print(f"  нет перевода: {len(missing)}")
        for m in missing[:10]:
            print(f"    - {m[:80]}")
        print(f"  пустой перевод: {len(empty)}")
        print(f"  подстановки расходятся: {len(bad_ph)}")
        for m, t in bad_ph[:10]:
            print(f"    - {m[:60]!r} -> {t[:60]!r}")
        print(f"  остался русский текст: {len(cyr_left)}")
        for m, t in cyr_left[:10]:
            print(f"    - {m[:50]!r} -> {t[:60]!r}")
        print(f"  подозрительно длинно (>2.2x): {len(too_long)}")
        for m, t in too_long[:5]:
            print(f"    - {m[:50]!r}")
        errors += len(missing) + len(empty) + len(bad_ph)
        warnings += len(cyr_left) + len(too_long)
        print()

    print(f"итого: ошибок {errors}, предупреждений {warnings}")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
