# ShatMat — pieces change their nature

A board-game prototype: one board, two games. The pieces go through a twilight
transition — a wave from the centre of the board turns chess pieces into
checkers (and back), and the rules change with them. The mode changes by time
or by moves.

## How to play

- On launch you see the normal game screen, but the game has not started yet:
  the board is under a matte veil, the timers are stopped, and **Rules** and
  **Settings** are active on the right. Start the game with the **Play** button
  in the right panel or with the `Space` key; there is no separate centre
  button.
- If you pause, clicking the **PAUSE** plate resumes the game.
- The **Rules** window has tabs **Rules / Pieces**: general rules open by
  default, and the **Pieces** tab opens the reference.
- Click a piece to select it, click a highlighted square to make a move.
- `Space` — pause, `R` — new game, `H` — rules, `Esc` — clear selection.
  Text in the rules window can be selected and copied with the mouse.
- Only the outline of the side to move pulses softly, and only on pieces that
  have a legal move: blocked pieces and the opponent's pieces do not blink.
  All pieces stay in place — there is no global "breathing" or size change.
- Moves and captures have short sounds; twilight has a noise wave. Each sound
  plays once and does not loop; the long twilight sound is not restarted while
  it is still playing.
- A capture uses the capture sound, not the move sound. Twilight plays on its
  own audio channel, so a move or capture does not cut it off.
- Sounds and music can be enabled or disabled in **Settings**.
- Your own sounds can be placed in the `sounds` folder next to the program:
  `move`, `capture`, `twilight` as `.wav` or `.mp3` (if both exist, `.mp3` wins).
- The right panel shows: the current phase, the countdown, whose turn it is,
  the move log, phase times and the opponent. By default you play against the
  computer as White; switch to two-player play with the opponent chip in the
  panel.
- The **Pieces** button in the **Rules** window opens the reference: first
  choose the game (Chess/Checkers), then a piece. On the left is the piece
  image, on the right are the **How it moves** and **How it captures**
  sections, and at the bottom are a mini-board and the **Move demo** /
  **Capture demo** buttons with animation. You can return to the rules from the
  reference with the **Rules** button.

## Settings

The **Settings** button in the panel:

- enable or disable sounds;
- sound source: built-in or WAV/MP3 from the `sounds` folder;
- enable or disable music; `music/theme.wav` plays in a loop on a separate
  channel and is not interrupted by move sounds. When the track ends it starts
  again automatically;
- pawn promotion: **Always queen** (default) or **Choose piece**;
- phase change: **By time** or **By moves**;
  - **By time**: **Fixed** (by default chess 20 s, checkers 10 s) or
    **Random** — then each change lasts a random interval from 10 s to the
    chosen maximum (up to 60 s), and the next interval differs from the
    previous one;
  - **By moves**: within one move there is always one game; after a move the
    phase may stay or change at random. Each move has a time of 10–180 s;
    when it runs out, the move passes to the opponent. The large panel timer
    shows the remaining time, and the caption below it reads "the move will
    pass to the opponent";
- mandatory capture in checkers: on or off;
- piece icons on checkers: king with a crown, knight, bishop, rook, queen;
  pawns stay without icons;
- crowning of non-pawns: **Queens: all** (default) or **Queens: pawns**;
- mate trajectories: separate **Attack** (red lines to the king) and
  **Surrounding** (yellow lines of pieces covering escape squares); both are
  on by default;
- interface font: Noto Sans by default (Cyrillic + Latin, including Turkish
  letters); the **Font** button cycles the built-in Noto Sans and custom fonts
  from the `fonts` folder;
- move smoothness: from 0.05 to 0.60 seconds.

## Rules code

The settings show the code of the current rules — 8 characters like
`S3HW-5RTD`. It packs the phase switching (by time or by moves), phase
durations, move limit, pawn promotion, mandatory capture and the damka mode.

- the **Copy** button puts the code into the clipboard;
- the code can be shared with another player and applied in the "rules code"
  field — **Apply** sets all parameters at once;
- a corrupted code (a wrong character) is rejected by the checksum: the game
  reports "Invalid code" and keeps the rules unchanged.

## Custom pieces and packs

The standard set lives in the pack `packs/standard/pieces/` — that is the
"Built-in" option in Settings. It is a pack like any other, so it is easy to
use as a template: copy `packs/standard` to `packs/my-set`, replace the images
and change `name=` in `pack.txt`.

**A pack** is a folder `packs/<name>/` with a `pack.txt` inside, containing
`pieces/`, `sounds/` and `music/` (any one of them is enough). In Settings a
pack appears as a separate option in each list: you can take the pieces from a
pack and keep the built-in sounds, or the other way round.

There is no separate `pieces/` folder next to the game any more: every set is
a pack. If you still have an old `pieces/` folder with files or set subfolders,
it keeps working: the selected pack wins, then files in `pieces/`, then the
standard pack.

Requirements: PNG RGBA with a transparent background, a 512×512 square
(256×256 is fine), the piece centred, height about 85–90% of the canvas, no
shadow, square, text or watermarks. Names: `white_pawn.png` …
`white_king.png` and the same `black_*.png`; `.svg` works too and is
preferred. The full list of names and requirements is in `packs/README.txt`.

You can also use vector pieces: place `.svg` files with the same names
(`white_pawn.svg`, …). The game prefers SVG over PNG. SVG is rendered at any
size without quality loss. Checkers can be replaced too:
`white_checker.svg`, `black_checker.svg`, and optionally
`white_damka.svg`, `black_damka.svg`, `white_royal.svg`, `black_royal.svg`
(the royal pawn). If a damka or royal image is missing, the crown or dot is
drawn on top of the normal checker.

## Custom fonts

The default interface font is **Noto Sans**: it covers Cyrillic and Latin,
including Turkish letters, and reads noticeably better at small sizes. Place
`.ttf` or `.otf` files in the `fonts` folder next to the program and restart the
game. In the settings press the **Font** button to cycle through fonts: the
built-in Noto Sans and custom fonts go in a circle. Details are in
`fonts/README.txt`.

## Logs and journal

Log recording is on by default. Files are stored in the `logs` folder next to
`shatmat.exe`: one text file per game. It contains the start and end time, the
mode, phase times, the result, every move (side, piece, coordinates, capture,
phase), the total number of moves and separately for White and Black. An
interrupted game is saved too, with a note.

In the settings you can disable recording: **Game logs**.

After a game, the result card has a **Journal** button — a window with the move
journal; the text can be selected and copied. Next to it is **Review**: a list
of all half-moves; clicking a move shows it on the board with animation; there
are **Back** and **Forward** buttons.

## After the game

The result card is translucent and sits above the panel without covering the
board. On the board, the last capture is shown in slow motion: who captured
whom and in which mode — chess or checkers. On mate, attack lines to the king
are shown on the board, and the card lists the pieces that delivered mate; this
can be disabled in the **Mate · trajectories** setting.

## Modes

- **Two players** — two players at one board, taking turns.
- **Against the computer** — you play White, the computer answers with Black.
  Before its move it pauses for 2–3 seconds and highlights the piece it is
  going to capture (a red ring and a line from its piece). While the computer
  is moving, the phase timer is stopped; twilight waits for its move to finish.
  In checkers the computer is careful with the king: it does not put it under
  attack if there is a safe move, and it values the loss of the king very
  highly. Difficulty is chosen there as well: **easy** (moves almost
  randomly), **medium** (looks two half-moves ahead) and **hard** (three, with
  pruning). The computer thinks for half a second and respects all phase
  rules, including mandatory captures and jump chains.
- **Over the network** — hidden until **Network game** is enabled in the
  settings. The game itself becomes the server: port 47771 opens
  automatically, nothing to configure. Players in the same local network find
  each other on their own; you can also connect by `IP:port` (for play over
  the internet the host needs a public IP or a forwarded port). The invite
  code in the network window is `XXXX-XXXX-XXXX`: the first two blocks are
  the network ID, the third is a compact block of the current rules. The
  opponent pastes the code into the connect field: the game finds the peer
  on the network and shows their rules in advance. The connection
  is encrypted (Noise_XX + ChaCha20-Poly1305); keys live in `keys.bin` next
  to the game. The host plays White and sets the rules — the opponent receives
  them as a code when connecting. Before the game there is a short greeting,
  during it — a chat (lives 60 seconds and is never logged). Moves are checked
  against a position hash: if they diverge, the game stops. The connection
  is maintained automatically: a ping every 5 seconds detects a drop, and
  for 60 seconds after it both sides try to restore the connection on their
  own — the game then continues where it stopped, nothing to press.

## Starting position

The game starts from the classic setup: 16 pieces each — 8 pawns and 8 pieces
(rooks, knights, bishops, queen and king).

## Rules

### When the pieces are chess

- Classic moves: pawn, knight, bishop, rook, queen, king.
- No castling and no en passant.
- A pawn on the last rank becomes a queen by default. Choosing the piece
  (queen, rook, bishop, knight) is enabled in the settings.
- If a pawn ends up on the last rank after a transition from checkers, it also
  becomes a queen; with choosing enabled the game will offer a piece.
- Check and mate work: the king cannot be left under attack. In the chess
  phase the king cannot be captured — it is check or mate; if after twilight
  only the king that just moved is in check, the move is passed to it so it can
  escape.
- Stalemate is not a draw: twilight starts immediately and the fight goes on.

### When the pieces are checkers

- Simple checkers (at the start of the checkers phase — all pieces) move
  diagonally forward and backward, and capture in any direction. This is the
  hybrid difference from traditional Russian checkers, where a simple checker
  moves only forward.
- A checker stays on squares of its own colour: a diagonal move always keeps
  the square colour.
- You may move into attack if there is no mandatory capture.
- At the start of the checkers phase there are no queens. A simple checker
  that reaches the last rank becomes a queen; queens fly diagonally any
  distance. With **Queens: all** on, pieces that already stand on the last
  rank become queens right away when the phase flips to checkers.
- The king in the checkers phase is an ordinary checker with a small dot; it
  can be told apart, captured and defeated.
- Capturing is mandatory; a capture chain continues while there is a target.
  A checker that reaches the last rank during a chain becomes a queen and
  continues the chain as a queen.
- Mandatory capture can be disabled in the settings: then you can move without
  capturing, and a started chain can be finished with the **Finish** button.
- If, with mandatory capture on, the only move is a capture, the checker that
  must capture pulses with a red ring; **FIGHT** lights up in the panel.
- By default (**Queens: all**) any piece can become a queen, including the
  king, knight, bishop, rook and queen: both by reaching the last rank and by
  standing on it when the phase flips to checkers. **Queens: pawns** leaves
  crowning only to pawns.
- In the **Pieces** reference checkers are described in three groups:
  **Pawns** (the pawn and all pieces with chess marks — the marks can be
  hidden in the settings), **Pawn-king** (an ordinary-looking pawn with a
  circle, but losing it means defeat, so watch it especially carefully) and
  **Queen**.
- If there is no move, the turn passes to the opponent. If both are stuck,
  twilight starts immediately and temporarily brings chess, to get out of the
  dead end.

### Twilight and mode change

- The transition lasts 1.7 seconds: moves freeze, a wave of light goes from the
  centre to the edges, and pieces flow into each other. Twilight does not start
  while a move animation, a capture chain or the computer's move is in
  progress: the move finishes first, then the phase changes.
- **By moves**: within one move the game does not change; after a move the
  phase may stay the same or change — at random. Each move has a time of
  10–180 s; when it runs out, the move passes to the opponent.
- In the last 5 seconds before the change, the timer waits while you hold a
  piece: the move finishes and shows its result before the game changes.
- After a move twilight appears with a 2-second pause, so the move result is
  visible before the game changes. If you already hold a piece, the pause
  waits for the move to finish and starts counting again. After twilight the
  computer waits at least 3 seconds before its move.
- The transition waits until the player finishes the capture chain.
- If both are stuck in the checkers phase, the transition starts immediately —
  this is a temporary exit to chess; after it the normal cycle continues.
- Victory: mate, a captured king, or the opponent has no pieces left.

## Download

The ready Windows build is on the releases page:
https://github.com/aleandr1tester/shatmat/releases/latest

## Build and run

The project lives in the DSH working folder:
`<home folder>\Documents\deepseek-harness\default-workspace\shatmat`.
Windows does not let the compiler write to `Documents`, so the local
`.cargo/config.toml` (not in git) directs the build to `AppData\Local`.

```sh
cargo run --release
```

A window snapshot for checking (it is better to put the path outside
Documents):

```sh
cargo run --release -- --screenshot "$env:TEMP/now.png"
```

In snapshot mode the window opens without focus and behind other windows,
draws about 120 frames, saves a PNG and closes.

Rules check:

```sh
cargo test
```


The application icon is `assets/icon.ico` (nine sizes, transparent
background). Regenerate it with `python tools/make_icon.py`; it is
embedded into the exe automatically by `build.rs` (needs `windres`
from MinGW; without it the build simply has no icon). The window icon
comes from `assets/icon-256.png`.
## Versions

The history of the game and all additions is in git:

```sh
git log --oneline
git show --stat HEAD
```

## Structure

- `src/game.rs` — rules: moves, captures, promotions, game statuses.
- `src/art.rs` — phase palettes, vector pieces and checkers, effects.
- `src/sound.rs` — move, capture and twilight sounds; reads the `sounds`
  folder.
- `src/logging.rs` — text game logs and the move journal.
- `src/main.rs` — window, layout, panel, move animation and replay, snapshot.
- `src/i18n.rs` — interface localisation (ru, en, es, de, fr, tr).
- `sounds/` — replaceable WAV/MP3 files: `move`, `capture`, `twilight`
  (same names, either extension).
- `music/` — `theme.wav` for background music.
- `packs/` — packs: pieces, sounds and music in one set. `packs/standard` is
  the standard set ("Built-in") and the template for your own pack (see
  `packs/README.txt`).
- `lang/` — interface translations: `ru.po`, `en.po`, `es.po`, `de.po`,
  `fr.po`, `tr.po` (or `.mo`).
- `logs/` — game log files (see `logs/README.txt`).

## License

The source is open but **noncommercial**: [PolyForm Noncommercial 1.0.0](LICENSE).
You may use, copy, modify and distribute the game for personal, educational,
research and charitable purposes. Commercial use requires a separate permission
from the author.

Contributions are accepted under [CLA.md](CLA.md): by sending a patch you allow
relicensing of the project. Third-party fonts, sounds and libraries are listed
in [THIRD-PARTY.md](THIRD-PARTY.md).

## Next

This is a prototype for approval. The next steps are a web version for quick
access and a mobile version on Godot, once the rules and look are approved.
