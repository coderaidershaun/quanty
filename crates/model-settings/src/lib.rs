//! The model choice shared by the desktop app, CLI tools, MCP server and OCR.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The model used when no choice has been saved or configured.
pub const DEFAULT_CODEX_MODEL: &str = "gpt-6.1-sol";
/// The model identifiers and labels offered by the desktop picker.
pub const MODEL_CHOICES: &[(&str, &str)] = &[
    ("gpt-6.1-sol", "Codex · GPT-6.1 Sol"),
    ("gpt-6-astra", "Codex · GPT-6 Astra"),
    ("gpt-6-sol", "Codex · GPT-6 Sol"),
    ("gpt-6-luna", "Codex · GPT-6 Luna"),
    ("gpt-5.6-sol", "Codex · GPT-5.6 Sol"),
    ("gpt-5.6-terra", "Codex · GPT-5.6 Terra"),
    ("gpt-5.6-luna", "Codex · GPT-5.6 Luna"),
    ("gpt-reserve", "Codex · GPT Reserve"),
    ("fable", "Claude · Fable"),
    ("opus", "Claude · Opus"),
    ("sonnet", "Claude · Sonnet"),
    ("haiku", "Claude · Haiku"),
];
/// Effort names accepted by the supported agent commands.
pub const THINKING_LEVELS: &[&str] = &["low", "medium", "high", "xhigh", "max", "ultra"];
/// The saved selection in the project home folder.
pub const MODEL_SETTINGS_FILE: &str = ".quanty-model.json";

/// Failures while reading, validating or saving a model selection.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("could not read model settings at {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid model settings at {}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not load model defaults from {}", path.display())]
    Environment {
        path: PathBuf,
        #[source]
        source: dotenvy::Error,
    },
    #[error("unsupported LLM_PROVIDER {provider:?}; expected codex or claude")]
    Provider { provider: String },
    #[error("model cannot be empty")]
    EmptyModel,
    #[error("unsupported thinking level {thinking:?} for {provider} model {model:?}")]
    Thinking {
        provider: String,
        model: String,
        thinking: String,
    },
    #[error("could not serialize model settings for {}", path.display())]
    Serialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not save model settings at {}", path.display())]
    Save {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// One model and effort choice shared by all model clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelSettings {
    pub provider: String,
    pub model: String,
    pub thinking: String,
}
impl Default for ModelSettings {
    fn default() -> Self {
        Self {
            provider: "codex".into(),
            model: DEFAULT_CODEX_MODEL.into(),
            thinking: "medium".into(),
        }
    }
}
impl ModelSettings {
    /// Whether the current selection permits the ultra effort level.
    pub fn supports_ultra(&self) -> bool {
        self.provider == "codex" && !self.model.contains("luna") && self.model != "gpt-reserve"
    }
    /// Select a picker model, lowering ultra effort if the model cannot use it.
    pub fn select(&mut self, model: &str) {
        self.model = model.into();
        self.provider = if model.starts_with("gpt-") {
            "codex"
        } else {
            "claude"
        }
        .into();
        if self.thinking == "ultra" && !self.supports_ultra() {
            self.thinking = "high".into();
        }
    }
    /// Check the provider, model name and effort before use.
    ///
    /// # Errors
    /// Returns [`SettingsError`] with the invalid selection.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if !["codex", "claude"].contains(&self.provider.as_str()) {
            return Err(SettingsError::Provider {
                provider: self.provider.clone(),
            });
        }
        if self.model.trim().is_empty() {
            return Err(SettingsError::EmptyModel);
        }
        if !THINKING_LEVELS.contains(&self.thinking.as_str())
            || (self.thinking == "ultra" && !self.supports_ultra())
        {
            return Err(SettingsError::Thinking {
                provider: self.provider.clone(),
                model: self.model.clone(),
                thinking: self.thinking.clone(),
            });
        }
        Ok(())
    }
    /// Locate the nearest project settings file from the working directory.
    pub fn path() -> PathBuf {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        cwd.ancestors()
            .find(|p| p.join(".env").is_file() || p.join(MODEL_SETTINGS_FILE).is_file())
            .unwrap_or(&cwd)
            .join(MODEL_SETTINGS_FILE)
    }
    /// Load the selection from the nearest project home.
    ///
    /// # Errors
    /// See [`Self::load_from`].
    pub fn load() -> Result<Self, SettingsError> {
        Self::load_from(&Self::path())
    }
    /// Read the saved selection, or environment defaults when the file is absent.
    ///
    /// # Errors
    /// Returns [`SettingsError`] for unreadable files, malformed data or invalid choices.
    pub fn load_from(path: &Path) -> Result<Self, SettingsError> {
        match std::fs::read(path) {
            Ok(bytes) => {
                let settings: Self =
                    serde_json::from_slice(&bytes).map_err(|source| SettingsError::Parse {
                        path: path.to_owned(),
                        source,
                    })?;
                settings.validate()?;
                return Ok(settings);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(SettingsError::Read {
                    path: path.to_owned(),
                    source,
                });
            }
        }
        let mut settings = Self::default();
        let env_path = path.with_file_name(".env");
        let mut values = std::collections::HashMap::new();
        let environment_error = |source| SettingsError::Environment {
            path: env_path.clone(),
            source,
        };
        match dotenvy::from_path_iter(&env_path) {
            Ok(entries) => {
                for entry in entries {
                    let (key, value) = entry.map_err(environment_error)?;
                    values.entry(key).or_insert(value);
                }
            }
            Err(error) if error.not_found() => {}
            Err(error) => return Err(environment_error(error)),
        }
        let value = |key: &str| {
            std::env::var(key)
                .ok()
                .or_else(|| values.get(key).cloned())
                .filter(|v| !v.trim().is_empty())
        };
        if let Some(provider) = value("LLM_PROVIDER") {
            settings.provider = provider;
        }
        settings.model = value("LLM_MODEL").unwrap_or_else(|| {
            if settings.provider == "claude" {
                "sonnet".into()
            } else {
                DEFAULT_CODEX_MODEL.into()
            }
        });
        if let Some(thinking) = value("LLM_THINKING") {
            settings.thinking = thinking;
        }
        settings.validate()?;
        Ok(settings)
    }
    /// Atomically replace the saved choice after validating it.
    ///
    /// # Errors
    /// Returns [`SettingsError`] when validation, serialization or writing fails.
    pub fn save_to(&self, path: &Path) -> Result<(), SettingsError> {
        use std::io::Write;
        self.validate()?;
        let save_error = |source| SettingsError::Save {
            path: path.to_owned(),
            source,
        };
        let bytes = serde_json::to_vec_pretty(self).map_err(|source| SettingsError::Serialize {
            path: path.to_owned(),
            source,
        })?;
        let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(Path::new(".")))
            .map_err(save_error)?;
        file.write_all(&bytes).map_err(save_error)?;
        file.persist(path)
            .map_err(|error| save_error(error.error))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_and_switches_round_trip_without_touching_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(MODEL_SETTINGS_FILE);
        let mut settings = ModelSettings::default();
        assert_eq!(settings.provider, "codex");
        settings.save_to(&path).unwrap();
        assert_eq!(ModelSettings::load_from(&path).unwrap(), settings);
        settings.thinking = "ultra".into();
        settings.select("haiku");
        assert_eq!(settings.provider, "claude");
        assert_eq!(settings.thinking, "high");
        settings.save_to(&path).unwrap();
        assert_eq!(ModelSettings::load_from(&path).unwrap(), settings);
        settings.select("gpt-6-luna");
        assert!(!settings.supports_ultra());
        settings.thinking = "invalid".into();
        assert!(settings.save_to(&path).is_err());
        std::fs::write(&path, "broken").unwrap();
        assert!(ModelSettings::load_from(&path).is_err());
    }
}
