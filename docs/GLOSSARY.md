# Глоссарий ShatMat

Единые термины для интерфейса, правил и гида на всех языках. Правило простое:
если термин есть здесь — в `lang/*.po` он переводится **только так**, без
синонимов. Это то, что можно проверить без носителя языка: одинаковость.

Проверка расхождений — `python tools/check_i18n.py`.

| Русский | English | Deutsch | Español | Français | Türkçe | Что означает |
|---|---|---|---|---|---|---|
| партия | game | Partie | partida | partie | parti | одна игра от старта до результата |
| ход | move | Zug | jugada | coup | hamle | смена стороны хода; в шашках серия взятий — один ход |
| полуход | half-move | Halbzug | medio movimiento | demi-coup | yarım hamle | один ход одной стороны |
| смена природы | nature shift | Naturwechsel | cambio de naturaleza | changement de nature | doğa değişimi | переход шахматы ⇄ шашки |
| сумерки | twilight | Dämmerung | crepúsculo | crépuscule | alacakaranlık | сама анимация перехода и пауза перед ней |
| фаза | phase | Phase | fase | phase | faz | отрезок партии в одной природе |
| шахматы (природа) | chess | Schach | ajedrez | échecs | satranç | игра по шахматным правилам |
| шашки (природа) | checkers | Damespiel | damas | dames | dama | игра по шашечным правилам |
| шашка | checker | Stein | ficha | pion | dama taşı | простая фигура в шашечной природе |
| пешка | pawn | Bauer | peón | pion | piyon | фигура-пешка (в шашках рисуется шашкой) |
| дамка | damka | Damka | damka | damka | damka | шашка, дошедшая до последнего ряда; летит по диагонали |
| король-шашка | king-checker | König-Stein | rey-ficha | roi-pion | şah-taşı | король в шашечной природе: ходит как дамка, его можно взять |
| конь | knight | Springer | caballo | cavalier | at | шахматный конь |
| слон | bishop | Läufer | alfil | fou | fil | шахматный слон |
| ладья | rook | Turm | torre | tour | kale | шахматная ладья |
| ферзь | queen | Dame | dama | dame | vezir | шахматный ферзь |
| король (шахматы) | king | König | rey | roi | şah | шахматный король; в шахматах его не берут — мат |
| взятие | capture | Schlag | captura | prise | vuruş | снятие чужой фигуры |
| серия взятий | capture chain | Schlagserie | cadena de capturas | enchaînement de prises | vuruş serisi | несколько взятий одной фигурой подряд |
| обязательное взятие | mandatory capture | Schlagzwang | captura obligatoria | prise obligatoire | zorunlu vuruş | если взятие есть, тихо ходить нельзя |
| коронация | promotion | Umwandlung | coronación | promotion | terfi | превращение шашки в дамку на последнем ряду |
| шах | check | Schach | jaque | échec | şah | король под боем (в шашках — сигнал «короля могут взять») |
| мат | checkmate | Matt | mate | mat | mat | королю некуда уйти — победа |
| ничья | draw | Remis | tablas | nulle | beraberlik | результат без победителя |
| код правил | rules code | Regelcode | código de reglas | code des règles | kural kodu | 4-символьный код настроек партии для обмена |
| журнал партии | game log | Partie-Log | registro de la partida | journal de la partie | parti günlüğü | файл с записью ходов и правил |
| просмотр | review | Ansicht | revisión | revue | inceleme | режим листания партии или лога |
| правила партии | game rules | Partieregeln | reglas de la partida | règles de la partie | parti kuralları | настройки, общие для обеих сторон |
| демонстрация | demo | Demo | demostración | démo | gösterim | партия ИИ против ИИ до старта |
| гид | guide | Leitfaden | guía | guide | rehber | экран обучения: правила, фигуры, примеры |
| просмотр лога | log review | Log-Ansicht | revisión del registro | revue du journal | günlük incelemesi | листание сохранённой партии |

## Как этим пользоваться

1. Перед добавлением новой строки в `lang/*.po` — свериться с таблицей.
2. Если термина нет, а он нужен в нескольких местах — сначала добавить его
   сюда (все шесть колонок), потом переводить строки.
3. Если перевод расходится с глоссарием — правится перевод, а не глоссарий:
   исключения фиксируются здесь отдельной пометкой.
