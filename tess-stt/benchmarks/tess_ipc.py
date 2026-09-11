"""
TESS IPC benchmark harness.

Measures:
    Python send -> Rust received -> parsed -> dispatch started

The current tess-core does not emit a success log for silent ExecutionResult::success(),
so "dispatch started" is the most reliable completion point available without changing
tess-core.

Requirements:
    Python 3.12+
    pywin32 (already a tess-stt dependency)

Run with tess-core already running:
    python benchmarks/ipc.py

Examples:
    python benchmarks/ipc.py --text "pause music" --iterations 50 --warmup 5
    python benchmarks/ipc.py --text "next song" --iterations 100
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
import time
import uuid
from collections import deque
from datetime import datetime, timedelta
from pathlib import Path
from threading import Lock, Thread
from typing import Any

import win32file
import pywintypes

PIPE_NAME = r"\\.\pipe\tess"
LOG_DIR = Path.home() / "AppData" / "Roaming" / "Tess" / "logs"

# The tracing JSON logger writes fields like:
# {
#   "timestamp": "...",
#   "level": "INFO",
#   "fields": {
#       "message": "dispatching command to skill",
#       "trace_id": "..."
#   }
# }
DISPATCH_MESSAGE = "dispatching command to skill"
PARSED_MESSAGE = "parsed command"
RECEIVED_MESSAGE = "received transcript event"


def current_log_path() -> Path:
    """Return the rolling tess log, shifted back by 6 hours."""
    log_time = datetime.now() - timedelta(hours=6)
    return LOG_DIR / f"tess.log.{log_time.strftime('%Y-%m-%d')}"


def wait_for_log(timeout: float) -> Path:
    deadline = time.perf_counter() + timeout

    while time.perf_counter() < deadline:
        path = current_log_path()
        if path.exists():
            return path
        time.sleep(0.05)

    raise TimeoutError(
        f"Could not find today's TESS log:\n  {current_log_path()}\n"
        "Make sure tess-core is running and has emitted at least one log."
    )


class LogTail:
    """
    Continuously tails the current TESS JSON log.

    We observe log arrival using perf_counter_ns(), so the benchmark does not
    depend on parsing Rust's wall-clock timestamp or synchronizing clocks.
    """

    def __init__(self, path: Path) -> None:
        self.path = path
        self._events: deque[tuple[int, dict[str, Any]]] = deque()
        self._condition = __import__("threading").Condition(Lock())
        self._stop = False
        self._thread = Thread(target=self._run, name="tess-log-tail", daemon=True)

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stop = True
        with self._condition:
            self._condition.notify_all()
        self._thread.join(timeout=1.0)

    def wait_for(
        self,
        trace_id: str,
        message: str,
        timeout: float,
    ) -> tuple[int, dict[str, Any]] | None:
        deadline = time.perf_counter() + timeout

        with self._condition:
            while True:
                for observed_ns, event in self._events:
                    fields = event.get("fields", {})
                    if (
                        fields.get("trace_id") == trace_id
                        and fields.get("message") == message
                    ):
                        return observed_ns, event

                remaining = deadline - time.perf_counter()
                if remaining <= 0:
                    return None

                self._condition.wait(timeout=min(remaining, 0.01))

    def _run(self) -> None:
        # Wait for the file to exist if necessary.
        while not self._stop and not self.path.exists():
            time.sleep(0.02)

        if self._stop:
            return

        try:
            with self.path.open("r", encoding="utf-8") as file:
                # Ignore everything already in the log. Every benchmark request
                # gets a fresh UUID, so old entries cannot match anyway.
                file.seek(0, 2)

                while not self._stop:
                    line = file.readline()

                    if not line:
                        # Handle daily rotation/recreation.
                        if not self.path.exists():
                            time.sleep(0.01)
                            continue

                        time.sleep(0.001)
                        continue

                    try:
                        event = json.loads(line)
                    except json.JSONDecodeError:
                        continue

                    observed_ns = time.perf_counter_ns()

                    with self._condition:
                        self._events.append((observed_ns, event))

                        # Keep memory bounded during large benchmarks.
                        if len(self._events) > 5000:
                            self._events.popleft()

                        self._condition.notify_all()

        except (OSError, UnicodeError):
            # The core can rotate/recreate the file. The benchmark will report
            # a timeout rather than crashing on a transient log-file operation.
            return


def connect_pipe(timeout: float) -> Any:
    """Connect to the TESS Windows named pipe."""
    deadline = time.perf_counter() + timeout

    while time.perf_counter() < deadline:
        try:
            return win32file.CreateFile(
                PIPE_NAME,
                win32file.GENERIC_WRITE,
                0,
                None,
                win32file.OPEN_EXISTING,
                0,
                None,
            )
        except pywintypes.error as exc:
            # ERROR_PIPE_BUSY = 231
            # ERROR_FILE_NOT_FOUND = 2
            if exc.winerror not in (2, 231):
                raise
            time.sleep(0.05)

    raise TimeoutError(
        f"Could not connect to {PIPE_NAME}.\n" "Make sure tess-core is running."
    )


def send_event(pipe: Any, text: str) -> tuple[str, int]:
    """Send one line-delimited TranscriptEvent and return its trace ID + timestamp."""
    trace_id = uuid.uuid4().hex

    event = {
        "schema_version": 1,
        "event_type": "stt_transcript",
        "trace_id": trace_id,
        "text": text,
    }

    payload = (json.dumps(event, separators=(",", ":")) + "\n").encode("utf-8")

    start_ns = time.perf_counter_ns()
    win32file.WriteFile(pipe, payload)

    return trace_id, start_ns


def percentile(values: list[float], p: float) -> float:
    if not values:
        return float("nan")

    ordered = sorted(values)
    index = (len(ordered) - 1) * p
    lower = int(index)
    upper = min(lower + 1, len(ordered))
    fraction = index - lower

    return ordered[lower] + (ordered[upper] - ordered[lower]) * fraction


def ms(ns: int) -> float:
    return ns / 1_000_000.0


def run_benchmark(
    text: str,
    iterations: int,
    warmup: int,
    timeout: float,
    interval: float,
) -> list[dict[str, Any]]:
    log_path = wait_for_log(timeout)

    print(f"Pipe: {PIPE_NAME}")
    print(f"Log:  {log_path}")
    print(f"Command: {text!r}")
    print(f"Warmup: {warmup}")
    print(f"Iterations: {iterations}")
    print()

    tail = LogTail(log_path)
    tail.start()

    pipe = connect_pipe(timeout)

    try:
        # Warmup removes first-use costs from the measured samples.
        if warmup:
            print(f"Warming up ({warmup})...", end="", flush=True)

        for _ in range(warmup):
            trace_id, _ = send_event(pipe, text)
            result = tail.wait_for(trace_id, DISPATCH_MESSAGE, timeout)

            if result is None:
                raise TimeoutError(
                    f"Warmup command timed out waiting for '{DISPATCH_MESSAGE}'. "
                    f"trace_id={trace_id}"
                )

            if interval:
                time.sleep(interval)

        if warmup:
            print(" done")

        results: list[dict[str, Any]] = []

        print("Running benchmark...")

        for i in range(iterations):
            trace_id, send_ns = send_event(pipe, text)

            # Parser latency.
            parsed = tail.wait_for(trace_id, PARSED_MESSAGE, timeout)

            # Dispatch latency / execution boundary.
            dispatched = tail.wait_for(trace_id, DISPATCH_MESSAGE, timeout)

            if dispatched is None:
                print(
                    f"  [{i + 1:>3}/{iterations}] TIMEOUT " f"trace_id={trace_id}",
                    file=sys.stderr,
                )
                continue

            dispatch_observed_ns, _ = dispatched

            row: dict[str, Any] = {
                "iteration": i + 1,
                "trace_id": trace_id,
                "send_to_dispatch_ms": ms(dispatch_observed_ns - send_ns),
            }

            if parsed is not None:
                parsed_observed_ns, _ = parsed
                row["send_to_parsed_ms"] = ms(parsed_observed_ns - send_ns)
                row["parsed_to_dispatch_ms"] = ms(
                    dispatch_observed_ns - parsed_observed_ns
                )

            results.append(row)

            print(
                f"  [{i + 1:>3}/{iterations}] " f"{row['send_to_dispatch_ms']:8.2f} ms",
                flush=True,
            )

            if interval:
                time.sleep(interval)

        return results

    finally:
        win32file.CloseHandle(pipe)
        tail.stop()


def print_summary(results: list[dict[str, Any]]) -> None:
    if not results:
        print("\nNo successful samples.")
        return

    def summarize(key: str) -> None:
        values = [float(row[key]) for row in results if key in row]

        if not values:
            return

        print(f"\n{key}")
        print(f"  min:     {min(values):8.2f} ms")
        print(f"  mean:    {statistics.mean(values):8.2f} ms")
        print(f"  median:  {statistics.median(values):8.2f} ms")
        print(f"  p90:     {percentile(values, 0.90):8.2f} ms")
        print(f"  p95:     {percentile(values, 0.95):8.2f} ms")
        print(f"  p99:     {percentile(values, 0.99):8.2f} ms")
        print(f"  max:     {max(values):8.2f} ms")
        if len(values) > 1:
            print(f"  stddev:  {statistics.stdev(values):8.2f} ms")

    print("\n" + "=" * 42)
    print(f"Successful samples: {len(results)}")

    summarize("send_to_dispatch_ms")
    summarize("send_to_parsed_ms")
    summarize("parsed_to_dispatch_ms")

    print("=" * 42)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Benchmark TESS Python -> named pipe -> parser -> dispatch."
    )

    parser.add_argument(
        "--text",
        default="pause music",
        help="Transcript to send to TESS (default: pause music)",
    )
    parser.add_argument(
        "--iterations",
        "-n",
        type=int,
        default=50,
        help="Measured iterations (default: 50)",
    )
    parser.add_argument(
        "--warmup",
        type=int,
        default=5,
        help="Warmup iterations excluded from results (default: 5)",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=5.0,
        help="Timeout per request in seconds (default: 5)",
    )
    parser.add_argument(
        "--interval",
        type=float,
        default=0.05,
        help="Delay between requests in seconds (default: 0.05)",
    )
    parser.add_argument(
        "--json",
        dest="json_path",
        type=Path,
        help="Also write raw results to this JSON file",
    )

    args = parser.parse_args()

    if args.iterations <= 0:
        parser.error("--iterations must be > 0")
    if args.warmup < 0:
        parser.error("--warmup must be >= 0")
    if args.timeout <= 0:
        parser.error("--timeout must be > 0")
    if args.interval < 0:
        parser.error("--interval must be >= 0")

    try:
        results = run_benchmark(
            text=args.text,
            iterations=args.iterations,
            warmup=args.warmup,
            timeout=args.timeout,
            interval=args.interval,
        )
    except KeyboardInterrupt:
        print("\nInterrupted.")
        return 130
    except Exception as exc:
        print(f"\nBenchmark failed: {exc}", file=sys.stderr)
        return 1

    print_summary(results)

    if args.json_path:
        args.json_path.write_text(
            json.dumps(
                {
                    "text": args.text,
                    "iterations": args.iterations,
                    "warmup": args.warmup,
                    "results": results,
                },
                indent=2,
            ),
            encoding="utf-8",
        )
        print(f"\nRaw results: {args.json_path}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
