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


def check():
    import pyaudio

    pa = pyaudio.PyAudio()

    # What's currently the default input device?
    default = pa.get_default_input_device_info()
    print(f"Default input: [{default['index']}] {default['name']}")

    # List every available input device
    print("\nAll input devices:")
    for i in range(pa.get_device_count()):
        info = pa.get_device_info_by_index(i)
        if info["maxInputChannels"] > 0:
            print(f"  [{info['index']}] {info['name']}")

    pa.terminate()


if __name__ == "__main__":
    check()
    asyncio.run(main())
