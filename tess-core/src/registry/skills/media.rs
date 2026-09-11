use crate::registry::{BoxFuture, IntentDescriptor, Skill};

pub struct MediaSkill;

impl Skill for MediaSkill {
    fn name(&self) -> &'static str {
        "media"
    }

    fn intents(&self) -> Vec<IntentDescriptor> {
        vec![
            IntentDescriptor::new(
                "media.pause",
                "Pause playback",
                &[],
                &["pause music", "stop playback", "pause song"],
            ),
            IntentDescriptor::new(
                "media.play",
                "Resume playback",
                &[],
                &["play music", "resume music", "start the song"],
            ),
            IntentDescriptor::new(
                "media.next",
                "Skip to the next track",
                &[],
                &[
                    "next song",
                    "skip this track",
                    "play next track",
                    "skip to next song",
                    "next track please",
                ],
            ),
            IntentDescriptor::new(
                "media.previous",
                "Skip to the previous track",
                &[],
                &[
                    "previous song",
                    "go back to the previous track",
                    "play previous track",
                    "previous track please",
                ],
            ),
            IntentDescriptor::new(
                "media.skip",
                "Skip video/media by a certain duration",
                &[],
                &[
                    "skip",
                    "skip 20 seconds",
                    "forward 30 seconds",
                    "go ahead by a minute",
                    "skip 10 minutes",
                ],
            ),
            IntentDescriptor::new(
                "media.back",
                "rewind video/media by a certain duration",
                &["optional: duration"],
                &[
                    "rewind",
                    "rewind 20 seconds",
                    "go back 30 seconds",
                    "go back by a minute",
                    "rewind 10 minutes",
                ],
            ),
        ]
    }

    fn execute<'a>(
        &'a self,
        _command: &'a crate::events::Event,
    ) -> BoxFuture<'a, Result<crate::registry::ExecutionResult, anyhow::Error>> {
        todo!()
    }
}
