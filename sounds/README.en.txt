Sounds for ModeShift: Bipolar.

Files (keep the same name, use either extension):
  move.wav / move.mp3          — piece move (a short "tick")
  capture.wav / capture.mp3    — piece capture
  twilight.wav / twilight.mp3  — twilight transition (a noise wave)

You can replace any file with your own WAV or MP3 sound.
The names must stay the same: move, capture, twilight.
If both .mp3 and .wav with the same name are present, the .mp3 plays.

WAV: 16-bit PCM, 22050 Hz or 44100 Hz, mono or stereo.
MP3: regular MPEG-1/2 Layer III, any bitrate works.
MP3 is decoded into memory when the sound is triggered.

The application takes sounds from the sounds folder next to shatmat.exe.
If a file is missing, the built-in variant plays.

The sound is read and played from memory without looping; service WAV
loop metadata is removed. After replacing a file, restart the game.
