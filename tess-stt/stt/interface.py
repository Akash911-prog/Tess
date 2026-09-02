from abc import ABC, abstractmethod


class STTEngine(ABC):
    @abstractmethod
    def transcribe(self, audio) -> str: ...
