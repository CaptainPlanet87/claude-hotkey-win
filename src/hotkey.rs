//! Hotkey-Daemon mit tao-EventLoop + global-hotkey-Crate.
//!
//! Registriert zwei Hotkeys aus config.json:
//!  - `picker_hotkey` → öffnet den Tastatur-Picker (`--listpicker`)
//!  - `pill_toggle_hotkey` → toggelt die Pille (`--pill`)
//!
//! Beide Aktionen spawnen das eigene Binary als Subprozess.

use crate::config::Config;
use anyhow::{Result, bail};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, hotkey::{Code, HotKey, Modifiers}};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};

fn self_exe() -> std::path::PathBuf {
    std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("claude-hotkey.exe"))
}

/// Parse "Ctrl+Shift+Y" → HotKey
fn parse_hotkey(hk: &str) -> Result<HotKey> {
    let mut mods = Modifiers::empty();
    let mut code: Option<Code> = None;
    for part in hk.split('+').map(|p| p.trim()) {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "shift" => mods |= Modifiers::SHIFT,
            "alt" => mods |= Modifiers::ALT,
            "super" | "meta" | "win" => mods |= Modifiers::META,
            other => {
                code = Some(name_to_code(other)?);
            }
        }
    }
    let code = code.ok_or_else(|| anyhow::anyhow!("Hotkey '{hk}' braucht eine Trigger-Taste"))?;
    Ok(HotKey::new(Some(mods), code))
}

fn name_to_code(name: &str) -> Result<Code> {
    if name.len() == 1 {
        let c = name.chars().next().unwrap().to_ascii_uppercase();
        match c {
            'A' => return Ok(Code::KeyA),
            'B' => return Ok(Code::KeyB),
            'C' => return Ok(Code::KeyC),
            'D' => return Ok(Code::KeyD),
            'E' => return Ok(Code::KeyE),
            'F' => return Ok(Code::KeyF),
            'G' => return Ok(Code::KeyG),
            'H' => return Ok(Code::KeyH),
            'I' => return Ok(Code::KeyI),
            'J' => return Ok(Code::KeyJ),
            'K' => return Ok(Code::KeyK),
            'L' => return Ok(Code::KeyL),
            'M' => return Ok(Code::KeyM),
            'N' => return Ok(Code::KeyN),
            'O' => return Ok(Code::KeyO),
            'P' => return Ok(Code::KeyP),
            'Q' => return Ok(Code::KeyQ),
            'R' => return Ok(Code::KeyR),
            'S' => return Ok(Code::KeyS),
            'T' => return Ok(Code::KeyT),
            'U' => return Ok(Code::KeyU),
            'V' => return Ok(Code::KeyV),
            'W' => return Ok(Code::KeyW),
            'X' => return Ok(Code::KeyX),
            'Y' => return Ok(Code::KeyY),
            'Z' => return Ok(Code::KeyZ),
            '0' => return Ok(Code::Digit0),
            '1' => return Ok(Code::Digit1),
            '2' => return Ok(Code::Digit2),
            '3' => return Ok(Code::Digit3),
            '4' => return Ok(Code::Digit4),
            '5' => return Ok(Code::Digit5),
            '6' => return Ok(Code::Digit6),
            '7' => return Ok(Code::Digit7),
            '8' => return Ok(Code::Digit8),
            '9' => return Ok(Code::Digit9),
            _ => {}
        }
    }
    let lower = name.to_lowercase();
    match lower.as_str() {
        "space" => Ok(Code::Space),
        "enter" | "return" => Ok(Code::Enter),
        "tab" => Ok(Code::Tab),
        "escape" | "esc" => Ok(Code::Escape),
        "backspace" => Ok(Code::Backspace),
        "delete" => Ok(Code::Delete),
        "f1" => Ok(Code::F1),
        "f2" => Ok(Code::F2),
        "f3" => Ok(Code::F3),
        "f4" => Ok(Code::F4),
        "f5" => Ok(Code::F5),
        "f6" => Ok(Code::F6),
        "f7" => Ok(Code::F7),
        "f8" => Ok(Code::F8),
        "f9" => Ok(Code::F9),
        "f10" => Ok(Code::F10),
        "f11" => Ok(Code::F11),
        "f12" => Ok(Code::F12),
        _ => bail!("Taste '{name}' nicht erkannt (oder DoubleShift — wird auf Windows noch nicht supported)"),
    }
}

#[derive(Debug, Clone, Copy)]
enum Action {
    OpenPicker,
    TogglePill,
}

pub fn run_daemon(cfg: Config) -> Result<()> {
    let manager = GlobalHotKeyManager::new()?;
    let picker = parse_hotkey(&cfg.picker_hotkey)?;
    let pill = parse_hotkey(&cfg.pill_toggle_hotkey)?;
    manager.register(picker)?;
    manager.register(pill)?;
    log::info!(
        "Hotkeys registriert: picker='{}' pill='{}'",
        cfg.picker_hotkey, cfg.pill_toggle_hotkey
    );

    let picker_id = picker.id();
    let pill_id = pill.id();

    let pill_child: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));

    let event_loop = EventLoopBuilder::new().build();
    let receiver = GlobalHotKeyEvent::receiver();

    event_loop.run(move |_event: Event<()>, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        // Hotkey-Channel hat globale Events — non-blocking polling
        while let Ok(ev) = receiver.try_recv() {
            if ev.state() == global_hotkey::HotKeyState::Pressed {
                let action = if ev.id() == picker_id {
                    Some(Action::OpenPicker)
                } else if ev.id() == pill_id {
                    Some(Action::TogglePill)
                } else {
                    None
                };
                if let Some(a) = action {
                    log::info!("[hotkey] → {a:?}");
                    match a {
                        Action::OpenPicker => spawn_listpicker(),
                        Action::TogglePill => toggle_pill(&pill_child),
                    }
                }
            }
        }
        // Tao timer für nächstes Polling
        std::thread::sleep(std::time::Duration::from_millis(50));
    });
}

fn spawn_listpicker() {
    match Command::new(self_exe()).arg("--listpicker").spawn() {
        Ok(_) => log::info!("listpicker gestartet"),
        Err(e) => log::error!("listpicker spawn: {e}"),
    }
}

fn toggle_pill(pill_child: &Arc<Mutex<Option<Child>>>) {
    // Auch alle stray pill/picker prozesse berücksichtigen (via pgrep-Äquivalent: tasklist)
    let pill_running = is_running("--pill");
    let picker_running = is_running("--picker");
    log::info!("[toggle_pill] pill={pill_running} picker={picker_running}");

    if pill_running || picker_running {
        kill_all_by_args("--pill");
        kill_all_by_args("--picker");
        if let Some(mut c) = pill_child.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        return;
    }

    match Command::new(self_exe()).arg("--pill").spawn() {
        Ok(child) => {
            log::info!("Pille gestartet PID {}", child.id());
            *pill_child.lock().unwrap() = Some(child);
        }
        Err(e) => log::error!("Pille spawn: {e}"),
    }
}

/// Sucht claude-hotkey.exe-Prozesse mit dem gegebenen Argument
fn is_running(arg: &str) -> bool {
    // tasklist /V zeigt nur Window-Titles, nicht command-line.
    // Wir nutzen wmic process get commandline.
    let out = Command::new("wmic")
        .args(["process", "where", "name='claude-hotkey.exe'", "get", "commandline"])
        .output();
    if let Ok(o) = out {
        let s = String::from_utf8_lossy(&o.stdout);
        return s.contains(arg);
    }
    false
}

fn kill_all_by_args(arg: &str) {
    // PowerShell: Get-Process | Where Args | Stop-Process
    let script = format!(
        "Get-CimInstance Win32_Process | Where-Object {{ $_.Name -eq 'claude-hotkey.exe' -and $_.CommandLine -like '*{}*' }} | ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force }}",
        arg.replace('\'', "''")
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output();
}
