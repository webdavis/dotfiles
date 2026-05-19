---
name: whisply
version: 1.0.0
description: Local speech-to-text with whisply (Apple MLX, NVIDIA GPU, or CPU; no API key).
homepage: https://github.com/tsmdt/whisply
metadata: {"clawdbot":{"emoji":"🍎","requires":{"bins":["whisply"]},"install":[{"id":"uv","kind":"uv-tool","package":"whisply[mlx,app]","bins":["whisply"],"label":"Install whisply with MLX + app extras (uv tool)"}]}}
---

# whisply

Local speech-to-text via `whisply`, a wrapper around faster-whisper / MLX-Whisper / whisperX that picks a backend per `-d`.

## Quick Start

```bash
whisply run -f /path/to/audio.mp3 -m large-v3-turbo -d mlx
```

## Common Usage

```bash
# Transcribe to a text file
whisply run -f audio.m4a -e txt -o ./output -d mlx

# Transcribe with language hint
whisply run -f audio.mp3 -l en -m large-v3-turbo -d mlx

# Generate subtitles (SRT)  — note: `-s` is required for any subtitle export
whisply run -f video.mp4 -e srt -s -o ./subs -d mlx

# Translate to English
whisply run -f foreign.mp3 -t -d mlx

# Speaker annotation (requires HuggingFace token)
whisply run -f meeting.mp3 -a -hf $HF_TOKEN -d mlx
```

## Flags

| Flag | Long | Meaning |
|---|---|---|
| `-f` | `--files` | Path, folder, URL, or `.list` to process |
| `-o` | `--output_dir` | Output folder (default `transcriptions`) |
| `-d` | `--device` | `auto` (default), `cpu`, `gpu` (NVIDIA), or `mlx` (Apple) |
| `-m` | `--model` | Model name — see `whisply list` (default `large-v3-turbo`) |
| `-l` | `--language` | Language hint like `en`/`de` (default auto-detect) |
| `-e` | `--export` | `all`, `txt`, `srt`, `vtt`, `webvtt`, `json`, `rttm`, `html` |
| `-t` | `--translate` | Translate transcription to English |
| `-s` | `--subtitle` | Create subtitles |
| `-a` | `--annotate` | Enable speaker annotation (needs `-hf`) |
| `-v` | `--verbose` | Print text chunks during transcription |

## Models

Models are unprefixed in whisply (no `mlx-community/` needed). Run `whisply list` to see what's available.

| Model | Size | Speed | Quality |
|---|---|---|---|
| `tiny` | ~75 MB | Fastest | Basic |
| `base` | ~140 MB | Fast | Good |
| `small` | ~470 MB | Medium | Better |
| `medium` | ~1.5 GB | Slower | Great |
| `large-v3` | ~3 GB | Slowest | Best |
| `large-v3-turbo` | ~1.6 GB | Fast | Excellent (default) |

## Notes

- `-d mlx` requires Apple Silicon and the `[mlx]` extra (install with `uv tool install 'whisply[mlx,app]'`).
- `-s` is required for any subtitle export (`srt`, `vtt`, `webvtt`, `rttm`). Without it whisply errors with "X export format requires subtitle option to be True". Default `-e all` without `-s` only emits `txt` + `json`.
- Whisply writes outputs to `<output_dir>/<basename>/<basename>_<lang>.<ext>` (a subdir per input) and also creates a `logs/` directory next to it.
- Whisply consumes (deletes) the input file during conversion. Copy first if you need to keep it.
- Models cache to `~/.cache/huggingface/`.
- For a GUI: `whisply app`.
