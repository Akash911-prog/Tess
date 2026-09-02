from interface import WakeWordDetector
from live_kit import LiveKitWakeWordDetector


def WakeWordDetectorFactory() -> WakeWordDetector:
    return LiveKitWakeWordDetector([""])
