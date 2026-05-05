---
name: video-transcript-downloader
description: Download videos, audio, subtitles, and clean paragraph-style transcripts from YouTube and any other yt-dlp supported site. Use when asked to “download this video”, “save this clip”, “rip audio”, “get subtitles”, “get transcript”, or to troubleshoot yt-dlp/ffmpeg and formats/playlists.
---

# Video Transcript Downloader

`./scripts/vtd.js` can:
- Print a transcript as a clean paragraph (timestamps optional).
- Download video/audio/subtitles.

Transcript behavior (three-stage fallback):
1. YouTube: fetch via `youtube-transcript-plus` when possible.
2. Otherwise: pull subtitles via `yt-dlp`, then clean into a paragraph.
3. If subtitles are unavailable: download audio and transcribe locally with `mlx_whisper` (Apple Silicon only).

## Setup

```bash
cd ~/workspaces/webdavis/uriel/agents/bob/workspace/skills/video-transcript-downloader && npm ci
```

## Transcript (default: clean paragraph)

```bash
./scripts/vtd.js transcript --url 'https://…'
./scripts/vtd.js transcript --url 'https://…' --lang en
./scripts/vtd.js transcript --url 'https://…' --timestamps
./scripts/vtd.js transcript --url 'https://…' --keep-brackets
```

## Whisper fallback (caption-less videos)

When neither the YouTube direct path nor `yt-dlp` subtitles produce text, vtd downloads the audio and runs `mlx_whisper` locally. Defaults are tuned for journalism-grade accuracy.

- Auto-engages on subtitle failure. No flag needed.
- Requires `mlx_whisper` on PATH (Apple Silicon only). Install: `uv tool install mlx-whisper` or `pip install mlx-whisper`.
- Default model: `mlx-community/whisper-large-v3` (~3 GB, max accuracy). First run downloads the model into `~/.cache/huggingface/hub/`; subsequent runs reuse the cache.
- `--timestamps` invokes mlx_whisper with `--word-timestamps True`, producing word-level cue boundaries — useful for citation work.
- Stderr emits a `transcribing audio with mlx_whisper…` notice before the spawn (fallback can take several minutes on long content).

```bash
# Disable the fallback and let the original subtitle error surface
./scripts/vtd.js transcript --url 'https://…' --no-whisper-fallback

# Override the model (e.g. trade accuracy for ~3x speed)
./scripts/vtd.js transcript --url 'https://…' --whisper-model 'mlx-community/whisper-large-v3-turbo'
```

## Download video / audio / subtitles

```bash
./scripts/vtd.js download --url 'https://…' --output-dir ~/Downloads
./scripts/vtd.js audio --url 'https://…' --output-dir ~/Downloads
./scripts/vtd.js subs --url 'https://…' --output-dir ~/Downloads --lang en
```

## Formats (list + choose)

List available formats (format ids, resolution, container, audio-only, etc):

```bash
./scripts/vtd.js formats --url 'https://…'
```

Download a specific format id (example):

```bash
./scripts/vtd.js download --url 'https://…' --output-dir ~/Downloads -- --format 137+140
```

Prefer MP4 container without re-encoding (remux when possible):

```bash
./scripts/vtd.js download --url 'https://…' --output-dir ~/Downloads -- --remux-video mp4
```

## Notes

- Default transcript output is a single paragraph. Use `--timestamps` only when asked.
- Bracketed cues like `[Music]` are stripped by default; keep them via `--keep-brackets`.
- Pass extra `yt-dlp` args after `--` for `transcript` fallback, `download`, `audio`, `subs`, `formats`.

```bash
./scripts/vtd.js formats --url 'https://…' -- -v
```

## Troubleshooting (only when needed)

- Missing `yt-dlp` / `ffmpeg`:

```bash
brew install yt-dlp ffmpeg
```

- Missing `mlx_whisper` (only relevant when fallback engages):

```bash
uv tool install mlx-whisper      # preferred
pip install mlx-whisper          # alternative
```

- Verify:

```bash
yt-dlp --version
ffmpeg -version | head -n 1
mlx_whisper --help | head -n 1
```
