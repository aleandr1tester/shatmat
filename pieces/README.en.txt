Piece images for ShatMat.


WHAT IS HERE

  current.txt                 — selected set (written by the game)

  README.txt / README.en.txt  — this file

  <set>/                      — the old way: a set folder inside pieces/


THE STANDARD SET

The built-in pieces moved into a pack: packs/standard/pieces/. That pack is
shown in Settings as "Built-in" and serves as the template for your own set:
copy packs/standard to packs/my-set, replace the images and change name= in
pack.txt.

Files placed directly in this folder (pieces/) still take precedence over the
standard pack: handy for testing a single piece without building a pack. If
there is no white_king here, the standard pack is used.


SETS (PRESETS)

A set is a folder inside pieces/ with piece images. The game finds every
subfolder containing white_king.svg or white_king.png and lists it in
Settings under "PIECE SET". The folder name is the set name; a nicer label
comes from name.txt with a single line:

  name.txt:  Frogs 78

Sets made before packs existed keep working unchanged. New ones are easier to
ship as packs — see packs/README.txt.


RESOLUTION ORDER

  selected pack  ->  files in the pieces/ root  ->  the standard pack


FILE NAMES INSIDE A SET (exactly these)

Chess, 12 files:

  white_pawn   white_knight   white_bishop   white_rook   white_queen   white_king

  black_pawn   black_knight   black_bishop   black_rook   black_queen   black_king

Checkers, 6 optional:

  white_checker / black_checker   — plain checker

  white_damka   / black_damka     — damka (king checker)

  white_royal   / black_royal     — pawn-king

Extension: .svg (preferred, vector) or .png.


Missing files come from the standard pack. If a set has no checkers of its

own, checkers show the standard chess silhouettes: the nature icon does not

depend on the selected set.


IMAGE REQUIREMENTS

  PNG: RGBA, transparent background, no square/board, no text or watermarks,

       square 512x512 (256 minimum), piece centered, same baseline for all.

  SVG: a plain SVG with viewBox, no external links, no embedded rasters.

  Height: regular pieces ~84% of the canvas, pawn ~70% (it must be smaller),

  bottom of the piece at about 94% of the canvas height.

  Light art for white pieces, dark art for black ones: files are not recolored.

  No shadow needed: the game draws its own pulse.


The pulse of the side to move can be turned off in Settings: "Pulse: off".

Files go into the pieces folder next to shatmat.exe. A new set appears in

Settings right away; the selected set is applied on the next launch.
