use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use fastembed::EmbeddingModel;
use sysinfo::{Pid, ProcessesToUpdate, System};

use tess_core::{
    events::{EventType, TranscriptEvent},
    parser::{EventParser, engine::ModelSource, semantic_parser::SemanticParser},
    registry::IntentDescriptor,
};

/// Returns the current process's resident memory in MB.
fn current_ram_mb(system: &mut System) -> f64 {
    let pid = Pid::from_u32(std::process::id());

    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);

    system
        .process(pid)
        .map(|process| process.memory() as f64 / (1024.0 * 1024.0))
        .unwrap_or(0.0)
}

/// Default intent catalog representing common voice assistant skills and anchor exemplars.
fn build_sample_catalog() -> Vec<IntentDescriptor> {
    vec![
        IntentDescriptor::new(
            "media.pause",
            "Pauses ongoing audio or video playback",
            &[
                "pause music",
                "pause the song",
                "stop playback",
                "pause track",
                "hold the music",
                "pause the audio",
            ],
        ),
        IntentDescriptor::new(
            "media.play",
            "Resumes or starts playback",
            &[
                "play music",
                "resume music",
                "start playback",
                "continue the song",
                "resume playback",
                "play the track",
            ],
        ),
        IntentDescriptor::new(
            "media.next",
            "Skips to the next track",
            &[
                "next song",
                "skip this track",
                "play next track",
                "skip to next song",
                "next track please",
            ],
        ),
        IntentDescriptor::new(
            "system.volume_up",
            "Increases the system output volume",
            &[
                "turn up volume",
                "increase sound",
                "make it louder",
                "volume up",
                "raise the volume",
                "boost the volume",
            ],
        ),
        IntentDescriptor::new(
            "system.volume_down",
            "Decreases the system output volume",
            &[
                "lower volume",
                "turn down volume",
                "decrease sound",
                "make it quieter",
                "volume down",
                "reduce the volume",
            ],
        ),
        IntentDescriptor::new(
            "app.open",
            "Launches an application",
            &[
                "open browser",
                "launch application",
                "start the program",
                "open my browser",
                "launch chrome",
            ],
        ),
        IntentDescriptor::new(
            "timer.set",
            "Creates or starts a countdown timer",
            &[
                "set a timer for five minutes",
                "start a timer",
                "set countdown",
                "create a timer",
                "start the timer",
            ],
        ),
    ]
}

struct TestCase {
    utterance: &'static str,
    expected_intent: Option<&'static str>,
    category: &'static str,
}

fn build_test_cases() -> Vec<TestCase> {
    vec![
        TestCase {
            utterance: "pause music",
            expected_intent: Some("media.pause"),
            category: "Exact",
        },
        TestCase {
            utterance: "volume up",
            expected_intent: Some("system.volume_up"),
            category: "Exact",
        },
        TestCase {
            utterance: "freeze the song",
            expected_intent: Some("media.pause"),
            category: "Paraphrase",
        },
        TestCase {
            utterance: "pump up the sound",
            expected_intent: Some("system.volume_up"),
            category: "Paraphrase",
        },
        TestCase {
            utterance: "make the sound quieter please",
            expected_intent: Some("system.volume_down"),
            category: "Paraphrase",
        },
        TestCase {
            utterance: "jump to the next song",
            expected_intent: Some("media.next"),
            category: "Paraphrase",
        },
        TestCase {
            utterance: "launch the web browser",
            expected_intent: Some("app.open"),
            category: "Paraphrase",
        },
        TestCase {
            utterance: "start a ten minute countdown",
            expected_intent: Some("timer.set"),
            category: "Paraphrase",
        },
        TestCase {
            utterance: "what is the capital of Australia?",
            expected_intent: None,
            category: "Out-of-Domain",
        },
        TestCase {
            utterance: "tell me a funny bedtime story",
            expected_intent: None,
            category: "Out-of-Domain",
        },
        TestCase {
            utterance: "order a pepperoni pizza",
            expected_intent: None,
            category: "Out-of-Domain",
        },
    ]
}

fn determine_model_source() -> (ModelSource, String) {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        let arg = args[1].to_lowercase();

        match arg.as_str() {
            "minilm" | "all-minilm" => (
                ModelSource::predefined(EmbeddingModel::AllMiniLML6V2),
                "sentence-transformers/all-MiniLM-L6-v2 (Predefined)".to_string(),
            ),
            "bge" | "bge-small" => (
                ModelSource::predefined(EmbeddingModel::BGESmallENV15),
                "BAAI/bge-small-en-v1.5 (Predefined)".to_string(),
            ),
            "bge-base" => (
                ModelSource::predefined(EmbeddingModel::BGEBaseENV15),
                "BAAI/bge-base-en-v1.5 (Predefined)".to_string(),
            ),
            "gemma" => (
                ModelSource::predefined(EmbeddingModel::EmbeddingGemma300M),
                "gemma300m (Predefined)".to_string(),
            ),
            "gemma_q" => (
                ModelSource::predefined(EmbeddingModel::EmbeddingGemma300MQ4),
                "gemma300mQ (Predefined)".to_string(),
            ),
            path if PathBuf::from(path).exists() => (
                ModelSource::from_directory(path),
                format!("User-Defined Directory: '{path}'"),
            ),
            other => {
                println!("Unknown model '{other}', defaulting to BGE-Small-EN-v1.5.");

                (
                    ModelSource::default(),
                    "BAAI/bge-small-en-v1.5 (Default)".to_string(),
                )
            }
        }
    } else {
        (
            ModelSource::default(),
            "BAAI/bge-small-en-v1.5 (Default)".to_string(),
        )
    }
}

fn main() {
    println!("\n===============================================================");
    println!("          TESS-CORE SEMANTIC PARSER & MODEL BENCHMARK          ");
    println!("===============================================================\n");

    let mut system = System::new();

    // Establish a baseline before loading anything.
    system.refresh_processes(ProcessesToUpdate::All, true);
    let baseline_ram = current_ram_mb(&mut system);

    println!("Baseline RAM:     {:.2} MB\n", baseline_ram);

    let (model_source, model_name) = determine_model_source();

    println!("Selected Model: {model_name}");

    // -----------------------------------------------------------
    // 1. Model Load
    // -----------------------------------------------------------

    print!("[1/4] Loading embedding model into memory... ");

    let load_start = Instant::now();

    let parser = SemanticParser::with_source(model_source, 0.60, 0.04)
        .expect("Failed to initialize semantic parser with selected model");

    let load_time = load_start.elapsed();

    println!("DONE in {:.2?}", load_time);
    let model_ram = current_ram_mb(&mut system);
    println!(
        "      RAM: {:.2} MB (+{:.2} MB)",
        model_ram,
        model_ram - baseline_ram
    );

    // -----------------------------------------------------------
    // 2. Catalog Indexing
    // -----------------------------------------------------------

    let catalog = build_sample_catalog();

    let total_exemplars: usize = catalog.iter().map(|c| c.exemplars.len()).sum();

    print!(
        "[2/4] Indexing {} intent descriptors ({} total exemplars)... ",
        catalog.len(),
        total_exemplars
    );

    let index_start = Instant::now();

    parser
        .load_catalog(&catalog)
        .expect("Failed to vectorize and index catalog");

    let index_time = index_start.elapsed();

    let catalog_ram = current_ram_mb(&mut system);

    let per_exemplar_us = index_time.as_micros() as f64 / total_exemplars as f64;

    println!(
        "DONE in {:.2?} ({:.1} µs/exemplar)",
        index_time, per_exemplar_us
    );

    println!(
        "      RAM: {:.2} MB (+{:.2} MB from model load)",
        catalog_ram,
        catalog_ram - model_ram
    );

    // -----------------------------------------------------------
    // 3. Warm-up
    // -----------------------------------------------------------

    print!("[3/4] Warming up inference engine... ");

    let warmup_event = Arc::new(TranscriptEvent {
        schema_version: 1,
        event_type: EventType::SttTranscript,
        trace_id: "warmup".into(),
        text: "warmup text".into(),
    });

    let _ = parser.parse(warmup_event);

    let warmup_ram = current_ram_mb(&mut system);

    println!("DONE");
    println!("      RAM: {:.2} MB\n", warmup_ram);

    // -----------------------------------------------------------
    // 4. Accuracy Test Suite
    // -----------------------------------------------------------

    println!("[4/4] Running Intent Classification Test Suite:");

    println!(
        "---------------------------------------------------------------------------------------------------------"
    );

    println!(
        "{:<14} | {:<32} | {:<20} | {:<7} | {:<8} | {:<6}",
        "Category", "Input Utterance", "Predicted Intent", "Score", "Latency", "Result"
    );

    println!(
        "---------------------------------------------------------------------------------------------------------"
    );

    let test_cases = build_test_cases();
    let mut passed_tests = 0;

    let mut peak_ram = warmup_ram;

    for (i, tc) in test_cases.iter().enumerate() {
        let event = Arc::new(TranscriptEvent {
            schema_version: 1,
            event_type: EventType::SttTranscript,
            trace_id: format!("test-{i}"),
            text: tc.utterance.to_string(),
        });

        let query_start = Instant::now();

        let results = parser.parse(event).expect("Parsing failed unexpectedly");

        let query_latency = query_start.elapsed();

        let ram = current_ram_mb(&mut system);
        peak_ram = peak_ram.max(ram);

        let (predicted, score) = match results.first() {
            Some(cmd) => (Some(cmd.intent.as_str()), cmd.confidence),
            None => (None, 0.0),
        };

        let is_correct = predicted == tc.expected_intent;

        if is_correct {
            passed_tests += 1;
        }

        let status_str = if is_correct { "PASS" } else { "FAIL" };
        let pred_str = predicted.unwrap_or("[REJECTED]");

        println!(
            "{:<14} | {:<32} | {:<20} | {:<7.3} | {:>6.2} ms | {:<6}",
            tc.category,
            tc.utterance,
            pred_str,
            score,
            query_latency.as_secs_f64() * 1000.0,
            status_str
        );
    }

    println!(
        "---------------------------------------------------------------------------------------------------------"
    );

    println!(
        "Accuracy: {}/{} tests passed ({:.1}%)\n",
        passed_tests,
        test_cases.len(),
        (passed_tests as f64 / test_cases.len() as f64) * 100.0
    );

    // -----------------------------------------------------------
    // Latency + RAM Benchmark
    // -----------------------------------------------------------

    println!("Running Latency & Throughput Benchmark (100 iterations)...");

    let benchmark_queries = [
        "pause music",
        "turn up the volume please",
        "skip to next song",
        "open the browser now",
        "start a timer for 5 minutes",
    ];

    let mut latencies: Vec<f64> = Vec::with_capacity(100);

    for i in 0..100 {
        let query = benchmark_queries[i % benchmark_queries.len()];

        let event = Arc::new(TranscriptEvent {
            schema_version: 1,
            event_type: EventType::SttTranscript,
            trace_id: format!("bench-{i}"),
            text: query.to_string(),
        });

        let start = Instant::now();

        let _ = parser.parse(event).unwrap();

        let latency = start.elapsed().as_secs_f64() * 1000.0;

        latencies.push(latency);

        let ram = current_ram_mb(&mut system);
        peak_ram = peak_ram.max(ram);
    }

    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let min = latencies[0];
    let max = latencies[latencies.len() - 1];
    let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;

    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[(latencies.len() as f64 * 0.95) as usize];
    let p99 = latencies[(latencies.len() as f64 * 0.99) as usize];

    let qps = 1000.0 / avg;

    // Final RAM measurement.
    let final_ram = current_ram_mb(&mut system);

    // -----------------------------------------------------------
    // Batch Size Throughput Benchmark
    // -----------------------------------------------------------

    println!("\nRunning Batch Size Throughput Benchmark (embedding 128 sentences)...");
    println!(
        "-----------------------------------------------------------------------------------------"
    );
    println!(
        "{:<12} | {:<12} | {:<16} | {:<18} | {:<12}",
        "Batch Size", "Total Time", "Latency / Text", "Throughput", "Peak RAM"
    );
    println!(
        "-----------------------------------------------------------------------------------------"
    );

    let engine = parser.engine();
    let batch_test_corpus: Vec<&str> = catalog
        .iter()
        .flat_map(|c| c.exemplars.iter().copied())
        .cycle()
        .take(128)
        .collect();

    let batch_sizes = [1, 2, 4, 8, 16, 32, 64, 128];
    let mut best_qps = 0.0;
    let mut best_batch_size = 1;

    for &bs in &batch_sizes {
        let start = Instant::now();
        let _ = engine
            .embed(&batch_test_corpus, Some(bs))
            .expect("Batch embedding failed");
        let elapsed = start.elapsed();

        let ram = current_ram_mb(&mut system);
        peak_ram = peak_ram.max(ram);

        let total_ms = elapsed.as_secs_f64() * 1000.0;
        let per_text_ms = total_ms / batch_test_corpus.len() as f64;
        let throughput = batch_test_corpus.len() as f64 / elapsed.as_secs_f64();

        if throughput > best_qps {
            best_qps = throughput;
            best_batch_size = bs;
        }

        println!(
            "{:<12} | {:>9.2} ms | {:>13.2} ms | {:>10.1} texts/sec | {:>9.2} MB",
            bs, total_ms, per_text_ms, throughput, ram
        );
    }
    println!(
        "-----------------------------------------------------------------------------------------"
    );
    println!(
        "Optimal Throughput Batch Size: {} ({:.1} texts/sec)\n",
        best_batch_size, best_qps
    );

    // -----------------------------------------------------------
    // Summary
    // -----------------------------------------------------------

    println!("\n---------------------------------------------------------------");
    println!("                   BENCHMARK SUMMARY RESULTS");
    println!("---------------------------------------------------------------");

    println!("  Model:            {}", model_name);
    println!("  Model Load Time:  {:.2?}", load_time);

    println!(
        "  Catalog Indexing: {:.2?} ({} exemplars)",
        index_time, total_exemplars
    );

    println!();
    println!("  RAM:");
    println!("    Baseline:       {:.2} MB", baseline_ram);
    println!(
        "    After Model:    {:.2} MB (+{:.2} MB)",
        model_ram,
        model_ram - baseline_ram
    );
    println!(
        "    After Catalog:  {:.2} MB (+{:.2} MB)",
        catalog_ram,
        catalog_ram - baseline_ram
    );
    println!("    After Warmup:   {:.2} MB", warmup_ram);
    println!("    Peak:           {:.2} MB", peak_ram);
    println!("    Final:          {:.2} MB", final_ram);

    println!();
    println!("  Latency (Single Query):");
    println!("    Min:            {:.2} ms", min);
    println!("    Median (p50):   {:.2} ms", p50);
    println!("    Average:        {:.2} ms", avg);
    println!("    p95:            {:.2} ms", p95);
    println!("    p99:            {:.2} ms", p99);
    println!("    Max:            {:.2} ms", max);
    println!("    Throughput:     {:.1} queries/second", qps);

    println!();
    println!("  Batch Size Optimum (128 texts):");
    println!(
        "    Best Batch Size: {} ({:.1} texts/second)",
        best_batch_size, best_qps
    );

    println!("---------------------------------------------------------------\n");

    println!("Tip: Run with a specific model:");
    println!("  cargo run --example model_benchmark --release -- minilm");
    println!("  cargo run --example model_benchmark --release -- bge");
    println!("  cargo run --example model_benchmark --release -- path/to/custom_onnx_dir\n");
}
