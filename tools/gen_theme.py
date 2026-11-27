#!/usr/bin/env python3
"""Генератор фоновой темы ShatMat «Сумерки».

Синтезирует бесшовный WAV-луп без чужих сэмплов: только синусы, поэтому
файл принадлежит проекту и не тянет за собой чужую лицензию.

Запуск:
    python tools/gen_theme.py music/theme.wav

Формат: моно, 16 бит, 22050 Гц — как ждёт MCI на Windows.
"""

import sys
import wave

import numpy as np

RATE = 22050
SECONDS = 16.0
FADE = 0.6  # секунды кроссфейда для бесшовного стыка


def sine(freq: float, n: int, phase: float = 0.0) -> np.ndarray:
    t = np.arange(n) / RATE
    return np.sin(2.0 * np.pi * freq * t + phase)


def adsr(n: int, attack: float, release: float) -> np.ndarray:
    env = np.ones(n, dtype=np.float64)
    a = max(1, int(attack * RATE))
    r = max(1, int(release * RATE))
    env[:a] = np.linspace(0.0, 1.0, a)
    env[-r:] *= np.linspace(1.0, 0.0, r)
    return env


def note(name: str) -> float:
    names = {"C": -9, "C#": -8, "D": -7, "D#": -6, "E": -5, "F": -4,
             "F#": -3, "G": -2, "G#": -1, "A": 0, "A#": 1, "B": 2}
    pitch, octave = name[:-1], int(name[-1])
    return 440.0 * 2.0 ** ((names[pitch] + (octave - 4) * 12) / 12.0)


def main(path: str) -> None:
    total = int(SECONDS * RATE)
    extra = int(FADE * RATE)
    buf = np.zeros(total + extra, dtype=np.float64)

    # Аккорды: Dm — Bb — F — C, по 4 секунды. Нижняя октава и квинта.
    chords = [
        ("D3", "A3", "D4"),
        ("A#2", "F3", "A#3"),
        ("F2", "C3", "F3"),
        ("C3", "G3", "C4"),
    ]
    seg = int(4.0 * RATE)
    for i, (root, fifth, octv) in enumerate(chords):
        start = i * seg
        n = min(seg, total + extra - start)
        if n <= 0:
            continue
        env = adsr(n, attack=0.9, release=0.9)
        part = np.zeros(n, dtype=np.float64)
        for name, amp, detune in [(root, 0.30, 1.0), (root, 0.16, 1.003),
                                  (fifth, 0.20, 0.998), (octv, 0.12, 1.001)]:
            f = note(name) * detune
            part += amp * sine(f, n)
        # Мягкий суб-бас на октаву ниже корня.
        part += 0.18 * sine(note(root) / 2.0, n)
        buf[start:start + n] += part * env

    # Арпеджио: восьмые, тихо, с затуханием — «звёзды в сумерках».
    step = 0.5
    melody = ["D5", "F5", "A5", "F5", "C5", "E5", "G5", "E5",
              "D5", "F5", "A5", "C6", "A5", "F5", "E5", "D5"]
    for k, name in enumerate(melody):
        start = int(k * step * RATE)
        n = int(1.2 * RATE)
        if start + n > len(buf):
            n = len(buf) - start
        if n <= 0:
            break
        t = np.arange(n) / RATE
        decay = np.exp(-3.2 * t)
        f = note(name)
        part = (sine(f, n) + 0.35 * sine(2 * f, n) + 0.12 * sine(3 * f, n)) * decay
        buf[start:start + n] += 0.11 * part

    # Кроссфейд: хвост плавно накладывается на начало — стык не слышно.
    head = buf[:extra].copy()
    tail = buf[total:total + extra]
    fade_in = np.linspace(0.0, 1.0, extra)
    buf[:extra] = head * fade_in + tail * (1.0 - fade_in)
    buf = buf[:total]

    peak = np.max(np.abs(buf)) or 1.0
    buf = buf / peak * 0.55
    pcm = (buf * 32767.0).astype("<i2")

    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(pcm.tobytes())
    print(f"записано {path}: {SECONDS:.0f} с, {len(pcm.tobytes())} байт")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "music/theme.wav")
