use std::sync::Arc;
use tess_core::{
    events::{EventType, TranscriptEvent},
    parser::{EventParser, semantic_parser::SemanticParser},
    registry::IntentDescriptor,
};

#[test]
fn test_model_intent_matching_in_isolation() {
    let parser = SemanticParser::default();

    let catalog = vec![
        IntentDescriptor::new(
            "media.pause",
            "Pause playback",
            &["pause music", "stop playback", "pause song"],
        ),
        IntentDescriptor::new(
            "media.play",
            "Resume playback",
            &["play music", "resume playback", "start the song"],
        ),
        IntentDescriptor::new(
            "system.volume_up",
            "Volume increase",
            &["turn up volume", "increase sound", "make it louder"],
        ),
    ];

    parser
        .load_catalog(&catalog)
        .expect("Failed to load catalog");

    // 1. Direct match
    let event = Arc::new(TranscriptEvent {
        schema_version: 1,
        event_type: EventType::SttTranscript,
        trace_id: "test-direct".into(),
        text: "pause music".into(),
    });
    let results = parser.parse(event).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].intent, "media.pause");
    assert!(results[0].confidence > 0.85);

    // 2. Paraphrased semantic match
    let event = Arc::new(TranscriptEvent {
        schema_version: 1,
        event_type: EventType::SttTranscript,
        trace_id: "test-para".into(),
        text: "pump up the sound".into(),
    });
    let results = parser.parse(event).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].intent, "system.volume_up");

    // 3. Out-of-domain query (should be rejected below threshold)
    let event = Arc::new(TranscriptEvent {
        schema_version: 1,
        event_type: EventType::SttTranscript,
        trace_id: "test-ood".into(),
        text: "what is the diameter of Jupiter?".into(),
    });
    let results = parser.parse(event).unwrap();
    assert!(results.is_empty(), "Out-of-domain query should be rejected");
}
