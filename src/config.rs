//! Config-Loading. Format kompatibel zur Linux-Variante (config.json).

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Modus {
    pub label: String,
    pub prompt: Option<String>,
    #[serde(default = "default_output")]
    pub output: String,
    #[serde(default = "default_backend")]
    pub backend: String,
}

fn default_output() -> String {
    "popup".to_string()
}
fn default_backend() -> String {
    "claude".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub modi: IndexMap<String, Modus>,

    #[serde(default = "default_picker_hotkey")]
    pub picker_hotkey: String,

    #[serde(default = "default_pill_toggle_hotkey")]
    pub pill_toggle_hotkey: String,

    #[serde(default = "default_timeout")]
    pub claude_timeout_sek: u64,
}

fn default_picker_hotkey() -> String {
    "Ctrl+Shift+Y".to_string()
}
fn default_pill_toggle_hotkey() -> String {
    // Windows: erstmal normaler Hotkey (DoubleShift braucht eigenen LL-Hook,
    // kommt in Phase 2)
    "Ctrl+Shift+P".to_string()
}
fn default_timeout() -> u64 {
    60
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("claude-hotkey")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            // Default-Config schreiben damit User einen Startpunkt hat
            let dir = path.parent().unwrap();
            std::fs::create_dir_all(dir).ok();
            let default = Self::default();
            let json = serde_json::to_string_pretty(&default)?;
            std::fs::write(path, json).ok();
            return Ok(default);
        }
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("Config-Datei lesen: {}", path.display()))?;
        let cfg: Config = serde_json::from_str(&raw)
            .with_context(|| format!("Config-JSON parsen: {}", path.display()))?;
        Ok(cfg)
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut modi = IndexMap::new();
        modi.insert("uebersetzen".into(), Modus {
            label: "🌐 Übersetzen".into(),
            prompt: Some("Übersetze den folgenden Text. Wenn er deutsch ist, übersetze ins Englische, sonst ins Deutsche. Gib NUR die Übersetzung aus, keine Erklärung, keine Anführungszeichen.".into()),
            output: "popup".into(),
            backend: "claude".into(),
        });
        modi.insert("verbessern".into(), Modus {
            label: "✨ Verbessern".into(),
            prompt: Some("Verbessere Rechtschreibung, Grammatik und Stil des folgenden Texts. Behalte den Sinn vollständig bei. Gib NUR den verbesserten Text aus, ohne Kommentar.".into()),
            output: "popup".into(),
            backend: "claude".into(),
        });
        modi.insert("zusammenfassen".into(), Modus {
            label: "📝 Zusammenfassen".into(),
            prompt: Some("Fasse den folgenden Text in 3-5 knappen Bullet-Points auf Deutsch zusammen.".into()),
            output: "popup".into(),
            backend: "claude".into(),
        });
        modi.insert("erklaeren".into(), Modus {
            label: "💡 Erklären".into(),
            prompt: Some("Erkläre den folgenden Text einfach und verständlich auf Deutsch, als würdest du es einem interessierten Laien erklären.".into()),
            output: "popup".into(),
            backend: "claude".into(),
        });
        modi.insert("fragen".into(), Modus {
            label: "❓ Frei fragen".into(),
            prompt: None,
            output: "popup".into(),
            backend: "claude".into(),
        });
        Self {
            modi,
            picker_hotkey: default_picker_hotkey(),
            pill_toggle_hotkey: default_pill_toggle_hotkey(),
            claude_timeout_sek: default_timeout(),
        }
    }
}
