//! Tastatur-Picker (Listbox, ersetzt fuzzel von Linux).
//! Tab/Pfeil = navigation, Enter = wählen, Esc = abbrechen.

use crate::backend;
use crate::config::{Config, config_path, config_dir};
use crate::output;
use iced::keyboard::{self, Key, key::Named};
use iced::theme::Style as Appearance;
use iced::widget::{button, column, container, scrollable, text};
use iced::window::{Level, Position};
use iced::{Background, Border, Color, Element, Event, Length, Shadow, Size, Subscription, Task, Theme};
use std::process::Command;
use std::sync::OnceLock;

const WINDOW_W: u32 = 400;
const WINDOW_H: u32 = 360;
const SETTINGS_ID: &str = "__settings__";

/// Einmal gegriffene Auswahl (in `run()` befüllt, solange die Quell-App noch
/// den Fokus hat). `State::default` liest hier, statt erneut zu greifen —
/// sonst kommt die zweite Greifung leer zurück, weil dann schon das
/// Picker-Fenster den Fokus hat ("Kein Text markiert").
static GRABBED_SELECTION: OnceLock<String> = OnceLock::new();

struct State {
    modi: Vec<(String, String)>,
    selection: String,
    selected_idx: usize,
}

impl Default for State {
    fn default() -> Self {
        let cfg = Config::load(&config_path()).unwrap_or_default();
        let mut modi: Vec<(String, String)> = cfg
            .modi
            .iter()
            .map(|(id, m)| (id.clone(), m.label.clone()))
            .collect();
        modi.push((
            SETTINGS_ID.to_string(),
            "⚙️ Einstellungen (config.json öffnen)".to_string(),
        ));

        let selection = GRABBED_SELECTION.get().cloned().unwrap_or_default();
        Self {
            modi,
            selection,
            selected_idx: 0,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Choose(usize),
    Event(Event),
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::Choose(idx) => {
            execute(state, idx);
        }
        Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            modifiers,
            ..
        })) => {
            let shift = modifiers.shift();
            let n = state.modi.len();
            match key {
                Key::Named(Named::Tab) | Key::Named(Named::ArrowDown) if !shift => {
                    state.selected_idx = (state.selected_idx + 1) % n;
                }
                Key::Named(Named::Tab) if shift => {
                    state.selected_idx = if state.selected_idx == 0 { n - 1 } else { state.selected_idx - 1 };
                }
                Key::Named(Named::ArrowUp) => {
                    state.selected_idx = if state.selected_idx == 0 { n - 1 } else { state.selected_idx - 1 };
                }
                Key::Named(Named::Enter) => {
                    execute(state, state.selected_idx);
                }
                Key::Named(Named::Escape) => {
                    std::process::exit(0);
                }
                _ => {}
            }
        }
        _ => {}
    }
    Task::none()
}

fn execute(state: &State, idx: usize) {
    let (id, _label) = state.modi.get(idx).cloned().unwrap_or_default();
    if id == SETTINGS_ID {
        let path = config_dir().join("config.json");
        let editor = std::env::var("VISUAL")
            .ok()
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| "notepad".to_string());
        let _ = Command::new(editor).arg(&path).spawn();
        std::process::exit(0);
    }
    if !id.is_empty() {
        // Backend in EIGENEM Prozess starten, der den Picker ueberlebt — sonst
        // killt das folgende process::exit(0) die Arbeit sofort mit (das war
        // "ich klicke Uebersetzen und nichts passiert").
        backend::spawn_mode_worker(&id, &state.selection);
        std::process::exit(0);
    }
}

fn view(state: &State) -> Element<'_, Message> {
    let header = text("Tab = weiter · Enter = ausführen · Esc = abbrechen")
        .size(11)
        .color(Color::from_rgb(0.55, 0.55, 0.55));

    let mut items = column![header].spacing(4).padding(12);

    for (i, (_id, label)) in state.modi.iter().enumerate() {
        let is_sel = i == state.selected_idx;
        let lbl = text(label.clone())
            .size(14)
            .color(Color::from_rgb(0.95, 0.95, 0.95));
        let btn = button(container(lbl).padding(6))
            .width(Length::Fill)
            .padding(0)
            .style(move |_theme: &Theme, status| {
                let bg = match (is_sel, status) {
                    (true, _) => Color::from_rgb(0.18, 0.46, 0.70),
                    (false, button::Status::Hovered) => Color::from_rgb(0.30, 0.30, 0.30),
                    _ => Color::from_rgba(0.20, 0.20, 0.20, 0.95),
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: Color::WHITE,
                    border: Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: 6.0.into(),
                    },
                    shadow: Shadow::default(),
                    snap: false,
                }
            })
            .on_press(Message::Choose(i));
        items = items.push(btn);
    }

    container(scrollable(items))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.16, 0.16, 0.16, 0.97))),
            border: Border {
                color: Color::from_rgb(0.35, 0.35, 0.35),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        })
        .into()
}

fn style(_state: &State, _theme: &Theme) -> Appearance {
    Appearance {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    }
}

fn subscription(_state: &State) -> Subscription<Message> {
    iced::event::listen().map(Message::Event)
}

/// Startup-Task: Fenster in den Vordergrund holen, damit Tastatur-Events
/// (Esc/Tab/Enter) ankommen. Vom Daemon (Hintergrundprozess) gespawnte
/// Fenster bekommen sonst keinen Fokus -> Esc tat nichts.
fn focus_on_open() -> Task<Message> {
    iced::window::latest().and_then(iced::window::gain_focus::<Message>)
}

pub fn run() -> anyhow::Result<()> {
    let sel = output::get_selection_via_clipboard().unwrap_or_default();
    if sel.trim().is_empty() {
        output::notify(
            "Claude Hotkey",
            "Kein Text markiert — markiere was und drück Strg+C, dann nochmal.",
        );
        return Ok(());
    }
    // Genau EINMAL greifen und merken. Würde State::default erneut greifen,
    // läge der Fokus schon beim Picker-Fenster -> leere Auswahl.
    let _ = GRABBED_SELECTION.set(sel);

    iced::application(
        || (State::default(), focus_on_open()),
        update,
        view,
    )
        .style(style)
        .subscription(subscription)
        .window(iced::window::Settings {
            size: Size::new(WINDOW_W as f32, WINDOW_H as f32),
            decorations: false,
            transparent: true,
            resizable: false,
            level: Level::AlwaysOnTop,
            position: Position::Centered,
            ..Default::default()
        })
        .run()
        .map_err(|e| anyhow::anyhow!("iced (listpicker): {e}"))
}
