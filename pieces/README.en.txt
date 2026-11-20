Piece images for ModeShift: Bipolar.

You can replace ALL pieces: chess pieces, checkers, queens (damka) and the
royal pawn. PNG and SVG (vector) are supported. If both .svg and .png with the
same name exist, the game uses .svg — it stays crisp at any size.

Chess pieces (6 per colour):
  white_pawn.png    white_knight.png    white_bishop.png
  white_rook.png    white_queen.png     white_king.png
  black_pawn.png    black_knight.png    black_bishop.png
  black_rook.png    black_queen.png     black_king.png
The same names can be used with .svg: white_pawn.svg, etc.

Checkers:
  white_checker.png / black_checker.png   — simple checker
  white_damka.png   / black_damka.png     — queen (optional)
  white_royal.png   / black_royal.png     — royal pawn (optional)
Or the same names as .svg.

If there is no queen image, the game draws a crown on top of the simple
checker; if there is no royal pawn image, it draws a dot. So you can replace
only the checkers and keep the drawn crowns.

PNG requirements: RGBA, transparent background, no square, text or watermarks.
Size — a 512×512 square (256×256 is fine), the piece centred, height about
85–90% of the canvas, the bottom of the piece on one line. No shadow is
needed: the game draws its own. White — light with a dark outline, black —
dark with a light outline.

SVG requirements: a normal SVG with a viewBox, no external links. The vector
scales automatically.

Put the files in the pieces folder next to shatmat.exe.
After adding or replacing files, restart the game.
