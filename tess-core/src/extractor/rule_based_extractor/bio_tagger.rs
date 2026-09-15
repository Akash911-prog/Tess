use std::{collections::HashMap, fs::File, io::BufWriter};

use fst::Map;
use memmap::Mmap;
use serde::Deserialize;
use tokio::fs::{self};

#[derive(Debug, Clone, Deserialize)]
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
    pub char2idx: HashMap<String, usize>,
    pub idx2tag: HashMap<usize, String>,
    pub tag2idx: HashMap<String, usize>,
    pub pad_token: String,
    pub unk_token: String,
}

impl Vocab {
    async fn load() -> Result<Self, anyhow::Error> {
        let cwd = std::env::current_dir().unwrap();
        let model_path = cwd.join(r"models\biotagger");
        let vocab_path = cwd.join(r"models\biotagger\vocab.json");
        let fst_path = model_path.join("word2idx.fst");

        if !fst_path.exists() {
            let wtr = BufWriter::new(File::create(fst_path.clone())?);

            let json = fs::read_to_string(vocab_path.clone()).await?;
            let w2i = serde_json::from_str::<HashMap<String, u64>>(&json)?;

            let mut builder = fst::MapBuilder::new(wtr)?;
            let mut keys: Vec<&String> = w2i.keys().collect();
            keys.sort();
            for key in keys {
                builder.insert(key, w2i[key])?;
            }

            builder.finish()?;
        }

        let metadata_path = model_path.join("metadata.json");

        let json = fs::read_to_string(metadata_path).await?;

        let vocab_metadata: VocabMetadata =
            tokio::task::spawn_blocking(move || serde_json::from_str::<VocabMetadata>(&json))
                .await??;

        let mmap = unsafe { Mmap::map(&File::open(&fst_path)?)? };
        let map = Map::new(mmap)?;

        let vocab = Self {
            word2idx: map,
            char2idx: vocab_metadata.char2idx,
            idx2tag: vocab_metadata.idx2tag,
            tag2idx: vocab_metadata.tag2idx,
            pad_token: vocab_metadata.pad_token,
            unk_token: vocab_metadata.unk_token,
        };

        tracing::info!("Loaded biotagger vocab");

        Ok(vocab)
    }
}

#[derive(Debug)]
pub struct BioTagger {
    pub vocab: Vocab,
}

impl BioTagger {
    pub async fn new() -> Self {
        let vocab = Vocab::load().await.unwrap();
        Self { vocab }
    }

    pub fn tag(&self, text: &str) -> Vec<String> {
        let mut tags = Vec::new();
        let mut tokens = text.split_whitespace();

        while let Some(token) = tokens.next() {
            let token = token.to_lowercase();
            if let Some(tag) = self.vocab.word2idx.get(&token) {
                tags.push(tag.to_string());
            } else {
                tags.push(token);
            }
        }

        tags
    }
}
