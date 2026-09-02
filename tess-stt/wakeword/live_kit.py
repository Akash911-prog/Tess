from interface import WakeWordDetector


class LiveKitWakeWordDetector(WakeWordDetector):
    def __init__(self, models: list[str]):
        self.models = models
        self.threshold = 0.1
        self.debounce = 2.0

    def detect(self, audio) -> str:
        return ""
