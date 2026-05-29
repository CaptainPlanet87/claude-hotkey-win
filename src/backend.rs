//! Backend-Abstraktion. Aktuell: Claude Code CLI (`claude --print`).
//! Auf Windows: claude.exe muss im PATH liegen (npm install -g @anthropic-ai/claude-code).

use crate::config::Config;
use crate::output;
use anyhow::{Context, Result, bail};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Findet das claude-Binary robust: bevorzugt den nativen Installer-Pfad
/// (~/.local/bin/claude.exe), sonst PATH. Wichtig, weil der per Task-Scheduler
/// gestartete Daemon ~/.local/bin evtl. nicht im PATH hat.
fn claude_program() -> std::ffi::OsString {
    if let Some(home) = dirs::home_dir() {
        let p = home.join(".local").join("bin").join("claude.exe");
        if p.exists() {
            return p.into_os_string();
        }
    }
    std::ffi::OsString::from("claude")
}

pub trait Backend {
    fn name(&self) -> &'static str;
    fn query(&self, prompt: &str, text: &str, timeout: Duration) -> Result<String>;
}

pub struct ClaudeBackend;

impl Backend for ClaudeBackend {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn query(&self, prompt: &str, text: &str, timeout: Duration) -> Result<String> {
        let full_prompt = if text.is_empty() {
            prompt.to_string()
        } else {
            format!("{prompt}\n\n---\n\n{text}")
        };

        let mut cmd = Command::new(claude_program());
        cmd.arg("--print")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // claude.exe braucht ~/.local/bin im PATH, sonst exit 129, wenn der per
        // Task-Scheduler gestartete Daemon das Verzeichnis nicht geerbt hat.
        if let Some(home) = dirs::home_dir() {
            let bin = home.join(".local").join("bin");
            let cur = std::env::var_os("PATH").unwrap_or_default();
            let mut search: Vec<std::path::PathBuf> = vec![bin];
            search.extend(std::env::split_paths(&cur));
            if let Ok(joined) = std::env::join_paths(search) {
                cmd.env("PATH", joined);
            }
        }
        let mut child = cmd
            .spawn()
            .context("claude --print starten (claude.exe im PATH?)")?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(full_prompt.as_bytes())
                .context("Prompt an claude schreiben")?;
        }

        // Timeout-Wachhund via Thread
        use std::sync::mpsc;
        use std::thread;
        let (tx, rx) = mpsc::channel::<()>();
        let pid = child.id();
        thread::spawn(move || {
            if rx.recv_timeout(timeout).is_err() {
                kill_process(pid);
            }
        });

        let output = child.wait_with_output().context("Auf claude warten")?;
        let _ = tx.send(());

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "claude exit {:?}: {}",
                output.status.code(),
                stderr.chars().take(300).collect::<String>()
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

#[cfg(windows)]
fn kill_process(pid: u32) {
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{OpenProcess, TerminateProcess};
    use winapi::um::winnt::PROCESS_TERMINATE;
    unsafe {
        let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if !h.is_null() {
            TerminateProcess(h, 1);
            CloseHandle(h);
        }
    }
}

#[cfg(not(windows))]
fn kill_process(_pid: u32) {}

pub fn get_backend(name: &str) -> Option<Box<dyn Backend>> {
    match name {
        "claude" => Some(Box::new(ClaudeBackend)),
        _ => None,
    }
}

pub fn run_mode(cfg: &Config, mode_id: &str, text: Option<String>) -> Result<()> {
    let modus = cfg
        .modi
        .get(mode_id)
        .ok_or_else(|| anyhow::anyhow!("Modus '{mode_id}' nicht in config.json"))?;

    let text = match text {
        Some(t) if !t.trim().is_empty() => t,
        _ => output::get_selection_via_clipboard().unwrap_or_default(),
    };
    if text.trim().is_empty() {
        output::notify("Claude Hotkey", "Kein Text markiert (Strg+C drücken vor Hotkey).");
        return Ok(());
    }

    let prompt = match &modus.prompt {
        Some(p) => p.clone(),
        None => {
            let frage = output::ask_user(&modus.label, "Was möchtest du wissen?")?;
            if frage.is_empty() {
                return Ok(());
            }
            format!("{frage}\n\nBezug zum folgenden markierten Text:")
        }
    };

    let backend = get_backend(&modus.backend).ok_or_else(|| {
        anyhow::anyhow!(
            "Backend '{}' (für Modus '{}') ist nicht registriert.",
            modus.backend,
            mode_id
        )
    })?;

    output::notify("Claude", &format!("{} läuft …", modus.label));

    let timeout = Duration::from_secs(cfg.claude_timeout_sek);
    let antwort = backend.query(&prompt, &text, timeout)?;

    output::deliver(&antwort, &modus.output, &modus.label)?;
    Ok(())
}
