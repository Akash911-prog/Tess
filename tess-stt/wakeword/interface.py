from abc import ABC, abstractmethod


class WakeWordDetector(ABC):
    @abstractmethod
    def detect(self, audio) -> str: ...
