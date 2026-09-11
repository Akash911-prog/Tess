use std::path::{Path, PathBuf};
use std::sync::Mutex;

use fastembed::{
    EmbeddingModel, InitOptionsUserDefined, TextEmbedding, TextInitOptions, TokenizerFiles,
    UserDefinedEmbeddingModel,
};

use crate::errors::ParserError;

/// Flexible model specification supporting predefined HuggingFace models,
/// custom user-defined ONNX models in memory, or local disk directories.
#[derive(Debug, Clone)]
pub enum ModelSource {
    /// Predefined model downloaded and cached by `fastembed` (e.g., BGESmallENV15, AllMiniLML6V2).
    Predefined {
        model: EmbeddingModel,
        options: Option<TextInitOptions>,
    },
    /// Custom user-defined ONNX model and tokenizer byte buffers in memory.
    UserDefined {
        model: UserDefinedEmbeddingModel,
        options: Option<InitOptionsUserDefined>,
    },
    /// User-defined model loaded from a local directory containing ONNX and tokenizer files.
    LocalDirectory {
        dir_path: PathBuf,
        onnx_filename: Option<String>,
        options: Option<InitOptionsUserDefined>,
    },
}

impl Default for ModelSource {
    fn default() -> Self {
        Self::Predefined {
            model: EmbeddingModel::EmbeddingGemma300MQ4,
            options: None,
        }
    }
}

impl ModelSource {
    /// Creates a source for a predefined `fastembed` model using default options.
    pub fn predefined(model: EmbeddingModel) -> Self {
        Self::Predefined {
            model,
            options: None,
        }
    }

    /// Creates a source for a predefined `fastembed` model with custom `TextInitOptions`.
    pub fn predefined_with_options(model: EmbeddingModel, options: TextInitOptions) -> Self {
        Self::Predefined {
            model,
            options: Some(options),
        }
    }

    /// Creates a source for an in-memory user-defined ONNX model.
    pub fn user_defined(model: UserDefinedEmbeddingModel) -> Self {
        Self::UserDefined {
            model,
            options: None,
        }
    }

    /// Creates a source for an in-memory user-defined ONNX model with custom options.
    pub fn user_defined_with_options(
        model: UserDefinedEmbeddingModel,
        options: InitOptionsUserDefined,
    ) -> Self {
        Self::UserDefined {
            model,
            options: Some(options),
        }
    }

    /// Creates a source pointing to a local directory containing model and tokenizer files.
    ///
    /// The directory is expected to contain:
    /// - `model.onnx` (or custom name via `from_directory_with_name`)
    /// - `tokenizer.json`
    /// - `config.json` (optional)
    /// - `special_tokens_map.json` (optional)
    /// - `tokenizer_config.json` (optional)
    pub fn from_directory(dir_path: impl Into<PathBuf>) -> Self {
        Self::LocalDirectory {
            dir_path: dir_path.into(),
            onnx_filename: None,
            options: None,
        }
    }

    /// Creates a source pointing to a local directory with a custom ONNX filename.
    pub fn from_directory_with_name(
        dir_path: impl Into<PathBuf>,
        onnx_filename: impl Into<String>,
    ) -> Self {
        Self::LocalDirectory {
            dir_path: dir_path.into(),
            onnx_filename: Some(onnx_filename.into()),
            options: None,
        }
    }

    /// Loads model files from explicit paths on disk.
    pub fn from_files(
        onnx_path: impl AsRef<Path>,
        tokenizer_path: impl AsRef<Path>,
    ) -> Result<Self, ParserError> {
        let onnx_bytes = std::fs::read(onnx_path.as_ref()).map_err(|e| {
            ParserError::Parse(anyhow::anyhow!(
                "failed to read ONNX file at '{}': {e}",
                onnx_path.as_ref().display()
            ))
        })?;

        let tokenizer_bytes = std::fs::read(tokenizer_path.as_ref()).map_err(|e| {
            ParserError::Parse(anyhow::anyhow!(
                "failed to read tokenizer file at '{}': {e}",
                tokenizer_path.as_ref().display()
            ))
        })?;

        let tokenizer_files = TokenizerFiles {
            tokenizer_file: tokenizer_bytes,
            config_file: b"{}".to_vec(),
            special_tokens_map_file: b"{}".to_vec(),
            tokenizer_config_file: b"{}".to_vec(),
        };

        let user_model = UserDefinedEmbeddingModel::new(onnx_bytes, tokenizer_files);
        Ok(Self::user_defined(user_model))
    }

    /// Loads the concrete `TextEmbedding` instance based on this specification.
    pub fn load(self) -> Result<TextEmbedding, ParserError> {
        match self {
            ModelSource::Predefined { model, options } => {
                let opts = options.unwrap_or_else(|| TextInitOptions::new(model));
                TextEmbedding::try_new(opts).map_err(|e| {
                    ParserError::Parse(anyhow::anyhow!("failed to load predefined model: {e}"))
                })
            }
            ModelSource::UserDefined { model, options } => {
                let opts = options.unwrap_or_default();
                TextEmbedding::try_new_from_user_defined(model, opts).map_err(|e| {
                    ParserError::Parse(anyhow::anyhow!("failed to load user-defined model: {e}"))
                })
            }
            ModelSource::LocalDirectory {
                dir_path,
                onnx_filename,
                options,
            } => {
                let onnx_name = onnx_filename.as_deref().unwrap_or("model.onnx");
                let onnx_path = dir_path.join(onnx_name);
                let onnx_bytes = std::fs::read(&onnx_path).map_err(|e| {
                    ParserError::Parse(anyhow::anyhow!(
                        "failed to read ONNX file at '{}': {e}",
                        onnx_path.display()
                    ))
                })?;

                let tokenizer_path = dir_path.join("tokenizer.json");
                let tokenizer_bytes = std::fs::read(&tokenizer_path).map_err(|e| {
                    ParserError::Parse(anyhow::anyhow!(
                        "failed to read tokenizer.json at '{}': {e}",
                        tokenizer_path.display()
                    ))
                })?;

                let read_opt = |name: &str| -> Vec<u8> {
                    let p = dir_path.join(name);
                    std::fs::read(p).unwrap_or_else(|_| b"{}".to_vec())
                };

                let tokenizer_files = TokenizerFiles {
                    tokenizer_file: tokenizer_bytes,
                    config_file: read_opt("config.json"),
                    special_tokens_map_file: read_opt("special_tokens_map.json"),
                    tokenizer_config_file: read_opt("tokenizer_config.json"),
                };

                let user_model = UserDefinedEmbeddingModel::new(onnx_bytes, tokenizer_files);
                let opts = options.unwrap_or_default();

                TextEmbedding::try_new_from_user_defined(user_model, opts).map_err(|e| {
                    ParserError::Parse(anyhow::anyhow!(
                        "failed to initialize user-defined model from '{}': {e}",
                        dir_path.display()
                    ))
                })
            }
        }
    }
}

/// Abstract contract for text embedding generation.
///
/// This trait ensures the Open-Closed Principle: `SemanticParser` does not directly
/// tie itself to `fastembed`. Any custom backend (ONNX runtime, PyTorch/Candle,
/// or a test mock) can be injected without modifying the parser.
pub trait EmbeddingEngine: Send + Sync {
    /// Computes dense vector embeddings for a slice of input strings.
    ///
    /// An optional `batch_size` can be specified to control the batch chunking for inference.
    fn embed(
        &self,
        texts: &[&str],
        batch_size: Option<usize>,
    ) -> Result<Vec<Vec<f32>>, ParserError>;
}

/// The default embedding engine backed by `fastembed`'s `TextEmbedding`.
pub struct FastEmbedEngine {
    inner: Mutex<TextEmbedding>,
    source: ModelSource,
}

impl FastEmbedEngine {
    /// Initializes a new `FastEmbedEngine` from a `ModelSource`.
    pub fn try_new(source: ModelSource) -> Result<Self, ParserError> {
        let model = source.clone().load()?;
        Ok(Self {
            inner: Mutex::new(model),
            source,
        })
    }

    /// Returns a reference to the source specification used to initialize this engine.
    pub fn source(&self) -> &ModelSource {
        &self.source
    }
}

impl EmbeddingEngine for FastEmbedEngine {
    fn embed(
        &self,
        texts: &[&str],
        batch_size: Option<usize>,
    ) -> Result<Vec<Vec<f32>>, ParserError> {
        let mut model = self.inner.lock().map_err(|_| {
            ParserError::Parse(anyhow::anyhow!("poisoned mutex on embedding model"))
        })?;

        model
            .embed(texts, batch_size)
            .map_err(|e| ParserError::Parse(anyhow::anyhow!("embedding generation failed: {e}")))
    }
}
