from interface import STTEngine
from whisper_engine import WhisperSTTEngine


def STTEngineFactory() -> STTEngine:
    return WhisperSTTEngine()
