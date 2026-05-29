//! Claude Hotkey — Windows 11 Port.
//!
//! Architektur:
//!   - ohne Args     → Hotkey-Daemon (tao + global-hotkey)
//!   - `--pill`      → Pille (floating top-most window)
//!   - `--picker`    → Halbkreis-Menu (Maus, gespawnt von Pille-Click)
//!   - `--listpicker`→ Tastatur-Listbox (gespawnt von picker_hotkey, Ersatz für fuzzel)
//!   - `--mode <id>` → Backend direkt ausführen (CLI/Debug)
//!   - `--settings`  → config.json öffnen
//!   - `--list`      → Modi auflisten

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod backend;
mod config;
mod hotkey;
mod listpicker;
mod output;
mod picker_ring;
mod pill;
mod result;

use anyhow::Result;
use config::{Config, config_path};

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--pill") {
        return pill::run();
    }
    if args.iter().any(|a| a == "--picker") {
        return picker_ring::run();
    }
    if args.iter().any(|a| a == "--listpicker") {
        return listpicker::run();
    }
    if args.iter().any(|a| a == "--result") {
        return result::run();
    }
    if args.iter().any(|a| a == "--list") {
        return list_modes();
    }
    if args.iter().any(|a| a == "--settings") {
        return open_settings();
    }
    if let Some(idx) = args.iter().position(|a| a == "--mode") {
        let mode = args
            .get(idx + 1)
            .ok_or_else(|| anyhow::anyhow!("--mode braucht eine Modus-ID"))?;
        let from_stdin = args.iter().any(|a| a == "--text-from-stdin");
        return run_mode_cli(mode, from_stdin);
    }

    // Default: Daemon
    let cfg = Config::load(&config_path())?;
    hotkey::run_daemon(cfg)
}

fn list_modes() -> Result<()> {
    let cfg = Config::load(&config_path())?;
    for (id, m) in &cfg.modi {
        println!("  {:20} {}", id, m.label);
    }
    Ok(())
}

fn run_mode_cli(mode_id: &str, text_from_stdin: bool) -> Result<()> {
    let cfg = Config::load(&config_path())?;
    let text = if text_from_stdin {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Some(buf)
    } else {
        None
    };
    backend::run_mode(&cfg, mode_id, text)
}

fn open_settings() -> Result<()> {
    let path = config_path();
    let editor = std::env::var("VISUAL")
        .ok()
        .or_else(|| std::env::var("EDITOR").ok())
        .unwrap_or_else(|| "notepad".to_string());
    std::process::Command::new(editor).arg(&path).spawn()?;
    Ok(())
}
