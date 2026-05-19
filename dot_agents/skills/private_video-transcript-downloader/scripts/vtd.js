#!/usr/bin/env node
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { YoutubeTranscript } from "youtube-transcript-plus";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

function die(message, code = 1) {
  process.stderr.write(String(message).trimEnd() + "\n");
  process.exit(code);
}

function parseArgs(argv) {
  // Tiny no-deps parser.
  // - `--flag` => boolean
  // - `--key value`
  // - `--` => forward remaining args to yt-dlp
  const positional = [];
  const opts = {};
  let i = 0;
  while (i < argv.length) {
    const a = argv[i];
    if (a === "--") {
      opts.extra = argv.slice(i + 1);
      break;
    }
    if (!a.startsWith("--")) {
      positional.push(a);
      i += 1;
      continue;
    }
    const key = a.slice(2);
    const next = argv[i + 1];
    const isValue = next !== undefined && !next.startsWith("--");
    if (!isValue) {
      opts[key] = true;
      i += 1;
      continue;
    }
    if (opts[key] === undefined) opts[key] = next;
    else if (Array.isArray(opts[key])) opts[key].push(next);
    else opts[key] = [opts[key], next];
    i += 2;
  }
  return { positional, opts };
}

function toArray(v) {
  if (v === undefined) return [];
  if (Array.isArray(v)) return v;
  return [v];
}

function which(cmd) {
  // Avoid shelling out to `which`; keep it portable + fast.
  const envPath = process.env.PATH || "";
  const parts = envPath.split(path.delimiter);
  for (const p of parts) {
    const full = path.join(p, cmd);
    if (fs.existsSync(full)) return full;
  }
  return null;
}

function resolveBin(name, fallback) {
  return which(name) || (fallback && fs.existsSync(fallback) ? fallback : null);
}

function run(cmd, args, { cwd } = {}) {
  return new Promise((resolve) => {
    // Capture stdout + stderr to keep yt-dlp’s error context intact.
    const child = spawn(cmd, args, { cwd, stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    child.stdout.on("data", (d) => (out += d.toString()));
    child.stderr.on("data", (d) => (out += d.toString()));
    child.on("close", (code) => resolve({ code, out }));
  });
}

function isYouTubeUrl(url) {
  return /(^https?:\/\/)?(www\.)?(youtube\.com|youtu\.be)\//i.test(url);
}

function extractYouTubeId(input) {
  if (!input) return null;
  const raw = String(input).trim();
  if (/^[a-zA-Z0-9_-]{11}$/.test(raw)) return raw;
  const m = raw.match(/(?:v=|youtu\.be\/)([a-zA-Z0-9_-]{11})/);
  return m ? m[1] : null;
}

function decodeHtmlEntities(input) {
  if (!input) return input;
  // Some transcripts come back double-encoded (e.g. "&amp;#39;").
  // Decode up to 2 passes; stop once stable.
  let text = input;
  for (let i = 0; i < 2; i++) {
    const decoded = text
      .replace(/&amp;/g, "&")
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .replace(/&quot;/g, '"')
      .replace(/&apos;/g, "'")
      .replace(/&#(\d+);/g, (_, dec) => String.fromCodePoint(Number(dec)))
      .replace(/&#x([0-9a-fA-F]+);/g, (_, hex) => String.fromCodePoint(parseInt(hex, 16)));
    if (decoded === text) break;
    text = decoded;
  }
  return text;
}

function formatTimestamp(seconds) {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = Math.floor(s % 60);
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
  return `${m}:${String(sec).padStart(2, "0")}`;
}

function cleanSegments(segments, { keepBrackets } = {}) {
  const cleaned = [];
  let prev = "";

  for (const seg of segments) {
    const s = String(seg || "")
      .replace(/\s+/g, " ")
      .trim();
    if (!s) continue;

    // Subtitles often contain HTML-ish tags; strip them.
    const withoutTags = s.replace(/<[^>]+>/g, "").trim();
    const withoutBrackets = keepBrackets ? withoutTags : withoutTags.replace(/\[[^\]]*\]/g, "").trim();
    const withoutCurlies = withoutBrackets.replace(/\{[^}]+\}/g, "").replace(/♪/g, "").trim();
    const t = withoutCurlies.replace(/\s+/g, " ").trim();
    if (!t) continue;
    if (t === prev) continue;
    // Dedup heuristic: captions often repeat previous line with a longer suffix.
    if (prev && t.startsWith(prev)) {
      const newPart = t.slice(prev.length).trim();
      if (newPart) cleaned.push(newPart);
    } else if (prev && t.includes(prev)) {
      // Another common pattern: current line contains previous line in the middle.
      const idx = t.indexOf(prev);
      const newPart = (t.slice(0, idx) + t.slice(idx + prev.length)).trim();
      if (newPart) cleaned.push(newPart);
    } else {
      cleaned.push(t);
    }
    prev = t;
  }

  return cleaned;
}

function toParagraph(segments, { keepBrackets } = {}) {
  const cleaned = cleanSegments(segments, { keepBrackets });
  return cleaned.join(" ").replace(/\s+/g, " ").trim();
}

function parseSrt(text) {
  const lines = String(text).split(/\r?\n/);
  const segments = [];
  for (const line of lines) {
    const l = line.trim();
    if (!l) continue;
    if (/^\d+$/.test(l)) continue;
    if (l.includes("-->")) continue;
    segments.push(l);
  }
  return segments;
}

function parseVtt(text) {
  const lines = String(text).split(/\r?\n/);
  const segments = [];
  let inNote = false;
  for (const line of lines) {
    const l = line.trim();
    if (!l) { inNote = false; continue; }
    if (inNote) continue;
    if (l.startsWith("WEBVTT")) continue; // header may carry a suffix (e.g. "WEBVTT whisply_test")
    if (l === "NOTE" || l.startsWith("NOTE ")) { inNote = true; continue; } // NOTE block runs until blank line
    if (l.startsWith("Kind:") || l.startsWith("Language:")) continue;
    // SRT-style cue indices (Vimeo VTT and others include them even though VTT spec doesn't require them)
    if (/^\d+$/.test(l)) continue;
    if (l.includes("-->")) continue;
    // cue settings like "align:start position:0%"
    if (/^(align|position|size|line):/i.test(l)) continue;
    // Remove inline timestamps like "<00:00:00.000>" (common in YouTube VTT).
    const cleaned = l.replace(/<\d{2}:\d{2}:\d{2}\.\d{3}>/g, "").trim();
    if (cleaned) segments.push(cleaned);
  }
  return segments;
}

function parseVttWithCues(text) {
  // Like parseVtt but preserves cue start times. Returns [{ start, text }].
  // Used by the Whisper fallback's --timestamps path so each cue emits its own [mm:ss] line.
  const lines = String(text).split(/\r?\n/);
  const cues = [];
  let currentStart = null;
  let currentText = [];
  let inNote = false;

  const flush = () => {
    if (currentStart !== null) {
      const joined = currentText.join(" ").replace(/<\d{2}:\d{2}:\d{2}\.\d{3}>/g, "").replace(/\s+/g, " ").trim();
      if (joined) cues.push({ start: currentStart, text: joined });
    }
    currentStart = null;
    currentText = [];
  };

  for (const line of lines) {
    const l = line.trim();
    if (!l) {
      flush();
      inNote = false;
      continue;
    }
    if (inNote) continue;
    if (l.startsWith("WEBVTT")) continue; // header may carry a suffix (e.g. "WEBVTT whisply_test")
    if (l === "NOTE" || l.startsWith("NOTE ")) { inNote = true; continue; }
    if (l.startsWith("Kind:") || l.startsWith("Language:")) continue;
    // SRT-style cue indices (Vimeo VTT and others include them even though VTT spec doesn't require them)
    if (/^\d+$/.test(l)) continue;
    if (l.includes("-->")) {
      // VTT cue lines use HH:MM:SS.mmm, but some tools emit MM:SS.mmm for short clips.
      const mHHMMSS = l.match(/^(\d{2}):(\d{2}):(\d{2})\.(\d{3})/);
      const mMMSS = l.match(/^(\d{2}):(\d{2})\.(\d{3})/);
      if (mHHMMSS) {
        currentStart = Number(mHHMMSS[1]) * 3600 + Number(mHHMMSS[2]) * 60 + Number(mHHMMSS[3]) + Number(mHHMMSS[4]) / 1000;
      } else if (mMMSS) {
        currentStart = Number(mMMSS[1]) * 60 + Number(mMMSS[2]) + Number(mMMSS[3]) / 1000;
      }
      continue;
    }
    if (/^(align|position|size|line):/i.test(l)) continue;
    currentText.push(l);
  }
  flush();
  return cues;
}

async function ytDlpSubtitlesToTemp({ url, lang, ytdlpPath, extra }) {
  const ytdlp = ytdlpPath || resolveBin("yt-dlp", "/opt/homebrew/bin/yt-dlp");
  if (!ytdlp) die("missing yt-dlp; install `yt-dlp` and ensure it is on PATH");

  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "vtd-subs-"));
  const outTemplate = path.join(tmpDir, "%(id)s.%(ext)s");

  const args = [];
  args.push(
    "--write-sub",
    "--write-auto-sub",
    "--skip-download",
    "--sub-lang",
    lang,
    "-o",
    outTemplate,
  );
  if (extra?.length) args.push(...extra);
  args.push(url);

  const r = await run(ytdlp, args);
  if (r.code !== 0) {
    fs.rmSync(tmpDir, { recursive: true, force: true });
    throw new Error(r.out.trim() || "yt-dlp subtitle download failed");
  }

  const files = fs
    .readdirSync(tmpDir)
    .map((f) => path.join(tmpDir, f))
    .filter((f) => /\.(vtt|srt|ass|ttml)$/i.test(f))
    .sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs);

  if (files.length === 0) {
    fs.rmSync(tmpDir, { recursive: true, force: true });
    throw new Error(`no subtitles found (lang=${lang})`);
  }

  return { tmpDir, subtitlePath: files[0] };
}

async function cmdTranscript({ url, lang, timestamps, keepBrackets, extra, whisperFallback, whisperModel }) {
  if (!url) die("missing --url");

  let subtitleErr = null;

  // Path 1: YouTube direct transcript (fastest; no yt-dlp / no files).
  if (isYouTubeUrl(url)) {
    const id = extractYouTubeId(url);
    if (id) {
      try {
        const transcript = await YoutubeTranscript.fetchTranscript(id);
        if (timestamps) {
          const out = transcript
            .map((entry) => {
              const ts = formatTimestamp(entry.offset / 1000);
              return `[${ts}] ${decodeHtmlEntities(entry.text).replace(/\s+/g, " ").trim()}`;
            })
            .join("\n");
          if (!out) throw new Error("empty transcript");
          process.stdout.write(out + "\n");
          return;
        }
        const paragraph = toParagraph(transcript.map((e) => decodeHtmlEntities(e.text)), { keepBrackets });
        if (!paragraph) throw new Error("empty transcript");
        process.stdout.write(paragraph + "\n");
        return;
      } catch (err) {
        // Fall through to yt-dlp subs path. Save the error in case it's the only one we get.
        subtitleErr = err;
      }
    }
  }

  // Path 2: yt-dlp subtitles (works on any yt-dlp-supported site).
  let subsTmpDir = null;
  try {
    const { tmpDir, subtitlePath } = await ytDlpSubtitlesToTemp({ url, lang, extra });
    subsTmpDir = tmpDir;
    const raw = fs.readFileSync(subtitlePath, "utf8");
    const segments = subtitlePath.endsWith(".srt") ? parseSrt(raw) : parseVtt(raw);
    const paragraph = toParagraph(segments, { keepBrackets });
    if (!paragraph) throw new Error("empty transcript from subtitles");
    // NOTE: yt-dlp subs path emits paragraph regardless of --timestamps. Existing quirk; out of scope here.
    process.stdout.write(paragraph + "\n");
    return;
  } catch (err) {
    subtitleErr = err;
  } finally {
    if (subsTmpDir) fs.rmSync(subsTmpDir, { recursive: true, force: true });
  }

  // Path 3: Whisper fallback (local, whisply wrapper).
  if (whisperFallback === false) die(subtitleErr.message);

  let para;
  try {
    para = await whisperFallbackTranscript({
      url,
      lang,
      extra,
      model: whisperModel,
      keepBrackets,
      timestamps,
    });
  } catch (err) {
    die(`${subtitleErr.message}; whisper fallback failed: ${err.message}`);
  }

  if (para === null) {
    die(
      `${subtitleErr.message}; whisply not on PATH — install with 'uv tool install whisply[mlx,app]', or pass --no-whisper-fallback`,
    );
  }
  if (!para) die("empty transcript from whisper");
  process.stdout.write(para + "\n");
}

async function cmdSubs({ url, lang, outputDir, extra }) {
  if (!url) die("missing --url");

  const { tmpDir, subtitlePath } = await ytDlpSubtitlesToTemp({
    url,
    lang,
    extra,
  });

  try {
    const out = path.resolve(outputDir);
    fs.mkdirSync(out, { recursive: true });
    const dest = path.join(out, path.basename(subtitlePath));
    fs.copyFileSync(subtitlePath, dest);
    process.stdout.write(dest + "\n");
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

async function cmdDownload({ url, outputDir, extra }) {
  if (!url) die("missing --url");
  const ytdlp = resolveBin("yt-dlp", "/opt/homebrew/bin/yt-dlp");
  if (!ytdlp) die("missing yt-dlp; install `yt-dlp` and ensure it is on PATH");

  const out = path.resolve(outputDir);
  fs.mkdirSync(out, { recursive: true });

  const args = [];

  // `--print after_move:filepath` gives the final path after merges/remux.
  args.push("-P", out, "-o", "%(title).200B (%(id)s).%(ext)s", "-S", "res,ext:mp4:m4a,tbr", "--print", "after_move:filepath");
  if (extra?.length) args.push(...extra);
  args.push(url);

  const r = await run(ytdlp, args);
  if (r.code !== 0) die(r.out.trim() || "yt-dlp download failed");

  const lines = r.out.split("\n").map((l) => l.trim());
  const filePath = lines.find((l) => l.startsWith("/") && fs.existsSync(l));
  if (!filePath) die(r.out.trim() || "could not determine downloaded file path");
  process.stdout.write(path.resolve(filePath) + "\n");
}

async function downloadAudioToTemp({ url, extra }) {
  // Shared by cmdAudio and the Whisper fallback path.
  // Caller is responsible for rmSync(tmpDir) when done.
  const ytdlp = resolveBin("yt-dlp", "/opt/homebrew/bin/yt-dlp");
  if (!ytdlp) throw new Error("missing yt-dlp; install `yt-dlp` and ensure it is on PATH");
  const ffmpeg = resolveBin("ffmpeg", "/opt/homebrew/bin/ffmpeg");
  if (!ffmpeg) throw new Error("missing ffmpeg; install `ffmpeg` (needed for audio extraction)");

  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "vtd-audio-"));

  const args = [
    "--ffmpeg-location",
    ffmpeg,
    "-P",
    tmpDir,
    "-o",
    "%(title).200B (%(id)s).%(ext)s",
    "-x",
    "--audio-format",
    "mp3",
    "--print",
    "after_move:filepath",
  ];
  if (extra?.length) args.push(...extra);
  args.push(url);

  const r = await run(ytdlp, args);
  if (r.code !== 0) {
    fs.rmSync(tmpDir, { recursive: true, force: true });
    throw new Error(r.out.trim() || "yt-dlp audio failed");
  }

  const lines = r.out.split("\n").map((l) => l.trim());
  const filePath = lines.find((l) => l.startsWith("/") && fs.existsSync(l));
  if (!filePath) {
    fs.rmSync(tmpDir, { recursive: true, force: true });
    throw new Error(r.out.trim() || "could not determine downloaded audio file path");
  }
  return { tmpDir, audioPath: path.resolve(filePath) };
}

async function cmdAudio({ url, outputDir, extra }) {
  if (!url) die("missing --url");
  const out = path.resolve(outputDir);
  fs.mkdirSync(out, { recursive: true });

  const { tmpDir, audioPath } = await downloadAudioToTemp({ url, extra });
  try {
    const dest = path.join(out, path.basename(audioPath));
    fs.copyFileSync(audioPath, dest);
    process.stdout.write(dest + "\n");
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

async function whisperFallbackTranscript({ url, lang, extra, model, keepBrackets, timestamps }) {
  // Returns the transcript paragraph (or timestamped string) on success.
  // Returns null when whisply is not available, so the caller can chain a clear error.
  // Throws when whisply itself fails (the caller surfaces that as a die()).
  // Note: --timestamps emits cue-level timestamps (typically per sentence/segment),
  // not the per-word boundaries the previous mlx_whisper path produced.
  const whisply = resolveBin("whisply", path.join(os.homedir(), ".local/bin/whisply"));
  if (!whisply) return null;

  const { tmpDir: audioTmp, audioPath } = await downloadAudioToTemp({ url, extra });
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), "vtd-whisper-"));

  process.stderr.write("transcribing audio with whisply; this may take several minutes…\n");

  try {
    // whisply picks the fastest backend automatically (mlx on Apple Silicon, gpu on NVIDIA, else cpu).
    // `-s` is required to actually emit subtitle exports (otherwise whisply refuses with "VTT export format requires subtitle option to be True").
    // cwd: outDir avoids a whisply 0.14 bug where it crashes computing a relative path if outDir is not under cwd.
    const args = ["run", "-f", audioPath, "-m", model, "-l", lang, "-e", "vtt", "-s", "-o", outDir];
    const r = await run(whisply, args, { cwd: outDir });
    if (r.code !== 0) throw new Error(r.out.trim() || "whisply failed");

    // whisply writes <outDir>/<basename>/<basename>_<lang>.vtt (basename derives from the input file).
    const vttPath = (function findVtt(dir) {
      for (const name of fs.readdirSync(dir)) {
        const full = path.join(dir, name);
        const stat = fs.statSync(full);
        if (stat.isDirectory()) {
          const hit = findVtt(full);
          if (hit) return hit;
        } else if (name.endsWith(".vtt")) {
          return full;
        }
      }
      return null;
    })(outDir);
    if (!vttPath) throw new Error(`whisply produced no VTT under ${outDir}`);
    const raw = fs.readFileSync(vttPath, "utf8");

    if (timestamps) {
      const cues = parseVttWithCues(raw);
      if (cues.length === 0) return "";
      return cues.map((c) => `[${formatTimestamp(c.start)}] ${c.text}`).join("\n");
    }

    const segments = parseVtt(raw);
    return toParagraph(segments, { keepBrackets });
  } finally {
    fs.rmSync(audioTmp, { recursive: true, force: true });
    fs.rmSync(outDir, { recursive: true, force: true });
  }
}

async function cmdFormats({ url, extra }) {
  if (!url) die("missing --url");
  const ytdlp = resolveBin("yt-dlp", "/opt/homebrew/bin/yt-dlp");
  if (!ytdlp) die("missing yt-dlp; install `yt-dlp` and ensure it is on PATH");

  // Print raw yt-dlp format table; user picks `--format <id>` for downloads.
  const args = ["-F"];
  if (extra?.length) args.push(...extra);
  args.push(url);

  const r = await run(ytdlp, args);
  if (r.code !== 0) die(r.out.trim() || "yt-dlp formats failed");
  process.stdout.write(r.out);
}

function usage() {
  const rel = path.relative(process.cwd(), path.join(__dirname, "vtd.js"));
  return [
    "usage:",
    `  ${rel} transcript --url 'https://…' [--lang en] [--timestamps] [--keep-brackets]`,
    `                            [--no-whisper-fallback] [--whisper-model <id>] [-- <yt-dlp extra…>]`,
    `  ${rel} download   --url 'https://…' [--output-dir ~/Downloads] [-- <yt-dlp extra…>]`,
    `  ${rel} audio      --url 'https://…' [--output-dir ~/Downloads] [-- <yt-dlp extra…>]`,
    `  ${rel} subs       --url 'https://…' [--output-dir ~/Downloads] [--lang en] [-- <yt-dlp extra…>]`,
    `  ${rel} formats    --url 'https://…' [-- <yt-dlp extra…>]`,
  ].join("\n");
}

async function main() {
  const { positional, opts } = parseArgs(process.argv.slice(2));
  const cmd = positional[0];

  if (!cmd || cmd === "help" || cmd === "-h" || cmd === "--help") {
    process.stdout.write(usage() + "\n");
    return;
  }

  const url = opts.url;
  const lang = opts.lang || "en";
  const outputDir = opts["output-dir"] || path.join(os.homedir(), "Downloads");

  const timestamps = Boolean(opts.timestamps);
  const keepBrackets = Boolean(opts["keep-brackets"]);
  const extra = opts.extra || [];

  // Whisper fallback config (max-accuracy default; CC requires this)
  const whisperFallback = opts["no-whisper-fallback"] !== true;
  const whisperModel = opts["whisper-model"] || "large-v3";

  if (cmd === "transcript") {
    await cmdTranscript({ url, lang, timestamps, keepBrackets, extra, whisperFallback, whisperModel });
    return;
  }
  if (cmd === "download") {
    await cmdDownload({ url, outputDir, extra });
    return;
  }
  if (cmd === "audio") {
    await cmdAudio({ url, outputDir, extra });
    return;
  }
  if (cmd === "subs") {
    await cmdSubs({ url, lang, outputDir, extra });
    return;
  }
  if (cmd === "formats") {
    await cmdFormats({ url, extra });
    return;
  }

  die(`unknown command: ${cmd}\n\n${usage()}`);
}

main().catch((e) => die(e?.stack || e?.message || String(e)));
