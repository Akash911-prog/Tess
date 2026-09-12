# Tess

A local, voice-first assistant. You say a wake word, Tess transcribes what you say, figures out what you meant, and dispatches it to a skill that acts on your machine — all on-device.

> **Status: work in progress.** There is no release yet, the API and skill set are small and actively changing, and things will break. This README describes the system as it exists today, not a finished product.

## How it works

Tess is split into two processes that talk to each other over a local pipe:

```
 mic audio
    │
    ▼
┌─────────────────────────┐        JSON lines over a named pipe        ┌──────────────────────────────────────────────┐
│         tess-stt         │ ───────────────────────────────────────▶ │                   tess-core                    │
│  (Python)                │                                           │  (Rust)                                        │
│                          │                                           │                                                 │
│  1. wake word detection  │                                           │  1. semantic parser matches transcript          │
│  2. speech-to-text       │                                           │     text to an intent via embeddings           │
│     (faster-whisper)     │                                           │  2. rule-based extractor pulls out arguments   │
│                          │                                           │  3. matched skill executes the command         │
└─────────────────────────┘                                           └──────────────────────────────────────────────┘
```

1. **`tess-stt`** listens for a wake word, then transcribes the speech that follows and emits a `TranscriptEvent` (as a line of JSON) over a named pipe.
2. **`tess-core`** reads events off that pipe, embeds the transcript and compares it against a catalog of known intents (cosine similarity over sentence embeddings) to decide *what* was asked, pulls out any arguments the intent needs (durations, numbers, on/off toggles, etc.), and dispatches the resulting command to the skill that owns that intent.
3. A **skill** is the thing that actually does something — e.g. pausing media or changing the system volume.

## Components

### `tess-core` (Rust)

The brain of the assistant. Highlights:

- **`registry/`** — the skill/intent system. Skills declare their intents (id, description, typed argument schema, and example phrases) through a small `skill!` macro; the registry indexes them for dispatch and hands the exemplar phrases to the parser. Adding a skill only means writing one file and listing it in `registry/skills/mod.rs`.
- **`parser/`** — turns a raw transcript into one or more `Event`s. The default parser (`SemanticParser`) embeds the transcript with a local model (via [`fastembed`](https://crates.io/crates/fastembed)) and matches it to the closest intent exemplar above a similarity threshold/margin.
- **`extractor/`** — once an intent is chosen, pulls the intent's declared arguments (`Integer`, `Duration`, `Text`, `Enum`) out of the transcript with regex/keyword rules.
- **`ipc.rs`** — a Windows named pipe (`\\.\pipe\tess`) that `tess-stt` writes transcript events to.
- **`event_bus.rs`** — a `tokio::broadcast` channel fanning parsed transcript events out to the pipeline.

Currently ships two skills: **`media`** (play/pause/next/previous/skip/rewind) and **`system`** (volume up/down).

> **Note:** `tess-core`'s IPC currently only targets Windows (`tokio::net::windows::named_pipe`). It hasn't been ported to other platforms yet.

### `tess-stt` (Python)

The ears. Runs a wake-word model ([`livekit-wakeword`](https://pypi.org/project/livekit-wakeword/)) to detect "Tess", then transcribes the following speech with [`faster-whisper`](https://github.com/SYSTRAN/faster-whisper) and writes the transcript to `tess-core` over the named pipe.

## Repo layout

```
tess-core/            Rust core: parsing, arg extraction, skill dispatch
  src/
    registry/         skills, intents, argument schemas
    parser/            embedding-based intent matching
    extractor/          rule-based argument extraction
    ipc.rs, event_bus.rs, events.rs
  examples/            benchmarking / scratch examples
  tests/               integration tests

tess-stt/             Python speech pipeline
  wakeword/            wake word detection
  stt/                 whisper-based transcription
  models/              wake word / STT model artifacts
  benchmarks/

package.json           bun scripts to run both processes together in dev
```

## Running it (development)

You'll need:

- Rust (the core currently requires an edition-2024-capable toolchain — a recent stable, e.g. 1.85+)
- Python 3.12+ with [`uv`](https://docs.astral.sh/uv/)
- [Bun](https://bun.sh/) (for the root dev scripts) — not required if you just run each side manually
- Windows, for the IPC layer to work as-is

From the repo root:

```bash
bun install

# run tess-core and tess-stt together
bun run dev

# or run them separately
bun run dev:core   # cargo run --manifest-path tess-core/Cargo.toml
bun run dev:stt    # cd tess-stt && uv run main.py
```

Rust-only work (e.g. iterating on the registry/parser/extractor) can skip the STT side entirely:

```bash
cd tess-core
cargo test
cargo run
```

## Roadmap / known gaps

This section exists so nobody mistakes the current state for a finished assistant:

- Only two skills exist (`media`, `system`); no persistence, config, or settings UI yet.
- `extractor`'s `Text` argument kind is a stub — it doesn't yet capture free-form text from a transcript.
- IPC is Windows-only.
- No packaging, installer, or release build yet.
- No license has been chosen yet.

Contributions and issues are welcome, but expect the internals to shift while this settles.