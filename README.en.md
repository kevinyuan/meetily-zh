<div align="center">

# Meetily-ZH

**A Chinese-first AI meeting assistant · Runs entirely on your own machine**

**English** · [中文](README.md)

[![Build](https://github.com/kevinyuan/meetily-zh/actions/workflows/build-macos.yml/badge.svg)](https://github.com/kevinyuan/meetily-zh/actions/workflows/build-macos.yml)
[![Release](https://img.shields.io/github/v/release/kevinyuan/meetily-zh?color=brightgreen)](https://github.com/kevinyuan/meetily-zh/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/kevinyuan/meetily-zh/total?color=blue)](https://github.com/kevinyuan/meetily-zh/releases)
[![Stars](https://img.shields.io/github/stars/kevinyuan/meetily-zh?style=flat&color=yellow)](https://github.com/kevinyuan/meetily-zh/stargazers)
[![License](https://img.shields.io/badge/License-MIT-blue)](LICENSE.md)
[![Platform](https://img.shields.io/badge/Platform-macOS_(Apple_Silicon)-lightgrey?logo=apple)](#install--macos)
[![Engine](https://img.shields.io/badge/Engine-SenseVoice-orange)](https://github.com/FunAudioLLM/SenseVoice)
[![Built with](https://img.shields.io/badge/Tauri_+_Rust-24C8B8?logo=tauri&logoColor=white)](https://tauri.app)

**No cloud · No audio leaves the device · No subscription**

</div>

---

A Chinese-first fork of [**Meetily**](https://github.com/Zackriya-Solutions/meeting-minutes) — a privacy-first AI meeting assistant that records, transcribes and summarises meetings **entirely on your own machine**. No cloud, no audio leaving the device.

Everything upstream does, this does too. What it adds is Chinese support that actually works.

---

## Why this fork exists

Upstream Meetily is a good piece of software, but its transcription and summarisation paths quietly assume English. Used on Chinese meetings, three things went wrong — and none of them was a small bug:

1. **Chinese speech produced English transcripts.** The default language preference was `auto-translate`: detect the language, then *translate it into English*.
2. **Chinese meetings produced English summaries.** Summaries were drafted in English by construction. On "auto", a Chinese meeting was summarised and then translated *into* English — the exact opposite of what the setting promised.
3. **The English-only engine was the default.** Onboarding installed Parakeet, which cannot transcribe Chinese at all.

So this fork fixes the defaults, adds a transcription engine built for Chinese, and translates the interface.

---

## What's added

### SenseVoice — a Chinese-first transcription engine

[SenseVoice](https://github.com/FunAudioLLM/SenseVoice) (Alibaba Tongyi / FunAudioLLM) joins Whisper and Parakeet as a third engine, and is the new default. It handles **Chinese, English, Japanese, Korean and Cantonese**, runs roughly **60× realtime on CPU**, and outputs punctuation.

The model is sherpa-onnx's official int8 export (~236 MB), downloaded from inside the app. It runs on the ONNX Runtime the app already ships for Parakeet, so nothing extra is bundled.

### Sentence-level transcripts

A transcript line used to be whatever fell between two silences longer than 400 ms. But in fluent speech the pause *between* sentences is usually shorter than that, so sentences merged — real recordings produced single lines up to **39 seconds** long.

Lines can now be split on **the punctuation the model itself predicts**, with timestamps from its acoustic (CTC) alignment. Both modes are selectable in **Settings → Recording**, and engines that cannot do this fall back to the old behaviour without losing any text.

### Per-sentence language detection

SenseVoice identifies the language of each utterance, so a meeting that switches between Chinese and English is labelled line by line.

Auto-detection is unreliable on short utterances: measured over a real meeting, **54.8%** of segments under half a second were mislabelled — and a misread language does more than mislabel the line, because the model then *decodes* in that language (Chinese 「对啦呢」 came back as Japanese kana, `だらね`). Short utterances are therefore decoded in the language the meeting is actually being held in, voted on by the segments long enough to be trusted.

### Chinese interface

The whole UI is translated (~890 strings). The display language is autodetected from your OS locale and can be changed in **Settings → General**. Transcription language, summary language and the model filter all follow it by default.

### Chinese-first defaults

Transcription no longer translates to English unless you ask it to. Summaries are written in the display language. The model list filters down to engines that can actually handle the language you chose — Parakeet is hidden under Chinese, because it genuinely cannot do it.

### Other fixes

- **Recovered dropped speech.** Utterances shorter than 250 ms were discarded, silently eating 「嗯」「好」「对」/ "yes" / "OK". The floor is now 120 ms.
- **Fixed the audio mixer.** System audio was mixed at full level despite a comment claiming it was attenuated, and the clipping guard then dragged the microphone down with it — quiet speech went undetected while a meeting was playing.
- **VAD crash mid-recording.** Silero *panics* rather than erroring when its padding reaches outside the buffer, which killed the audio pipeline outright: transcription stopped a few seconds in while the UI carried on as if it were still recording.
- **Transcript timestamps.** Segments force-cut out of long unbroken speech drifted to roughly twice their true offset.
- **Indexed the transcript lookup.** Opening any meeting scanned the entire transcripts table.
- **Markdown export.** Meetings live in SQLite; export gives you plain-text copies for a folder, Git or iCloud.
- **The macOS menu-bar icon** is a proper monochrome template icon instead of the full-colour app icon.

---

## Install — macOS

**Apple Silicon (arm64) only.**

### Option 1 — Download the DMG (recommended)

Grab `meetily_*_aarch64.dmg` from [Releases](https://github.com/kevinyuan/meetily-zh/releases/latest) and drag it into Applications.

> **Right-click → Open on first launch.**
> The build is not signed or notarised with an Apple Developer certificate, so Gatekeeper will block it once. After that it opens normally.

### Option 2 — Build from source

```bash
git clone https://github.com/kevinyuan/meetily-zh.git
cd meetily-zh/frontend
pnpm install
./dev.sh                  # development
pnpm run tauri:build      # release build
```

**Prerequisites**

- **Xcode** — the full app, not just the Command Line Tools. The `cidre` dependency (macOS system-audio capture) needs `xcodebuild`.
- **cmake** — `brew install cmake`, used to build whisper.cpp.

> Use `./dev.sh` for development, not `pnpm run tauri:dev`.
> The latter opens the window before Next.js has finished compiling, so the webview reads a truncated JS bundle and React never hydrates — the UI is visible but **completely unclickable**. `dev.sh` proves every chunk is complete and parseable before the window opens.

### First launch

Downloads the SenseVoice model (~236 MB) and a summarisation model into:

```
~/Library/Application Support/com.meetily-zh.ai/models/
```

They are reused across builds and releases, so you never download them twice.

macOS will ask for **microphone** and **screen recording** permission. Screen recording is what captures the *other* side of a call — without it you only record yourself.

> **Migrating from upstream Meetily:** this fork uses its own app data directory (`com.meetily-zh.ai`) and will not read meetings or models from upstream's `com.meetily.ai`. The two can be installed side by side.

---

## Windows

**The code supports Windows. This fork has not been tested on it.**

Upstream builds and ships Windows installers, and nothing added here is macOS-specific: SenseVoice, the ONNX runtime, the segmentation logic and the interface are all cross-platform. The repository's own workflow (`.github/workflows/build-windows.yml`) builds MSI and NSIS installers on a GitHub-hosted Windows runner, and it succeeds without a code-signing certificate (it skips signing rather than failing).

I don't have a Windows machine and don't have time to test there, so I'm making no claims about it. Two areas would need real verification first:

- **Audio capture**, which uses WASAPI rather than CoreAudio. The mixing and voice-detection changes here were only validated on macOS, and buffer behaviour differs between the two.
- **System-audio capture**, which is a separate implementation entirely.

If you build and test it on Windows, please open an issue with what you find. Contributions welcome.

**Linux** compiles but is not released upstream either — you would be building from source.

---

## Credits

- [**Meetily**](https://github.com/Zackriya-Solutions/meeting-minutes) by Zackriya Solutions — everything this is built on.
- [**SenseVoice**](https://github.com/FunAudioLLM/SenseVoice) by Alibaba's Tongyi Lab, exported to ONNX by [**sherpa-onnx**](https://github.com/k2-fsa/sherpa-onnx) (Next-gen Kaldi).
- The SenseVoice inference code is a vendored subset of [**transcribe-rs**](https://crates.io/crates/transcribe-rs) (MIT), ported back to an older ONNX Runtime — see `frontend/src-tauri/src/sensevoice_engine/vendor/`.

MIT, like upstream.
