import asyncio
from livekit.wakeword import WakeWordModel, WakeWordListener
from pathlib import Path

MODEL_DIR = Path(__file__).resolve().parent / "models"
model = WakeWordModel(models=[str(MODEL_DIR / "tess.onnx")])


async def main():
    async with WakeWordListener(model, threshold=0.1, debounce=2.0) as listener:
        while True:
            print("Listening...")
            detection = await listener.wait_for_detection()
            print(f"Detected {detection.name}! ({detection.confidence:.2f})")


if __name__ == "__main__":
    asyncio.run(main())
