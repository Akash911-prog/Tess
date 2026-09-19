use anyhow::Ok;

use crate::registry::{ArgKind, ArgSpec, ExecutionResult, skill::RiskLevel};

crate::skill! {
    struct MediaSkill;
    name = "media";

    intent "media.pause" {
        desc: "Pause playback",
        args: [ArgSpec::optional("app", ArgKind::Text)],
        exemplars: ["pause music", "stop playback", "pause song", "stop the track"],
        risk_level: RiskLevel::Low,
    }

    intent "media.play" {
        desc: "Resume playback",
        args: [ArgSpec::optional("app", ArgKind::Text)],
        exemplars: ["play music", "resume music", "start the song", "unpause", "continue"],
        risk_level: RiskLevel::Low,
    }

    intent "media.next" {
        desc: "Skip to the next track",
        args: [],
        exemplars: [
            "next song",
            "skip this track",
            "play next track",
            "skip to next song",
            "next track please",
        ],
        risk_level: RiskLevel::Low,
    }

    intent "media.previous" {
        desc: "Skip to the previous track",
        args: [],
        exemplars: [
            "previous song",
            "go back to the previous track",
            "play previous track",
            "previous track please",
        ],
        risk_level: RiskLevel::Low,
    }

    intent "media.skip" {
        desc: "Skip video/media forward by a duration",
        args: [ArgSpec::optional("duration", ArgKind::Duration)],
        exemplars: [
            "skip",
            "skip 20 seconds",
            "forward 30 seconds",
            "go ahead by a minute",
            "skip 10 minutes",
        ],
        risk_level: RiskLevel::Low,
    }

    intent "media.back" {
        desc: "Rewind video/media by a duration",
        args: [ArgSpec::optional("duration", ArgKind::Duration)],
        exemplars: [
            "rewind",
            "rewind 20 seconds",
            "go back 30 seconds",
            "go back by a minute",
            "rewind 10 minutes",
        ],
        risk_level: RiskLevel::Low,
    }

    execute(command) {
        println!("MediaSkill::execute({:?})", command);
        Ok(ExecutionResult::success())
    }
}
