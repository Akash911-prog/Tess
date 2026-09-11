use crate::registry::{ArgKind, ArgSpec};

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
    }

    execute(command) {
        todo!("wire up OS volume backend for '{}'", command.intent)
    }
}
