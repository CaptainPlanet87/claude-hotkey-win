# claude-hotkey für Windows 11 — Setup-Anleitung

**Für Claude Code auf Windows:** Diese Anleitung schrittweise abarbeiten. Es ist eine Rust-App, die das gleiche Feature-Set wie die Linux-Variante hat (Pille + Halbkreis-Menü + Tastatur-Picker).

## Was wird gebaut

- **Hotkey-Daemon** läuft im Hintergrund, hört auf zwei Hotkeys:
  - **Strg+Shift+Y** → öffnet Tastatur-Picker (Listbox mit Tab-Navigation + Einstellungen-Eintrag)
  - **Strg+Shift+P** → toggelt die schwebende Pille (Maus-Bedienung)
- **Pille** = kleines „Claude"-Element unten zentral, immer im Vordergrund. Klick → öffnet Halbkreis-Menü.
- **Halbkreis-Menü** = 5 Modi (Übersetzen, Verbessern, Zusammenfassen, Erklären, Frei fragen) in Halbkreis um die Pille.
- **Backend:** Claude Code CLI (`claude --print`) — nutzt deine eingeloggte Subscription.

> **Hinweis:** Linux-Version hat „Strg+DoubleShift" als Pille-Toggle. Auf Windows braucht das einen LowLevelKeyboardHook (kommt in Phase 2). Vorerst normaler Hotkey **Strg+Shift+P**. Kannst du in `config.json` ändern.

## Selection-Verhalten (anders als Linux!)

Linux hat eine „Primary Selection" — markierter Text wird automatisch lesbar.
**Windows hat das nicht.** Daher: die App simuliert ein **Strg+C** im Moment des Hotkey-Drucks. Das heißt der Text muss markiert sein UND die Quell-App muss noch Fokus haben. In den meisten Apps klappt das, in Terminals mit eigenem Strg+C-Mapping nicht.

## Voraussetzungen

1. **Windows 11** (oder Windows 10)
2. **Rust-Toolchain**:
   ```powershell
   # Falls noch nicht installiert: rustup von https://rustup.rs holen
   # Oder via winget:
   winget install Rustlang.Rustup
   rustup install stable
   rustup default stable
   ```
3. **Claude Code CLI** muss im PATH sein und eingeloggt:
   ```powershell
   npm install -g @anthropic-ai/claude-code
   claude --version  # sollte was ausspucken
   ```
   Falls Login nötig: `claude` einmal interaktiv starten.
4. **Visual Studio Build Tools** (MSVC) — wird von rustup oft mit-installiert. Falls Cargo später meckert wegen Linker, manuell holen:
   `winget install Microsoft.VisualStudio.2022.BuildTools` (mit C++-Workload).

## Repo clonen

```powershell
cd $env:USERPROFILE
git clone https://github.com/CaptainPlanet87/claude-hotkey-win.git
cd claude-hotkey-win
```

## Build

```powershell
cargo build --release
```

Dauert beim ersten Mal **mehrere Minuten** (iced + wgpu + alle Deps). Folge-Builds sind schnell.

Das Binary liegt unter `target\release\claude-hotkey.exe`.

## Smoke-Test (vor Autostart)

1. **Config wird beim ersten Start automatisch angelegt** unter `%APPDATA%\claude-hotkey\config.json`. Du kannst sie vorher ansehen:
   ```powershell
   .\target\release\claude-hotkey.exe --list
   # Listet die 5 Default-Modi
   ```

2. **Modus direkt testen** (ohne Hotkey):
   ```powershell
   # Erst etwas in die Zwischenablage kopieren, dann:
   .\target\release\claude-hotkey.exe --mode uebersetzen
   ```
   Sollte Claude aufrufen → Übersetzung → Popup mit Antwort + Clipboard.

3. **Pille testen**:
   ```powershell
   .\target\release\claude-hotkey.exe --pill
   ```
   Sollte ein kleines Pillen-Window unten zentral zeigen. Mit Klick auf X (Taskleiste) beenden.

4. **Daemon testen** (Hotkeys aktiv):
   ```powershell
   .\target\release\claude-hotkey.exe
   ```
   Läuft im Vordergrund mit Log-Output. **Strg+Shift+Y** sollte Picker öffnen. Strg+C zum Beenden.

## Autostart (Anmeldung → Daemon läuft)

Variante A: **Startup-Ordner** (einfach)

1. `Win+R` → `shell:startup` öffnet den Startup-Ordner.
2. Dort eine Verknüpfung anlegen die `C:\Users\<User>\claude-hotkey-win\target\release\claude-hotkey.exe` startet.
3. In den Verknüpfungs-Eigenschaften: **„Ausführen: Minimiert"** wählen damit kein Terminal-Fenster aufpoppt.

Variante B: **Task Scheduler** (sauberer, kein Konsolen-Fenster)

```powershell
$action = New-ScheduledTaskAction -Execute "$env:USERPROFILE\claude-hotkey-win\target\release\claude-hotkey.exe"
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -DontStopOnIdleEnd
Register-ScheduledTask -TaskName "ClaudeHotkey" -Action $action -Trigger $trigger -Settings $settings -RunLevel Limited
```

Zum Deinstallieren:
```powershell
Unregister-ScheduledTask -TaskName "ClaudeHotkey" -Confirm:$false
```

## Bedienung

| Aktion | Tastenkombination |
|---|---|
| Text markieren + Strg+C drücken | (Vorbereitung) |
| Tastatur-Picker (Listbox) | **Strg+Shift+Y** |
| Pille toggeln | **Strg+Shift+P** |
| Klick auf Pille | öffnet Halbkreis-Menü |
| Im Picker/Listbox: Tab/Pfeile + Enter | navigieren + wählen |
| Esc | abbrechen |

## Config anpassen

`%APPDATA%\claude-hotkey\config.json` editieren. Format identisch zu Linux-Variante. Hotkeys umstellen z.B.:
```json
{
  "picker_hotkey": "Ctrl+Alt+C",
  "pill_toggle_hotkey": "Ctrl+Shift+P",
  "claude_timeout_sek": 60,
  "modi": { ... }
}
```

Nach Config-Änderung: **Daemon neu starten** (Task Scheduler-Eintrag rechtsklick → „Beenden", dann „Ausführen").

## Troubleshooting

**Daemon startet nicht / Build-Errors:**
- Sicherstellen dass MSVC oder mingw-w64 als Linker da ist (`rustc --print sysroot` → schauen ob lld dabei)
- `cargo clean && cargo build --release` versuchen

**Hotkey wird nicht erkannt:**
- Liegt vermutlich an Konflikt mit anderer App die den Hotkey schon registriert hat (z.B. Snipping Tool nimmt Win+Shift+S). Anderen Hotkey in config.json wählen, Daemon neu starten.

**Pille erscheint nicht / X-Symbol in Taskleiste:**
- Sollte unter Windows 11 normal aussehen. Falls Icon-Mismatch: in `target/release/` müsste sich ein `.exe` mit Standard-Icon zeigen. Custom-Icon ist Future-Feature.

**Claude wird nicht gefunden:**
- `claude --version` testen. Falls nicht im PATH: `npm install -g @anthropic-ai/claude-code`, dann neu einloggen mit `claude /login`.

**Selection (markierter Text) wird nicht gelesen:**
- Die App simuliert Strg+C. Wenn die Quell-App das nicht standard-mäßig macht (z.B. Terminal mit Custom-Binding), markiere und drück selbst Strg+C vor dem Hotkey.

## Was nicht (noch nicht) implementiert ist

- **DoubleShift-Hotkey** wie auf Linux (braucht LowLevelKeyboardHook, Phase 2)
- **System-Tray-Icon** (man sieht den Daemon nur im Task Manager)
- **Multi-Backend** (nur Claude, OpenAI/Gemini Hook ist im Backend-Trait vorbereitet)
- **Custom App-Icon** (nutzt Default exe-Icon)

## Bei Problemen / Wünschen

Issues im Repo aufmachen oder Steve sagen, was nicht klappt — der Linux-Code (`claude-hotkey-rs`) ist die Referenz, da kommt jede Feature-Variante zuerst.
