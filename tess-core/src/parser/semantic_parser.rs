use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use fastembed::similarity::cosine_similarity;

use crate::parser::normalizer::normalize_text;
use crate::{
    errors::ParserError,
    events::{Event, TranscriptEvent},
    parser::{
        EventParser,
        engine::{EmbeddingEngine, FastEmbedEngine, ModelSource},
    },
    registry::IntentDescriptor,
};

/// Default cosine similarity threshold required for an intent match.
pub const DEFAULT_SIMILARITY_THRESHOLD: f32 = 0.65;

/// Default minimum margin required between top match and runner-up to prevent ambiguity.
pub const DEFAULT_MIN_MARGIN: f32 = 0.05;

/// An indexed exemplar embedding representing a canonical phrase for an intent.
#[derive(Debug, Clone)]
pub struct ExemplarEmbedding {
    pub intent: &'static str,
    pub exemplar: &'static str,
    pub vector: Vec<f32>,
}

/// Semantic intent parser utilizing vector similarity over registered skill exemplars.
///
/// Under the Open-Closed Principle, this parser is decoupled from model loading:
/// it interacts exclusively with the `EmbeddingEngine` trait.
pub struct SemanticParser {
    engine: RwLock<Arc<dyn EmbeddingEngine>>,
    intent_embeddings: RwLock<Vec<ExemplarEmbedding>>,
    similarity_threshold: RwLock<f32>,
    min_margin: RwLock<f32>,
    batch_size: RwLock<Option<usize>>,
    cached_catalog: RwLock<Vec<IntentDescriptor>>,
}

impl SemanticParser {
    /// Creates a new `SemanticParser` using the default model and given threshold/margin.
    pub fn new(similarity_threshold: f32, min_margin: f32) -> Result<Self, ParserError> {
        Self::with_source(ModelSource::default(), similarity_threshold, min_margin)
    }

    /// Creates a new `SemanticParser` with a specified `ModelSource`.
    pub fn with_source(
        source: ModelSource,
        similarity_threshold: f32,
        min_margin: f32,
    ) -> Result<Self, ParserError> {
        let engine = FastEmbedEngine::try_new(source)?;
        Ok(Self::from_engine(
            Arc::new(engine),
            similarity_threshold,
            min_margin,
        ))
    }

    /// Creates a new `SemanticParser` using any custom `EmbeddingEngine` implementation.
    pub fn from_engine(
        engine: Arc<dyn EmbeddingEngine>,
        similarity_threshold: f32,
        min_margin: f32,
    ) -> Self {
        Self {
            engine: RwLock::new(engine),
            intent_embeddings: RwLock::new(Vec::new()),
            similarity_threshold: RwLock::new(similarity_threshold),
            min_margin: RwLock::new(min_margin),
            batch_size: RwLock::new(None),
            cached_catalog: RwLock::new(Vec::new()),
        }
    }

    /// Sets the batch size used when computing embeddings for catalogs.
    pub fn set_batch_size(&self, batch_size: Option<usize>) {
        if let Ok(mut bs) = self.batch_size.write() {
            *bs = batch_size;
        }
    }

    /// Returns a shared reference to the active embedding engine.
    pub fn engine(&self) -> Arc<dyn EmbeddingEngine> {
        self.engine
            .read()
            .expect("poisoned lock on embedding engine")
            .clone()
    }

    /// Swaps the current embedding model with a new `ModelSource`.
    ///
    /// Automatically re-indexes the registered intent catalog with the new model.
    pub fn swap_model(&self, source: ModelSource) -> Result<(), ParserError> {
        let new_engine = FastEmbedEngine::try_new(source)?;
        self.swap_engine(Arc::new(new_engine))
    }

    /// Swaps the current embedding engine with any custom `EmbeddingEngine`.
    ///
    /// Automatically re-indexes the registered intent catalog with the new engine.
    pub fn swap_engine(&self, new_engine: Arc<dyn EmbeddingEngine>) -> Result<(), ParserError> {
        let catalog = self
            .cached_catalog
            .read()
            .map_err(|_| ParserError::Parse(anyhow::anyhow!("poisoned lock on cached catalog")))?
            .clone();

        if !catalog.is_empty() {
            self.reindex_catalog(&new_engine, &catalog)?;
        }

        let mut engine_guard = self.engine.write().map_err(|_| {
            ParserError::Parse(anyhow::anyhow!("poisoned lock on embedding engine"))
        })?;
        *engine_guard = new_engine;

        tracing::info!("successfully swapped embedding engine and updated catalog embeddings");
        Ok(())
    }

    /// Dynamically updates the confidence threshold and minimum margin.
    pub fn set_thresholds(&self, similarity_threshold: f32, min_margin: f32) {
        if let Ok(mut st) = self.similarity_threshold.write() {
            *st = similarity_threshold;
        }
        if let Ok(mut mm) = self.min_margin.write() {
            *mm = min_margin;
        }
    }

    /// Internal helper to re-embed all exemplars in the provided catalog.
    fn reindex_catalog(
        &self,
        engine: &Arc<dyn EmbeddingEngine>,
        catalog: &[IntentDescriptor],
    ) -> Result<(), ParserError> {
        let mut all_texts = Vec::new();
        let mut metadata = Vec::new();

        for descriptor in catalog {
            for &exemplar in descriptor.exemplars {
                all_texts.push(exemplar);
                metadata.push((descriptor.id, exemplar));
            }
        }

        if all_texts.is_empty() {
            let mut guard = self.intent_embeddings.write().map_err(|_| {
                ParserError::Parse(anyhow::anyhow!("poisoned lock on intent embeddings"))
            })?;
            guard.clear();
            return Ok(());
        }

        tracing::info!(
            exemplar_count = all_texts.len(),
            "batch computing embeddings for intent catalog"
        );

        let batch_size = *self
            .batch_size
            .read()
            .map_err(|_| ParserError::Parse(anyhow::anyhow!("poisoned lock on batch size")))?;
        let vectors = engine.embed(&all_texts, batch_size)?;

        let mut new_embeddings = Vec::with_capacity(vectors.len());
        for ((intent, exemplar), vector) in metadata.into_iter().zip(vectors) {
            new_embeddings.push(ExemplarEmbedding {
                intent,
                exemplar,
                vector,
            });
        }

        let mut guard = self.intent_embeddings.write().map_err(|_| {
            ParserError::Parse(anyhow::anyhow!("poisoned lock on intent embeddings"))
        })?;
        *guard = new_embeddings;

        Ok(())
    }
}

impl Default for SemanticParser {
    fn default() -> Self {
        Self::new(DEFAULT_SIMILARITY_THRESHOLD, DEFAULT_MIN_MARGIN)
            .expect("fatal: failed to initialize default semantic parser model")
    }
}

impl EventParser for SemanticParser {
    fn init(&self) -> Result<(), ParserError> {
        let engine = self
            .engine
            .read()
            .map_err(|_| ParserError::Parse(anyhow::anyhow!("poisoned lock on embedding engine")))?
            .clone();

        tracing::info!("warming up semantic parser embedding engine");
        let _ = engine.embed(&["warmup query"], Some(1))?;

        let catalog = self
            .cached_catalog
            .read()
            .map_err(|_| ParserError::Parse(anyhow::anyhow!("poisoned lock on cached catalog")))?
            .clone();

        if !catalog.is_empty() {
            self.reindex_catalog(&engine, &catalog)?;
        }

        tracing::info!("semantic parser initialized successfully");
        Ok(())
    }

    fn load_catalog(&self, catalog: &[IntentDescriptor]) -> Result<(), ParserError> {
        let mut cat_guard = self
            .cached_catalog
            .write()
            .map_err(|_| ParserError::Parse(anyhow::anyhow!("poisoned lock on cached catalog")))?;
        *cat_guard = catalog.to_vec();

        let engine = self
            .engine
            .read()
            .map_err(|_| ParserError::Parse(anyhow::anyhow!("poisoned lock on embedding engine")))?
            .clone();

        self.reindex_catalog(&engine, catalog)
    }

    fn parse(&self, event: Arc<TranscriptEvent>) -> Result<Vec<Event>, ParserError> {
        let text = normalize_text(&event.text);

        let embeddings_guard = self.intent_embeddings.read().map_err(|_| {
            ParserError::Parse(anyhow::anyhow!("poisoned lock on intent embeddings"))
        })?;

        if embeddings_guard.is_empty() {
            tracing::warn!("semantic parser invoked with empty exemplar catalog");
            return Ok(vec![]);
        }

        let engine = self
            .engine
            .read()
            .map_err(|_| ParserError::Parse(anyhow::anyhow!("poisoned lock on embedding engine")))?
            .clone();

        let query_embeddings = engine.embed(&[&text], Some(1))?;
        let query_vec = match query_embeddings.first() {
            Some(v) => v,
            None => return Ok(vec![]),
        };

        // Score all intents by maximum similarity across their respective exemplars
        let mut intent_scores: HashMap<&'static str, f32> = HashMap::new();

        for exemplar in embeddings_guard.iter() {
            let score = cosine_similarity(query_vec, &exemplar.vector);
            intent_scores
                .entry(exemplar.intent)
                .and_modify(|s| *s = s.max(score))
                .or_insert(score);
        }

        let mut ranked: Vec<(&'static str, f32)> = intent_scores.into_iter().collect();
        ranked.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));

        if ranked.is_empty() {
            return Ok(vec![]);
        }

        let (top_intent, top_score) = ranked[0];
        let threshold = *self.similarity_threshold.read().unwrap();
        let min_margin = *self.min_margin.read().unwrap();

        if top_score < threshold {
            tracing::debug!(
                text = %text,
                top_intent = %top_intent,
                top_score = %top_score,
                threshold = %threshold,
                "semantic match below threshold"
            );
            tracing::info!(trace_id = %&event.trace_id ,"No commands Found");
            return Ok(vec![]);
        }

        // Validate margin against runner-up to eliminate ambiguous false positives
        if ranked.len() > 1 {
            let (runner_up_intent, runner_up_score) = ranked[1];
            let margin = top_score - runner_up_score;
            if margin < min_margin {
                tracing::warn!(
                    text = %text,
                    top_intent = %top_intent,
                    top_score = %top_score,
                    runner_up_intent = %runner_up_intent,
                    runner_up_score = %runner_up_score,
                    margin = %margin,
                    min_margin = %min_margin,
                    "semantic match ambiguous between top candidates"
                );
                return Ok(vec![]);
            }
        }

        tracing::info!(
            text = %text,
            intent = %top_intent,
            confidence = %top_score,
            "semantic intent recognized"
        );

        Ok(vec![Event {
            trace_id: event.trace_id.clone(),
            intent: top_intent.to_string(),
            args: HashMap::new(),
            confidence: top_score,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventType;

    struct MockEngine;

    impl EmbeddingEngine for MockEngine {
        fn embed(
            &self,
            texts: &[&str],
            _batch_size: Option<usize>,
        ) -> Result<Vec<Vec<f32>>, ParserError> {
            Ok(texts
                .iter()
                .map(|t| match *t {
                    "pause music" | "pause" | "stop playback" => vec![1.0, 0.0, 0.0],
                    "play music" | "resume song" => vec![0.0, 1.0, 0.0],
                    "set timer" => vec![0.0, 0.0, 1.0],
                    _ => vec![0.5, 0.5, 0.5],
                })
                .collect())
        }
    }

    #[test]
    fn test_semantic_parser_mock_classification() {
        let parser = SemanticParser::from_engine(Arc::new(MockEngine), 0.70, 0.05);

        let catalog = vec![
            IntentDescriptor::new(
                "media.pause",
                "Pause playback",
                &[],
                &["pause music", "stop playback"],
            ),
            IntentDescriptor::new("media.play", "Resume playback", &[], &["play music"]),
        ];

        parser.load_catalog(&catalog).unwrap();

        let event = Arc::new(TranscriptEvent {
            schema_version: 1,
            event_type: EventType::SttTranscript,
            trace_id: "trace-123".into(),
            text: "pause music".into(),
        });

        let results = parser.parse(event).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].intent, "media.pause");
        assert!(results[0].confidence > 0.99);
    }

    #[test]
    fn test_threshold_rejection() {
        let parser = SemanticParser::from_engine(Arc::new(MockEngine), 0.90, 0.05);
        let catalog = vec![IntentDescriptor::new(
            "media.pause",
            "Pause playback",
            &[],
            &["pause music"],
        )];
        parser.load_catalog(&catalog).unwrap();

        let event = Arc::new(TranscriptEvent {
            schema_version: 1,
            event_type: EventType::SttTranscript,
            trace_id: "trace-123".into(),
            text: "something completely random".into(),
        });

        let results = parser.parse(event).unwrap();
        assert!(results.is_empty());
    }
}
