use crate::registry::{ArgKind, ArgSpec, ExecutionResult, skill::RiskLevel};

crate::skill! {
    struct SystemSkill;
    name = "system";

    intent "system.volume_up" {
        desc: "Increase the system output volume",
        args: [ArgSpec::optional("volume", ArgKind::Integer)],
        exemplars: [
            "turn up volume",
            "increase sound",
            "make it louder",
            "volume up",
            "raise the volume",
            "boost the volume",
        ],
        risk_level: RiskLevel::Low,
    }

    intent "system.volume_down" {
        desc: "Decrease the system output volume",
        args: [ArgSpec::optional("volume", ArgKind::Integer)],
        exemplars: [
            "lower volume",
            "turn down volume",
            "decrease sound",
            "make it quieter",
            "volume down",
            "reduce the volume",
        ],
        risk_level: RiskLevel::Low,
    }

    intent "system.mute" {
        desc: "Mute the system output",
        args: [ArgSpec::optional("app", ArgKind::Text)],
        exemplars: [
            "mute music",
            "silence playback",
            "mute song",
            "mute the music",
            "silence the music",
        ],
        risk_level: RiskLevel::Low,
    }

    intent "system.unmute" {
        desc: "Unmute the system output",
        args: [ArgSpec::optional("app", ArgKind::Text)],
        exemplars: [
            "unmute music",
            "unmute playback",
            "unmute song",
            "unmute the music",
            "unmute the track",
        ],
        risk_level: RiskLevel::Low,
    }

    intent "system.toggle_wifi" {
        desc: "Toggle WiFi",
        args: [ArgSpec::required("action", ArgKind::Enum(&["on", "off"])), ArgSpec::optional("target", ArgKind::Text)],
        exemplars: [
            "turn on wifi",
            "turn off wifi",
            "enable wifi",
            "disable wifi",
            "connect to wifi",
            "disconnect from wifi",
        ],
        risk_level: RiskLevel::Medium,
    }

    execute(command) {
        println!("SystemSkill::execute({:?})", command);
        Ok(ExecutionResult::success())
    }
}
