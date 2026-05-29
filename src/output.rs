//! Output: Clipboard (arboard), Notify (notify-rust = Toast unter Win11),
//! Selection-Grabbing (SendInput Strg+C → kurz warten → Clipboard).

use anyhow::Result;
use std::time::Duration;

/// "Selection" auf Windows = wir simulieren Strg+C und lesen Clipboard.
/// Voraussetzung: User hat den Text wirklich markiert + die Quell-App
/// hat noch Fokus.
pub fn get_selection_via_clipboard() -> Option<String> {
    let prev = read_clipboard().unwrap_or_default();
    send_ctrl_c();
    // Kurz warten damit die Quell-App das Strg+C verarbeitet
    std::thread::sleep(Duration::from_millis(120));
    let after = read_clipboard().unwrap_or_default();
    if after.is_empty() {
        return None;
    }
    // Wenn sich nichts geändert hat, könnte der User wirklich diesen Text
    // markiert haben (Clipboard schon == Selection) oder nichts markiert
    // — wir geben trotzdem zurück; backend prüft auf leer.
    if after == prev && prev.is_empty() {
        return None;
    }
    Some(after)
}

pub fn read_clipboard() -> Option<String> {
    arboard::Clipboard::new().ok().and_then(|mut c| c.get_text().ok())
}

pub fn set_clipboard(text: &str) -> Result<()> {
    let mut c = arboard::Clipboard::new()
        .map_err(|e| anyhow::anyhow!("Clipboard: {e}"))?;
    c.set_text(text.to_string())
        .map_err(|e| anyhow::anyhow!("Clipboard write: {e}"))?;
    Ok(())
}

pub fn notify(summary: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(summary)
        .body(&body.chars().take(300).collect::<String>())
        .show();
}

/// Antwort dem User zugänglich machen.
/// `mode`: "popup" oder "clipboard".
pub fn deliver(antwort: &str, mode: &str, label: &str) -> Result<()> {
    set_clipboard(antwort).ok();

    match mode {
        "clipboard" => {
            notify("Claude", "✓ In Zwischenablage");
        }
        _ => {
            notify("Claude", "📋 Antwort in der Zwischenablage");
            show_popup(antwort, label)?;
        }
    }
    Ok(())
}

/// Ergebnis-Anzeige: startet das bearbeitbare iced-Fenster (`--result`,
/// siehe result.rs) als eigenen Prozess. Loest die alte, nicht editierbare
/// PowerShell-MessageBox ab.
fn show_popup(text: &str, label: &str) -> Result<()> {
    // Bearbeitbares Ergebnis-Fenster als EIGENER Prozess starten. Text geht
    // ueber stdin (kein Arg-Laengen-/Escaping-Limit). Das Fenster ueberlebt den
    // beendenden Worker, weil es ein eigenstaendiger Prozess ist.
    use std::io::Write;
    use std::process::{Command, Stdio};
    let exe = std::env::current_exe()
        .unwrap_or_else(|_| std::path::PathBuf::from("claude-hotkey.exe"));
    match Command::new(exe)
        .args(["--result", label])
        .stdin(Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
                // stdin droppt hier -> Pipe-EOF -> Fenster kann zu Ende lesen.
            }
        }
        Err(e) => {
            log::error!("result-window spawn: {e}");
            notify(label, text); // Fallback: wenigstens als Toast zeigen.
        }
    }
    Ok(())
}

/// User um Texteingabe bitten (für "Frei fragen"-Modus).
/// Pragmatisch via PowerShell InputBox.
pub fn ask_user(titel: &str, prompt: &str) -> Result<String> {
    let script = format!(
        "Add-Type -AssemblyName Microsoft.VisualBasic; \
         [Microsoft.VisualBasic.Interaction]::InputBox('{}','{}','')",
        prompt.replace('\'', "''"),
        titel.replace('\'', "''")
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(s)
}

// === SendInput Strg+C ===

#[cfg(windows)]
fn send_ctrl_c() {
    use winapi::um::winuser::{
        INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CONTROL, VK_MENU,
        VK_SHIFT,
    };
    use std::mem::{size_of, zeroed};

    unsafe {
        // Zuerst evtl. gehaltene Modifier lösen: Wird der Picker per
        // Strg+Shift-Geste geöffnet, hält der User evtl. noch Shift — dann
        // würde aus unserem Strg+C ein Strg+Shift+C (öffnet Konsole/DevTools,
        // kopiert NICHT). Alt analog.
        let mut rel: [INPUT; 2] = [zeroed(); 2];
        rel[0].type_ = INPUT_KEYBOARD;
        *rel[0].u.ki_mut() = KEYBDINPUT {
            wVk: VK_SHIFT as u16,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };
        rel[1].type_ = INPUT_KEYBOARD;
        *rel[1].u.ki_mut() = KEYBDINPUT {
            wVk: VK_MENU as u16,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };
        SendInput(rel.len() as u32, rel.as_mut_ptr(), size_of::<INPUT>() as i32);
        std::thread::sleep(Duration::from_millis(25));

        let mut inputs: [INPUT; 4] = [zeroed(); 4];
        // Ctrl down
        inputs[0].type_ = INPUT_KEYBOARD;
        *inputs[0].u.ki_mut() = KEYBDINPUT {
            wVk: VK_CONTROL as u16,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: 0,
        };
        // C down
        inputs[1].type_ = INPUT_KEYBOARD;
        *inputs[1].u.ki_mut() = KEYBDINPUT {
            wVk: 0x43, // 'C'
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: 0,
        };
        // C up
        inputs[2].type_ = INPUT_KEYBOARD;
        *inputs[2].u.ki_mut() = KEYBDINPUT {
            wVk: 0x43,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };
        // Ctrl up
        inputs[3].type_ = INPUT_KEYBOARD;
        *inputs[3].u.ki_mut() = KEYBDINPUT {
            wVk: VK_CONTROL as u16,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };
        SendInput(
            inputs.len() as u32,
            inputs.as_mut_ptr(),
            size_of::<INPUT>() as i32,
        );
    }
}

#[cfg(not(windows))]
fn send_ctrl_c() {
    // No-op auf Nicht-Windows (für lokales Compilier-Test auf Linux)
}
