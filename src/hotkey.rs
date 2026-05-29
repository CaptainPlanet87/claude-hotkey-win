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
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};
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

// ===== Geste: Strg HALTEN + Shift 2x tippen -> Picker =====
// RegisterHotKey kann keine reinen Modifier-Gesten. Daher ein
// WH_KEYBOARD_LL-Hook, der Shift-Down-Flanken zaehlt, SOLANGE Strg
// durchgehend gehalten wird. Zwei Flanken innerhalb SHIFT_DOUBLE_WINDOW_MS
// -> GESTURE_FIRED (wird im Event-Loop gepollt). Strg loslassen oder eine
// andere Taste druecken setzt den Zaehler zurueck. Auto-Repeat von Shift
// (gehaltene Taste) wird ignoriert — nur echte Down-Flanken zaehlen.
static GESTURE_FIRED: AtomicBool = AtomicBool::new(false);
static CTRL_DOWN: AtomicBool = AtomicBool::new(false);
static SHIFT_DOWN: AtomicBool = AtomicBool::new(false);
static SHIFT_TAPS: AtomicI64 = AtomicI64::new(0);
static LAST_SHIFT_MS: AtomicI64 = AtomicI64::new(0);
const SHIFT_DOUBLE_WINDOW_MS: i64 = 600;

// ===== Geste 2: Strg + Rechtsklick -> Picker am Mauszeiger =====
// Reines globales Rechtsklick-Abfangen wuerde normale Kontextmenues ueberall
// kaputtmachen -> daher NUR bei gehaltenem Strg. Flag + Cursor-Position werden
// im Event-Loop gepollt; SUPPRESS_RUP schluckt das zum Down gehoerige Up.
static RCLICK_FIRED: AtomicBool = AtomicBool::new(false);
static RCLICK_X: AtomicI32 = AtomicI32::new(0);
static RCLICK_Y: AtomicI32 = AtomicI32::new(0);
static SUPPRESS_RUP: AtomicBool = AtomicBool::new(false);

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(windows)]
unsafe extern "system" fn ll_keyboard_proc(
    code: i32,
    wparam: winapi::shared::minwindef::WPARAM,
    lparam: winapi::shared::minwindef::LPARAM,
) -> winapi::shared::minwindef::LRESULT {
    use winapi::um::winuser::{
        CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, VK_CONTROL, VK_LCONTROL, VK_LSHIFT, VK_RCONTROL,
        VK_RSHIFT, VK_SHIFT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };
    if code == HC_ACTION {
        let kb = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        let vk = kb.vkCode as i32;
        let down = wparam == WM_KEYDOWN as usize || wparam == WM_SYSKEYDOWN as usize;
        let up = wparam == WM_KEYUP as usize || wparam == WM_SYSKEYUP as usize;
        let is_ctrl = vk == VK_CONTROL || vk == VK_LCONTROL || vk == VK_RCONTROL;
        let is_shift = vk == VK_SHIFT || vk == VK_LSHIFT || vk == VK_RSHIFT;

        if down {
            if is_ctrl {
                CTRL_DOWN.store(true, Ordering::SeqCst);
            } else if is_shift {
                // Nur die Down-FLANKE zaehlen (Auto-Repeat ignorieren).
                let was_down = SHIFT_DOWN.swap(true, Ordering::SeqCst);
                if !was_down && CTRL_DOWN.load(Ordering::SeqCst) {
                    let now = now_ms();
                    let last = LAST_SHIFT_MS.swap(now, Ordering::SeqCst);
                    if SHIFT_TAPS.load(Ordering::SeqCst) >= 1
                        && last != 0
                        && now - last <= SHIFT_DOUBLE_WINDOW_MS
                    {
                        // Zweiter Shift-Tipp im Zeitfenster -> Geste feuert.
                        GESTURE_FIRED.store(true, Ordering::SeqCst);
                        SHIFT_TAPS.store(0, Ordering::SeqCst);
                        LAST_SHIFT_MS.store(0, Ordering::SeqCst);
                    } else {
                        // Erster Tipp (oder zu langsam) -> als ersten werten.
                        SHIFT_TAPS.store(1, Ordering::SeqCst);
                    }
                }
            } else {
                // Andere Taste bei gehaltenem Strg -> Geste abbrechen.
                SHIFT_TAPS.store(0, Ordering::SeqCst);
                LAST_SHIFT_MS.store(0, Ordering::SeqCst);
            }
        } else if up {
            if is_ctrl {
                CTRL_DOWN.store(false, Ordering::SeqCst);
                // Strg muss durchgehend gehalten werden -> Reset beim Loslassen.
                SHIFT_TAPS.store(0, Ordering::SeqCst);
                LAST_SHIFT_MS.store(0, Ordering::SeqCst);
            }
            if is_shift {
                SHIFT_DOWN.store(false, Ordering::SeqCst);
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

// LL-Maus-Hook: Rechtsklick bei gehaltenem Strg (CTRL_DOWN s.o.) -> Picker am
// Mauszeiger. Das Rechtsklick-Event wird NUR in diesem Fall geschluckt, damit
// nicht zusaetzlich das Kontextmenue der App aufgeht. Ohne Strg: nichts.
#[cfg(windows)]
unsafe extern "system" fn ll_mouse_proc(
    code: i32,
    wparam: winapi::shared::minwindef::WPARAM,
    lparam: winapi::shared::minwindef::LPARAM,
) -> winapi::shared::minwindef::LRESULT {
    use winapi::um::winuser::{
        CallNextHookEx, HC_ACTION, MSLLHOOKSTRUCT, WM_RBUTTONDOWN, WM_RBUTTONUP,
    };
    if code == HC_ACTION {
        let msg = wparam as u32;
        if msg == WM_RBUTTONDOWN && CTRL_DOWN.load(Ordering::SeqCst) {
            let ms = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
            RCLICK_X.store(ms.pt.x, Ordering::SeqCst);
            RCLICK_Y.store(ms.pt.y, Ordering::SeqCst);
            RCLICK_FIRED.store(true, Ordering::SeqCst);
            SUPPRESS_RUP.store(true, Ordering::SeqCst);
            return 1; // Down schlucken -> kein App-Kontextmenue
        }
        if msg == WM_RBUTTONUP && SUPPRESS_RUP.swap(false, Ordering::SeqCst) {
            return 1; // zugehoeriges Up ebenfalls schlucken
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

#[cfg(windows)]
fn install_ll_hook() {
    // Eigener Thread mit eigener GetMessage-Schleife: LL-Hooks muessen auf dem
    // installierenden Thread *prompt* bedient werden (Windows LowLevelHooksTimeout).
    // Der tao-Thread schlaeft 50ms pro Runde und ist dafuer ungeeignet.
    // Beide Hooks (Tastatur + Maus) laufen auf DIESEM Thread / einer Pumpe.
    std::thread::spawn(|| {
        use winapi::um::libloaderapi::GetModuleHandleW;
        use winapi::um::winuser::{
            GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, MSG, WH_KEYBOARD_LL, WH_MOUSE_LL,
        };
        unsafe {
            let hmod = GetModuleHandleW(std::ptr::null());
            let kb = SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), hmod, 0);
            if kb.is_null() {
                log::warn!("LL-Keyboard-Hook nicht installiert — Geste (Strg halten + Shift 2x) inaktiv");
            } else {
                log::info!("LL-Keyboard-Hook aktiv: Strg halten + Shift 2x -> Picker");
            }
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(ll_mouse_proc), hmod, 0);
            if mouse.is_null() {
                log::warn!("LL-Maus-Hook nicht installiert — Strg+Rechtsklick inaktiv");
            } else {
                log::info!("LL-Maus-Hook aktiv: Strg+Rechtsklick -> Picker am Cursor");
            }
            let mut msg: MSG = std::mem::zeroed();
            // GetMessageW haelt den Thread am Pumpen -> Hooks werden prompt bedient.
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
            if !kb.is_null() {
                UnhookWindowsHookEx(kb);
            }
            if !mouse.is_null() {
                UnhookWindowsHookEx(mouse);
            }
        }
    });
}

/// Registriert einen Hotkey best-effort und gibt dessen id zurueck (oder None).
fn register_hotkey(manager: &GlobalHotKeyManager, spec: &str, name: &str) -> Option<u32> {
    match parse_hotkey(spec) {
        Ok(hk) => match manager.register(hk) {
            Ok(_) => {
                log::info!("{name}-Hotkey '{spec}' registriert");
                Some(hk.id())
            }
            Err(e) => {
                log::warn!("{name}-Hotkey '{spec}' nicht registrierbar: {e}");
                None
            }
        },
        Err(e) => {
            log::warn!("{name}-Hotkey '{spec}' ungueltig: {e}");
            None
        }
    }
}

pub fn run_daemon(cfg: Config) -> Result<()> {
    let manager = GlobalHotKeyManager::new()?;

    // Hotkeys best-effort registrieren (Konflikt/Fehler killt den Daemon nicht;
    // die Geste "Strg halten + Shift 2x" funktioniert unabhaengig davon).
    let picker_id = register_hotkey(&manager, &cfg.picker_hotkey, "picker");
    let pill_id = register_hotkey(&manager, &cfg.pill_toggle_hotkey, "pill");

    // Low-Level-Hooks: Tastatur-Geste (Strg halten + Shift 2x) und
    // Maus-Geste (Strg + Rechtsklick) -> Picker.
    #[cfg(windows)]
    install_ll_hook();

    let pill_child: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));

    let event_loop = EventLoopBuilder::new().build();
    let receiver = GlobalHotKeyEvent::receiver();

    event_loop.run(move |_event: Event<()>, _, control_flow| {
        // Poll statt Wait: global-hotkey liefert Events über einen eigenen
        // Hintergrund-Thread in einen Channel. Mit Wait würde tao mangels
        // Fenster nach dem ersten Durchlauf dauerhaft in GetMessage blockieren
        // und den Channel nie wieder leeren -> Hotkeys reagieren nicht.
        *control_flow = ControlFlow::Poll;

        // Geste "Strg halten + Shift 2x" (Low-Level-Hook)?
        if GESTURE_FIRED.swap(false, Ordering::SeqCst) {
            log::info!("[gesture] Strg halten + Shift 2x -> Picker");
            spawn_listpicker();
        }

        // Strg+Rechtsklick (Low-Level-Maus-Hook) -> Picker am Mauszeiger?
        if RCLICK_FIRED.swap(false, Ordering::SeqCst) {
            let x = RCLICK_X.load(Ordering::SeqCst);
            let y = RCLICK_Y.load(Ordering::SeqCst);
            log::info!("[rclick] Strg+Rechtsklick @ {x},{y} -> Picker");
            spawn_listpicker_at(Some((x, y)));
        }

        // Hotkey-Channel hat globale Events — non-blocking polling
        while let Ok(ev) = receiver.try_recv() {
            if ev.state() == global_hotkey::HotKeyState::Pressed {
                let action = if Some(ev.id()) == picker_id {
                    Some(Action::OpenPicker)
                } else if Some(ev.id()) == pill_id {
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
    spawn_listpicker_at(None);
}

fn spawn_listpicker_at(pos: Option<(i32, i32)>) {
    let mut cmd = Command::new(self_exe());
    cmd.arg("--listpicker");
    if let Some((x, y)) = pos {
        // Cursor-Position fuer das "Kontextmenue am Mauszeiger".
        cmd.args(["--at", &x.to_string(), &y.to_string()]);
    }
    match cmd.spawn() {
        Ok(_) => log::info!("listpicker gestartet"),
        Err(e) => log::error!("listpicker spawn: {e}"),
    }
}

fn toggle_pill(pill_child: &Arc<Mutex<Option<Child>>>) {
    // Schnell & ohne wmic: den bereits getrackten Pillen-Child per try_wait
    // pruefen. (wmic ist auf Win11 deprecated und braucht mehrere Sekunden pro
    // Aufruf, was den Pillen-Toggle spuerbar verzoegert hat.)
    let mut guard = pill_child.lock().unwrap();
    let pill_alive = matches!(guard.as_mut().map(|c| c.try_wait()), Some(Ok(None)));
    log::info!("[toggle_pill] pill_alive={pill_alive}");

    if pill_alive {
        if let Some(mut c) = guard.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        // evtl. offenes Halbkreis-Menue (Klick-Spawn der Pille) mitschliessen
        kill_all_by_args("--picker");
        return;
    }

    // tote/abgestuerzte Referenz verwerfen, dann frisch starten
    *guard = None;
    match Command::new(self_exe()).arg("--pill").spawn() {
        Ok(child) => {
            log::info!("Pille gestartet PID {}", child.id());
            *guard = Some(child);
        }
        Err(e) => log::error!("Pille spawn: {e}"),
    }
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
