//! Ergebnis-Fenster: kleines, BEARBEITBARES Textfenster fuer die Antwort.
//! Ersetzt die haessliche, nicht editierbare PowerShell-MessageBox.
//!
//! Wird vom Worker (`--mode`) ueber `output::show_popup` als separater Prozess
//! gestartet: `claude-hotkey --result "<label>"`, der Antworttext kommt ueber
//! stdin (kein Arg-Laengen-/Escaping-Limit). Eigener Prozess = konsistent mit
//! pill/picker/listpicker und ueberlebt den beendenden Worker.
//!
//! Features: editierbar, Strg+Z (Undo, eigener Stack), "Kopieren", Esc/Schliessen.

use crate::output;
use iced::keyboard::{self, Key, key::Named};
use iced::widget::{button, column, container, row, text, text_editor};
use iced::window::{Level, Position};
use iced::{Element, Event, Length, Size, Subscription, Task, Theme};
use std::io::Read;
use std::sync::OnceLock;

const WINDOW_W: u32 = 580;
const WINDOW_H: u32 = 440;
const UNDO_MAX: usize = 300;

static INITIAL_TEXT: OnceLock<String> = OnceLock::new();
static LABEL: OnceLock<String> = OnceLock::new();

struct State {
    content: text_editor::Content,
    label: String,
    copied: bool,
    undo: Vec<String>,
}

impl Default for State {
    fn default() -> Self {
        let txt = INITIAL_TEXT.get().cloned().unwrap_or_default();
        Self {
            content: text_editor::Content::with_text(&txt),
            label: LABEL.get().cloned().unwrap_or_else(|| "Claude".to_string()),
            copied: false,
            undo: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    Undo,
    Copy,
    Close,
    Event(Event),
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::Edit(action) => {
            if matches!(&action, text_editor::Action::Edit(_)) {
                state.copied = false;
                // Snapshot des Textes VOR der Aenderung fuer Undo.
                let cur = state.content.text();
                if state.undo.last().map(|s| s != &cur).unwrap_or(true) {
                    state.undo.push(cur);
                    if state.undo.len() > UNDO_MAX {
                        state.undo.remove(0);
                    }
                }
            }
            state.content.perform(action);
        }
        Message::Undo => {
            if let Some(prev) = state.undo.pop() {
                state.content = text_editor::Content::with_text(&prev);
                state.copied = false;
            }
        }
        Message::Copy => {
            let _ = output::set_clipboard(&state.content.text());
            state.copied = true;
        }
        Message::Close => std::process::exit(0),
        Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::Escape),
            ..
        })) => {
            std::process::exit(0);
        }
        _ => {}
    }
    Task::none()
}

fn view(state: &State) -> Element<'_, Message> {
    let header = text(state.label.clone()).size(15);

    let editor = text_editor(&state.content)
        .on_action(Message::Edit)
        .key_binding(|kp| {
            // Strg+Z (ohne Shift) -> Undo. Alles andere: Standardbelegung.
            let is_z =
                matches!(kp.key.as_ref(), Key::Character(c) if c.eq_ignore_ascii_case("z"));
            if kp.modifiers.command() && !kp.modifiers.shift() && is_z {
                Some(text_editor::Binding::Custom(Message::Undo))
            } else {
                text_editor::Binding::from_key_press(kp)
            }
        })
        .height(Length::Fill)
        .padding(10);

    let undo_btn = button(text("↶ Rückgängig (Strg+Z)").size(13))
        .padding([6.0, 16.0])
        .on_press(Message::Undo);
    let copy_label = if state.copied { "✓ Kopiert" } else { "Kopieren" };
    let copy_btn = button(text(copy_label).size(13))
        .padding([6.0, 16.0])
        .on_press(Message::Copy);
    let close_btn = button(text("Schließen (Esc)").size(13))
        .padding([6.0, 16.0])
        .on_press(Message::Close);

    let buttons = row![undo_btn, copy_btn, close_btn].spacing(8);

    container(column![header, editor, buttons].spacing(10).padding(14))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn subscription(_state: &State) -> Subscription<Message> {
    iced::event::listen().map(Message::Event)
}

/// Startup-Task: Fenster in den Vordergrund holen (Tastatur-Fokus), da vom
/// Hintergrund-Worker gespawnt.
fn focus_on_open() -> Task<Message> {
    iced::window::latest().and_then(iced::window::gain_focus::<Message>)
}

// ===== Icon (programmatisch gezeichnet, kein externes Asset) =====

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

fn inside_rounded_rect(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    if px < x0 || px > x1 || py < y0 || py > y1 {
        return false;
    }
    let nx = px.clamp(x0 + r, x1 - r);
    let ny = py.clamp(y0 + r, y1 - r);
    let dx = px - nx;
    let dy = py - ny;
    dx * dx + dy * dy <= r * r
}

/// Goldenes Rounded-Square mit weissem Funkeln (Astroid sqrt|x|+sqrt|y| <= 1).
fn app_icon() -> Option<iced::window::Icon> {
    const S: u32 = 256;
    let s = S as f32;
    let pad = s * 0.06;
    let radius = s * 0.22;
    let cx = s / 2.0;
    let cy = s / 2.0;
    let star_r = s * 0.34;
    let mut rgba = vec![0u8; (S * S * 4) as usize];
    for y in 0..S {
        for x in 0..S {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            if !inside_rounded_rect(fx, fy, pad, pad, s - pad, s - pad, radius) {
                continue; // ausserhalb -> transparent
            }
            let idx = ((y * S + x) * 4) as usize;
            // Gold-Verlauf (oben hell -> unten dunkler)
            let t = (fy - pad) / (s - 2.0 * pad);
            let mut r = lerp(0xf5, 0xe8, t);
            let mut g = lerp(0xc8, 0xa9, t);
            let mut b = lerp(0x4a, 0x20, t);
            // weisses Funkeln
            let snx = ((fx - cx) / star_r).abs().sqrt();
            let sny = ((fy - cy) / star_r).abs().sqrt();
            if snx + sny <= 1.0 {
                r = 255;
                g = 255;
                b = 255;
            }
            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = 255;
        }
    }
    iced::window::icon::from_rgba(rgba, S, S).ok()
}

pub fn run() -> anyhow::Result<()> {
    // Antworttext von stdin lesen.
    let mut txt = String::new();
    let _ = std::io::stdin().read_to_string(&mut txt);
    let _ = INITIAL_TEXT.set(txt);

    // Label = Token nach "--result".
    let label = std::env::args()
        .skip_while(|a| a.as_str() != "--result")
        .nth(1)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Claude".to_string());
    let _ = LABEL.set(label);

    iced::application(|| (State::default(), focus_on_open()), update, view)
        .theme(Theme::Dark)
        .title(|state: &State| format!("Claude – {}", state.label))
        .subscription(subscription)
        .window(iced::window::Settings {
            size: Size::new(WINDOW_W as f32, WINDOW_H as f32),
            decorations: true,
            transparent: false,
            resizable: true,
            level: Level::Normal,
            position: Position::Centered,
            icon: app_icon(),
            ..Default::default()
        })
        .run()
        .map_err(|e| anyhow::anyhow!("iced (result): {e}"))
}
