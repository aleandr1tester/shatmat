# Карта кода ShatMat

Ориентир по файлам и символам. Номеров строк здесь нет специально: они
протухают после первой же правки. Ищите по имени символа через `grep`
(или `Select-String` в PowerShell) — так надёжнее.

## Модули

| Файл | За что отвечает |
| --- | --- |
| `src/main.rs` | Интерфейс egui/eframe: экраны, отрисовка доски, панели, настройки, плашка результата. Все тексты интерфейса — здесь. |
| `src/game.rs` | Правила: доска, ходы, фазы и сумерки, статусы партии, сериализация состояния, unit-тесты правил. |
| `src/ai.rs` | ИИ: negamax, оценка позиции с учётом фаз. |
| `src/arena.rs` | Турнир ИИ против ИИ без окна (`--arena`): сетка длин фаз, CSV с метриками. |
| `src/net.rs` | Сетевой протокол: сессии, обмен ходами и состоянием. |
| `src/rulecode.rs` | Коды правил `XXXX-XXXX` и компактный код. |
| `src/logging.rs` | Журнал партии и демо-логи (`logs/`, `logs/demo/`). |
| `src/i18n.rs` | gettext-локализация: `lang/*.po`, функции `tr`/`tr_owned`/`log`. |
| `src/art.rs` | Отрисовка фигур (SVG/канвас), стили. |
| `src/sound.rs`, `src/music.rs` | Звуки ходов/взятий и музыка. |
| `src/updater.rs` | Проверка обновлений. |
| `src/fonts.rs` | Шрифты. |
| `build.rs` | Встраивание иконки в exe через `windres`. |
| `tools/make_icon.py` | Генерация `assets/icon.ico` и `icon-256.png`. |

## Ядро правил (`src/game.rs`)

**Типы:** `Side`, `Kind`, `Phase`, `Piece`, `Move { from, to, captured }`,
`MoveRecord { side, kind, damka, from, to, captured, phase }`,
`Status { Playing, Check(side), Win { side, reason }, Draw { reason } }`,
`Game` (поле `board: [[Option<Piece>; 8]; 8]`, `turn`, `phase`, `status`, `log`).

**Ключевые методы `Game`:**

- Ход: `click`, `select`, `deselect`, `moves_for`, `all_moves`, `apply`,
  `end_chain`, `choose_promotion`.
- Проверки: `in_check`, `king_sq`, `attackers_to`, `count`, `material_diff`,
  `king_capture_available`, `king_under_attack`, `no_capture_plies`.
- Фазы/сумерки: `tick`, `flip_phase`, `refresh_status`,
  `resolve_transition_check`, `promote_pawns_after_flip`, `settle_twilight`,
  `start_stalemate_twilight`, `start_rescue_twilight`, `twilight_should_wait`.
- Настройки фазы: `set_phase_moves`, `set_twilight_by_moves`, `set_len`,
  `set_move_time_limit`, `refresh_phase_timer`, `random_phase_len`.
- Сеть/реконнект: `export_state`, `import_state`.

**Поток:** `apply` применяет ход и зовёт `refresh_status`; `tick` отсчитывает
время, при `trans` завершает переход и зовёт `flip_phase`, а тот —
`refresh_status` уже в новой фазе.

**Важно:** мат, найденный внутри `flip_phase`, помечается причиной
`"мат после сумерек"` (флаг `twilight_flip`), потому что последний ход был
в прежней игре.

## Константы правил (в начале `src/game.rs`)

`TRANS_SECS`, `MOVE_ANIM_SECS`, `MIN_PHASE`/`MAX_PHASE`,
`MIN_MOVE_SECS`/`MAX_MOVE_SECS`, `GRACE_SECS`, `MOVE_TWILIGHT_PAUSE`,
`TWILIGHT_HOLD_MAX`, `NO_CAPTURE_DRAW_PLIES`.

## Причины результата

Строки задаются в `refresh_status` (`src/game.rs`), а показываются уже
готовыми: плашка результата в `src/main.rs` (поиск `Status::Win`),
журнал в `src/logging.rs`. Добавили новую причину — добавьте её msgid
в `lang/*.po` и `lang/msgids.txt`; полноту проверяет тест
`i18n::tests::bundled_po_covers_all_msgids`.

## Где что менять

| Задача | Куда смотреть |
| --- | --- |
| Текст/подпись в интерфейсе | `src/main.rs`, затем `lang/*.po` + `lang/msgids.txt` |
| Новое правило или причина результата | `src/game.rs` (`refresh_status`, `Status`), затем `lang/` |
| Журнал партии, демо-логи | `src/logging.rs` |
| Фигуры, иконки на доске | `src/art.rs`, `pieces/`, `packs/` |
| Звук и музыка | `src/sound.rs`, `src/music.rs`, `packs/*/sounds`, `packs/*/music` |
| Сетевые сообщения | `src/net.rs` |
| Код правил | `src/rulecode.rs` |

## Тесты

- Unit-тесты лежат внутри модулей (`mod tests`): `game.rs`, `i18n.rs`,
  `ai.rs`, `rulecode.rs`, `logging.rs`, `sound.rs`, `updater.rs`, `main.rs`.
- Весь набор: `cargo test --offline`.
- Временные реплеи партий из логов удобно генерировать скриптом в `shots/`
  и не коммитить.

## Сборка

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:CARGO_TARGET_DIR = "C:\Users\quasiroot\AppData\Local\shatmat-target"
$env:CARGO_BUILD_JOBS = "1"
cargo build --release --offline
```

`build.rs` ищет `windres.exe` в PATH и типовых путях MinGW; без него
сборка проходит, но exe остаётся без иконки.