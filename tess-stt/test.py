"""
Manual test client for tess-core's named pipe IPC.

Lets you type arbitrary text and send it as an stt.transcript event,
without needing the real wake word / STT pipeline running at all.
Useful for testing the parser/registry/dispatch chain in isolation.

Setup:
    pip install pywin32

Usage:
    python test_client.py
    (then just type things and hit enter — "pause", "next track", etc.)
    Ctrl+C to quit.
"""

import json
import time
import uuid

import pywintypes
import win32file

PIPE_NAME = r"\\.\pipe\tess"


def connect_pipe(retries: int = 20, delay: float = 0.2):
    for attempt in range(retries):
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
        except pywintypes.error as e:
            print(
                f"  connect attempt {attempt + 1}/{retries} failed ({e.strerror}), retrying..."
            )
            time.sleep(delay)
    raise ConnectionError(
        f"could not connect to {PIPE_NAME} after {retries} attempts — is tess-core running?"
    )


def send_text(handle, text: str) -> None:
    event = {
        "schema_version": 1,
        "type": "stt_transcript",
        "trace_id": str(uuid.uuid4()),
        "text": text,
    }
    line = (json.dumps(event) + "\n").encode("utf-8")
    win32file.WriteFile(handle, line)


def main() -> None:
    print(f"Connecting to {PIPE_NAME}...")
    handle = connect_pipe()
    print("Connected. Type text to send as a transcript (Ctrl+C to quit).\n")

    try:
        while True:
            text = input("> ").strip()
            if not text:
                continue
            send_text(handle, text)
            print(f"  sent: {text!r}")
    except KeyboardInterrupt:
        print("\nClosing.")
    finally:
        win32file.CloseHandle(handle)  # type: ignore


if __name__ == "__main__":
    main()
