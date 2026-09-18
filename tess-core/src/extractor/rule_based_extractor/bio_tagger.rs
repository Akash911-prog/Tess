use std::{collections::HashMap, fs::File, path::Path};

use fst::Map;
use memmap::Mmap;
use regex::Regex;
use serde::Deserialize;
use tokio::fs;
use tract::prelude::*;

#[derive(Debug)]
pub enum Tags {
    BTarget,
    ITarget,
    O,
}

impl From<&String> for Tags {
    fn from(tag: &String) -> Self {
        match tag.as_str() {
            "B-TARGET" => Tags::BTarget,
            "I-TARGET" => Tags::ITarget,
            "O" => Tags::O,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct VocabMetadata {
    pub char2idx: HashMap<String, usize>,
    pub idx2tag: HashMap<usize, String>,
    pub tag2idx: HashMap<String, usize>,
    pub pad_token: String,
    pub unk_token: String,
}

#[derive(Debug)]
pub struct Vocab {
    pub word2idx: Map<Mmap>,

    // ASCII lookup table.
    // Much smaller and faster than HashMap<String, usize>.
    pub char2idx: [u64; 128],

    // Tag ID -> tag.
    pub idx2tag: Vec<String>,

    pub tag2idx: HashMap<String, usize>,

    pub pad_token: String,
    pub unk_token: String,

    // Avoid looking up <UNK> for every unknown word.
    pub unk_word_id: u64,
}

impl Vocab {
    async fn load() -> Result<Self, anyhow::Error> {
        let cwd = std::env::current_dir()?;
        let model_path = cwd.join(r"models\biotagger");

        let vocab_path = model_path.join("vocab.json");
        let fst_path = model_path.join("word2idx.fst");
        let metadata_path = model_path.join("metadata.json");

        /*
         * Build FST once if it doesn't exist.
         */
        if !fst_path.exists() {
            let json = fs::read_to_string(&vocab_path).await?;

            let w2i = tokio::task::spawn_blocking(move || {
                serde_json::from_str::<HashMap<String, u64>>(&json)
            })
            .await??;

            // FST requires sorted keys.
            let mut entries: Vec<(String, u64)> = w2i.into_iter().collect();
            entries.sort_unstable_by(|a, b| a.0.cmp(&b.0));

            tokio::task::spawn_blocking({
                let fst_path = fst_path.clone();

                move || -> Result<(), anyhow::Error> {
                    let file = File::create(fst_path)?;
                    let mut builder = fst::MapBuilder::new(file)?;

                    for (word, idx) in entries {
                        builder.insert(word, idx)?;
                    }

                    builder.finish()?;

                    Ok(())
                }
            })
            .await??;
        }

        /*
         * Metadata is tiny compared with word2idx.
         */
        let json = fs::read_to_string(metadata_path).await?;

        let metadata =
            tokio::task::spawn_blocking(move || serde_json::from_str::<VocabMetadata>(&json))
                .await??;

        /*
         * Memory-map the FST.
         *
         * The FST itself is not copied into the heap.
         */
        let file = File::open(&fst_path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let word2idx = Map::new(mmap)?;

        /*
         * Convert char2idx into an ASCII lookup table.
         *
         * Your tokenizer only produces ASCII:
         *
         * [A-Za-z']+
         * \d+
         * punctuation
         *
         * Therefore a HashMap<String, usize> is unnecessary here.
         */
        let mut char2idx = [0u64; 128];

        for (character, idx) in metadata.char2idx {
            let bytes = character.as_bytes();

            if bytes.len() == 1 {
                let byte = bytes[0];

                if byte < 128 {
                    char2idx[byte as usize] = idx as u64;
                }
            }
        }

        /*
         * Convert idx2tag HashMap into a Vec.
         *
         * Tag IDs are normally dense:
         *
         * 0 -> O
         * 1 -> B-X
         * 2 -> I-X
         * ...
         *
         * Vec is much cheaper/faster for this.
         */
        let max_tag_id = metadata.idx2tag.keys().copied().max().unwrap_or(0);

        let mut idx2tag = vec![String::new(); max_tag_id + 1];

        for (idx, tag) in metadata.idx2tag {
            idx2tag[idx] = tag;
        }

        /*
         * Look up UNK exactly once.
         */
        let unk_word_id = word2idx
            .get(&metadata.unk_token)
            .ok_or_else(|| anyhow::anyhow!("UNK token not found in word2idx FST"))?;

        tracing::info!("Loaded biotagger vocab");

        Ok(Self {
            word2idx,
            char2idx,
            idx2tag,
            tag2idx: metadata.tag2idx,
            pad_token: metadata.pad_token,
            unk_token: metadata.unk_token,
            unk_word_id,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct CrfParams {
    pub transitions: Vec<Vec<f32>>,
    pub start_transitions: Vec<f32>,
    pub end_transitions: Vec<f32>,
}

impl CrfParams {
    fn validate(&self) -> Result<(), anyhow::Error> {
        let num_tags = self.start_transitions.len();

        if num_tags == 0 {
            anyhow::bail!("CRF has no tags");
        }

        if self.end_transitions.len() != num_tags {
            anyhow::bail!(
                "CRF end_transitions has {} tags, expected {}",
                self.end_transitions.len(),
                num_tags
            );
        }

        if self.transitions.len() != num_tags {
            anyhow::bail!(
                "CRF transition matrix has {} rows, expected {}",
                self.transitions.len(),
                num_tags
            );
        }

        if self.transitions.iter().any(|row| row.len() != num_tags) {
            anyhow::bail!("CRF transition matrix must be square");
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct Crf {
    params: CrfParams,
}

impl Crf {
    pub async fn load(path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let json = fs::read_to_string(path).await?;
        let params: CrfParams = serde_json::from_str(&json)?;

        Ok(Self { params })
    }

    pub fn decode(&self, emissions: &[Vec<f32>]) -> Result<Vec<usize>, anyhow::Error> {
        if emissions.is_empty() {
            return Ok(Vec::new());
        }

        const NUM_TAGS: usize = 3;

        // Validate the emission dimension once.
        for (i, scores) in emissions.iter().enumerate() {
            if scores.len() != NUM_TAGS {
                anyhow::bail!(
                    "invalid emission size at token {}: expected {}, got {}",
                    i,
                    NUM_TAGS,
                    scores.len()
                );
            }
        }

        let seq_len = emissions.len();

        /*
         * Only the current Viterbi scores are needed.
         *
         * These live on the stack:
         *     3 * 4 bytes * 2 = 24 bytes
         */
        let mut previous = [0.0f32; NUM_TAGS];
        let mut current = [0.0f32; NUM_TAGS];

        /*
         * One byte is enough to store a tag ID because we only have
         * three tags: 0, 1, 2.
         *
         * [token][current_tag] = best previous tag
         */
        let mut backpointers = vec![[0u8; NUM_TAGS]; seq_len];

        // --------------------------------------------------------------------
        // Initialization
        // --------------------------------------------------------------------

        for tag in 0..NUM_TAGS {
            previous[tag] = self.params.start_transitions[tag] + emissions[0][tag];
        }

        // --------------------------------------------------------------------
        // Viterbi forward pass
        // --------------------------------------------------------------------

        for token_idx in 1..seq_len {
            for current_tag in 0..NUM_TAGS {
                let mut best_score = f32::NEG_INFINITY;
                let mut best_previous_tag = 0usize;

                for previous_tag in 0..NUM_TAGS {
                    let score =
                        previous[previous_tag] + self.params.transitions[previous_tag][current_tag];

                    if score > best_score {
                        best_score = score;
                        best_previous_tag = previous_tag;
                    }
                }

                current[current_tag] = best_score + emissions[token_idx][current_tag];

                backpointers[token_idx][current_tag] = best_previous_tag as u8;
            }

            std::mem::swap(&mut previous, &mut current);
        }

        // --------------------------------------------------------------------
        // Add end transition
        // --------------------------------------------------------------------

        let mut best_last_tag = 0usize;
        let mut best_final_score = f32::NEG_INFINITY;

        for tag in 0..NUM_TAGS {
            let score = previous[tag] + self.params.end_transitions[tag];

            if score > best_final_score {
                best_final_score = score;
                best_last_tag = tag;
            }
        }

        // --------------------------------------------------------------------
        // Backtrack
        // --------------------------------------------------------------------

        let mut tags = vec![0usize; seq_len];

        tags[seq_len - 1] = best_last_tag;

        for token_idx in (1..seq_len).rev() {
            let current_tag = tags[token_idx];

            tags[token_idx - 1] = backpointers[token_idx][current_tag] as usize;
        }

        Ok(tags)
    }
}

#[derive(Debug)]
pub struct BioTagger {
    pub vocab: Vocab,
    pub model: Runnable,
    pub token_re: Regex,
    pub crf: Crf,
}

impl BioTagger {
    pub async fn new() -> Self {
        /*
         * Both are independent, so load concurrently.
         */
        let cwd = std::env::current_dir().expect("failed to get current directory");
        let crf_path = cwd.join(r"models\biotagger\crf_params.json");
        let (vocab, runnable, crf) =
            tokio::join!(Vocab::load(), Self::load_model(), Crf::load(crf_path));

        let token_re =
            Regex::new(r"[A-Za-z']+|\d+|[^\sA-Za-z0-9]").expect("invalid tokenizer regex");

        Self {
            vocab: vocab.expect("failed to load biotagger vocabulary"),
            model: runnable.expect("failed to load biotagger model"),
            token_re,
            crf: crf.expect("failed to load crf"),
        }
    }

    async fn load_model() -> Result<Runnable, anyhow::Error> {
        let cwd = std::env::current_dir()?;
        let model_path = cwd.join(r"models\biotagger");

        let model = tract::onnx()?
            .load(model_path.join("model.onnx"))?
            .into_model()?;

        let runtime = tract::runtime_for_name("default")?;

        let runnable = runtime.prepare(model)?;

        tracing::info!("Loaded biotagger model");

        Ok(runnable)
    }

    /*
     * Tokenize without allocating a String for every token.
     *
     * The returned &str slices point directly into `text`.
     */
    fn tokenize<'a>(&self, text: &'a str) -> Vec<&'a str> {
        self.token_re.find_iter(text).map(|m| m.as_str()).collect()
    }

    /*
     * Build word IDs.
     *
     * Output:
     *
     * [word_0, word_1, ..., word_n]
     */
    fn word_ids(&self, tokens: &[&str]) -> Vec<i64> {
        let mut ids = Vec::with_capacity(tokens.len());

        for token in tokens {
            /*
             * The Python model uses:
             *
             * word2idx.get(t.lower(), UNK)
             *
             * Do the same here.
             */
            let lower = token.to_ascii_lowercase();

            let id = self
                .vocab
                .word2idx
                .get(&lower)
                .unwrap_or(self.vocab.unk_word_id);

            ids.push(id as i64);
        }

        ids
    }

    /*
     * Build character IDs.
     *
     * Output shape:
     *
     * [seq_len, max_word_len]
     *
     * The batch dimension is added when creating the Tensor.
     */
    fn char_ids(&self, tokens: &[&str], max_word_len: usize) -> Vec<i64> {
        let seq_len = tokens.len();

        let mut chars = vec![0i64; seq_len * max_word_len];

        for (word_idx, token) in tokens.iter().enumerate() {
            /*
             * Same lowercase behavior as the Python word lookup
             * is NOT necessary for characters.
             *
             * The original Python code uses:
             *
             * for c in enumerate(t)
             *
             * so preserve the original token characters.
             */
            for (char_idx, byte) in token.bytes().enumerate() {
                if char_idx >= max_word_len {
                    break;
                }

                let id = if byte < 128 {
                    self.vocab.char2idx[byte as usize]
                } else {
                    0
                };

                chars[word_idx * max_word_len + char_idx] = id as i64;
            }
        }

        chars
    }

    /*
     * Run ONNX and return raw emission scores.
     *
     * No CRF/Viterbi yet.
     *
     * Returns:
     *
     * Vec<Vec<f32>>
     *
     * [sequence][tag]
     */
    pub fn emissions(&self, text: &str) -> Result<Vec<Vec<f32>>, anyhow::Error> {
        let tokens = self.tokenize(text);

        if tokens.is_empty() {
            return Ok(Vec::new());
        }

        let seq_len = tokens.len();

        /*
         * Find maximum token length.
         *
         * `.len()` is byte length, which is what we want because
         * the tokenizer is ASCII-oriented.
         */
        let max_word_len = tokens.iter().map(|token| token.len()).max().unwrap_or(1);

        /*
         * WORD IDS
         *
         * [1, seq_len]
         *
         */
        let word_ids_vec = self.word_ids(&tokens);
        let word_ids = Tensor::from_slice(&[1, seq_len], &word_ids_vec)?;

        /*
         * CHAR IDS
         *
         * [1, seq_len, max_word_len]
         */
        let chars_vec = self.char_ids(&tokens, max_word_len);
        let chars = Tensor::from_slice(&[1, seq_len, max_word_len], &chars_vec)?;

        /*
         * IMPORTANT:
         *
         * This assumes the ONNX inputs are in the same order
         * as the original model:
         *
         * 0 = word_ids
         * 1 = mask
         * 2 = lengths
         * 3 = chars
         *
         * We should verify this against the actual ONNX graph.
         *
         * `Runnable::run` in the new API takes tensors directly as a
         * tuple (via `IntoInputs`) and returns `Vec<Tensor>` — no
         * `tvec!`/`.into_tvalue()`/`TValue` involved.
         */
        let outputs = self
            .model
            .run((word_ids, chars))
            .expect("Model failed to run");

        /*
         * First output = emission scores.
         *
         * Expected:
         *
         * [1, seq_len, num_tags]
         *
         * `shape()` on the new Tensor returns a Result, unlike the
         * old internal Tensor's infallible `shape()`.
         */
        let emissions_tensor = &outputs[0];
        let shape = emissions_tensor.shape()?;

        if shape.len() != 3 {
            anyhow::bail!("unexpected emission tensor rank: {}", shape.len());
        }

        let num_tags = shape[2];
        let data = emissions_tensor.as_slice::<f32>()?;

        let mut result = Vec::with_capacity(seq_len);

        for word_idx in 0..seq_len {
            let start = word_idx * num_tags;
            result.push(data[start..start + num_tags].to_vec());
        }

        Ok(result)
    }

    /*
     * Convenience function for inspecting the model output.
     *
     * Viterbi will eventually replace this stage.
     */
    pub fn _tag(&self, text: &str) -> Result<Vec<(String, Vec<f32>)>, anyhow::Error> {
        let tokens = self.tokenize(text);
        let emissions = self.emissions(text)?;

        Ok(tokens
            .into_iter()
            .zip(emissions)
            .map(|(token, scores)| (token.to_owned(), scores))
            .collect())
    }

    pub fn predict(&self, text: &str) -> Result<Vec<(String, Tags)>, anyhow::Error> {
        let tokens = self.tokenize(text);

        if tokens.is_empty() {
            return Ok(Vec::new());
        }

        let emissions = self.emissions(text)?;

        let tag_ids = self.crf.decode(&emissions)?;

        let result: Vec<(String, String)> = tokens
            .into_iter()
            .zip(tag_ids)
            .map(|(token, tag_id)| {
                let tag = self
                    .vocab
                    .idx2tag
                    .get(tag_id)
                    .cloned()
                    .unwrap_or_else(|| "<INVALID>".to_owned());

                (token.to_owned(), tag)
            })
            .collect();

        let mut outputs: Vec<(String, Tags)> = Vec::new();

        for (token, tag) in result.iter() {
            let tag: Tags = tag.into();
            outputs.push((token.to_owned(), tag));
        }

        Ok(outputs)
    }
}
