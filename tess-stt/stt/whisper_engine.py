from interface import STTEngine


class WhisperSTTEngine(STTEngine):
    def __init__(self, model_size: str = "tiny.en") -> None:
        from faster_whisper import WhisperModel

        self._model = WhisperModel(model_size, device="cpu", compute_type="int8")

    def transcribe(self, audio) -> str:
        segments, _info = self._model.transcribe(audio)
        return " ".join(seg.text.strip() for seg in segments)
