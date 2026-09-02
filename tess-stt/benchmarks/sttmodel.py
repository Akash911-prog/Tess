"""
Benchmarks faster-whisper model sizes for the lazy-load-on-wake-word design:
load model -> transcribe -> unload, per test clip, per model size.

Measures:
  - RAM delta caused by loading the model (the number that actually matters
    for your "only when triggered" design — not steady-state RAM)
  - Peak RAM during transcription itself
  - Load latency (cold, from disk/cache)
  - Transcribe latency (per clip)
  - Accuracy via Word Error Rate, if you provide ground-truth transcripts

Setup:
    uv add faster-whisper psutil jiwer

Usage:
    Put test clips in ./clips/ as .wav (16kHz mono recommended, but
    faster-whisper will resample for you if not).
    Optionally put matching ground-truth text in ./clips/<name>.txt
    (same basename as the .wav) for WER scoring.

    python benchmark_stt.py

Notes:
    - Run this on the SAME machine tess-stt will actually run on. RAM/CPU
      figures from a different machine (esp. a cloud sandbox) won't transfer.
    - First run per model size will be slower — CTranslate2/faster-whisper
      downloads the model to a local cache on first use. Run once to warm
      the cache, then run again for representative "already downloaded"
      numbers, since that's the steady-state you'll actually see in
      production after the first launch ever.
"""

import gc
import time
import statistics
from dataclasses import dataclass, field
from pathlib import Path

import psutil
from faster_whisper import WhisperModel

try:
    from jiwer import wer

    HAS_JIWER = True
except ImportError:
    HAS_JIWER = False

MODEL_SIZES = ["tiny.en", "base.en"]
COMPUTE_TYPE = "int8"  # matches the quantization you'd actually deploy with
CLIPS_DIR = Path("./clips")

PROCESS = psutil.Process()


def rss_mb() -> float:
    return PROCESS.memory_info().rss / (1024 * 1024)


@dataclass
class ClipResult:
    clip: str
    load_s: float
    transcribe_s: float
    ram_after_load_mb: float
    ram_peak_mb: float
    text: str
    wer_score: float | None = None


@dataclass
class ModelResult:
    model_size: str
    clips: list[ClipResult] = field(default_factory=list)

    def summary(self) -> str:
        loads = [c.load_s for c in self.clips]
        transcribes = [c.transcribe_s for c in self.clips]
        ram_loads = [c.ram_after_load_mb for c in self.clips]
        ram_peaks = [c.ram_peak_mb for c in self.clips]
        wers = [c.wer_score for c in self.clips if c.wer_score is not None]

        lines = [
            f"\n=== {self.model_size} ===",
            f"  load latency:       avg={statistics.mean(loads):.2f}s  max={max(loads):.2f}s",
            f"  transcribe latency: avg={statistics.mean(transcribes):.2f}s  max={max(transcribes):.2f}s",
            f"  RAM delta on load:  avg={statistics.mean(ram_loads):.0f}MB  max={max(ram_loads):.0f}MB",
            f"  RAM peak (loaded+transcribing): avg={statistics.mean(ram_peaks):.0f}MB  max={max(ram_peaks):.0f}MB",
        ]
        if wers:
            lines.append(
                f"  WER: avg={statistics.mean(wers):.1%}  (n={len(wers)} clips with ground truth)"
            )
        elif HAS_JIWER:
            lines.append("  WER: no ground-truth .txt files found next to clips")
        else:
            lines.append("  WER: jiwer not installed, skipping accuracy scoring")
        return "\n".join(lines)


def benchmark_model(model_size: str, clip_paths: list[Path]) -> ModelResult:
    result = ModelResult(model_size=model_size)

    for clip_path in clip_paths:
        gc.collect()
        baseline_rss = rss_mb()

        # --- Load (simulates: wake word just fired) ---
        t0 = time.perf_counter()
        model = WhisperModel(model_size, device="cpu", compute_type=COMPUTE_TYPE)
        load_s = time.perf_counter() - t0
        ram_after_load = rss_mb() - baseline_rss

        # --- Transcribe ---
        t0 = time.perf_counter()
        segments, _info = model.transcribe(str(clip_path), beam_size=5)
        text = " ".join(seg.text.strip() for seg in segments)
        transcribe_s = time.perf_counter() - t0
        ram_peak = rss_mb() - baseline_rss

        # --- Unload (simulates: going idle again until next wake word) ---
        del model
        gc.collect()

        wer_score = None
        gt_path = clip_path.with_suffix(".txt")
        if HAS_JIWER and gt_path.exists():
            ground_truth = gt_path.read_text().strip()
            wer_score = wer(ground_truth.lower(), text.lower())  # type: ignore

        result.clips.append(
            ClipResult(
                clip=clip_path.name,
                load_s=load_s,
                transcribe_s=transcribe_s,
                ram_after_load_mb=ram_after_load,
                ram_peak_mb=ram_peak,
                text=text,
                wer_score=wer_score,
            )
        )

        print(
            f"  [{model_size}] {clip_path.name}: "
            f"load={load_s:.2f}s transcribe={transcribe_s:.2f}s "
            f"ram_load={ram_after_load:.0f}MB ram_peak={ram_peak:.0f}MB "
            f'-> "{text}"'
        )

    return result


def main() -> None:
    if not CLIPS_DIR.exists():
        print(
            f"Create {CLIPS_DIR}/ and put some .wav test clips in it first "
            f"(short media-control commands, same conditions you'll actually use)."
        )
        return

    clip_paths = sorted(CLIPS_DIR.glob("*.wav"))
    if not clip_paths:
        print(f"No .wav files found in {CLIPS_DIR}/")
        return

    print(f"Found {len(clip_paths)} clip(s): {[p.name for p in clip_paths]}")
    if not HAS_JIWER:
        print(
            "(jiwer not installed -> WER scoring will be skipped. `uv add jiwer` to enable it.)"
        )

    results = []
    for model_size in MODEL_SIZES:
        print(f"\nBenchmarking {model_size}...")
        results.append(benchmark_model(model_size, clip_paths))

    print("\n" + "=" * 50)
    print("SUMMARY")
    print("=" * 50)
    for r in results:
        print(r.summary())


if __name__ == "__main__":
    main()
