//! Halbkreis-Picker (Maus-Bedienung).
//! Wird gespawnt durch Klick auf die Pille (`--picker`).
//! Zeigt Modi in einem Halbkreis um eine imaginäre Pillen-Mitte.

use crate::backend;
use crate::config::{Config, config_path};
use crate::output;
use std::sync::OnceLock;
use iced::theme::Style as Appearance;
use iced::widget::{button, container, stack, text, Space};
use iced::window::{Level, Position};
use iced::{Background, Border, Color, Element, Length, Padding, Point, Shadow, Size, Task, Theme};

const WINDOW_W: u32 = 520;
const WINDOW_H: u32 = 360;
const BOTTOM_MARGIN: f32 = 80.0;
const PILL_H: f32 = 44.0;
const RING_RADIUS: f32 = 170.0;
const ITEM_W: f32 = 130.0;
const ITEM_H: f32 = 36.0;

/// Einmal gegriffene Auswahl — siehe listpicker.rs. Verhindert die
/// Doppel-/Dreifach-Greifung, die sonst leer zurückkommt.
static GRABBED_SELECTION: OnceLock<String> = OnceLock::new();

struct State {
    modi: Vec<(String, String)>,
    selection: String,
}

impl Default for State {
    fn default() -> Self {
        let cfg = Config::load(&config_path()).unwrap_or_default();
        let modi: Vec<(String, String)> = cfg
            .modi
            .iter()
            .map(|(id, m)| (id.clone(), m.label.clone()))
            .collect();
        // Auswahl wurde in run() bereits gegriffen (mit App-Fokus) — hier nur
        // noch lesen, NICHT erneut greifen (sonst leer, weil Fokus weg).
        let selection = GRABBED_SELECTION.get().cloned().unwrap_or_default();
        Self { modi, selection }
    }
}

#[derive(Debug, Clone)]
enum Message {
    ModeChosen(String),
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::ModeChosen(id) => {
            // Eigener Worker-Prozess (backend::spawn_mode_worker): muss den
            // Picker ueberleben, sonst killt process::exit(0) die Arbeit sofort.
            backend::spawn_mode_worker(&id, &state.selection);
            std::process::exit(0);
        }
    }
}

fn ring_item<'a>(label: &str, id: &str) -> Element<'a, Message> {
    let lbl = text(label.to_string())
        .size(13)
        .color(Color::from_rgb(0.95, 0.95, 0.95));
    let bg = container(lbl)
        .center_x(Length::Fill)
        .center_y(Length::Fill);
    let id = id.to_string();
    button(bg)
        .width(Length::Fixed(ITEM_W))
        .height(Length::Fixed(ITEM_H))
        .padding(0)
        .style(|_theme: &Theme, status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered => Color::from_rgb(0.18, 0.46, 0.70),
                _ => Color::from_rgba(0.20, 0.20, 0.20, 0.95),
            })),
            text_color: Color::WHITE,
            border: Border {
                color: Color::from_rgb(0.4, 0.4, 0.4),
                width: 1.0,
                radius: 18.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 6.0,
            },
            snap: false,
        })
        .on_press(Message::ModeChosen(id))
        .into()
}

fn view(state: &State) -> Element<'_, Message> {
    let pill_cx = (WINDOW_W as f32) / 2.0;
    let pill_cy = (WINDOW_H as f32) - PILL_H / 2.0;

    let mut layers: Vec<Element<Message>> = Vec::new();
    layers.push(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    );

    let n = state.modi.len();
    for (i, (id, label)) in state.modi.iter().enumerate() {
        let frac = if n == 1 {
            0.5
        } else {
            i as f32 / (n - 1) as f32
        };
        let angle = std::f32::consts::PI * (1.0 - frac);
        let dx = RING_RADIUS * angle.cos();
        let dy = RING_RADIUS * angle.sin();
        let item_cx = pill_cx + dx;
        let item_cy = pill_cy - dy;
        let left = (item_cx - ITEM_W / 2.0).max(0.0);
        let top = (item_cy - ITEM_H / 2.0).max(0.0);
        let positioned = container(ring_item(label, id))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Left)
            .align_y(iced::alignment::Vertical::Top)
            .padding(Padding {
                top,
                right: 0.0,
                bottom: 0.0,
                left,
            });
        layers.push(positioned.into());
    }

    stack(layers).into()
}

fn style(_state: &State, _theme: &Theme) -> Appearance {
    Appearance {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    }
}

/// Startup-Task: Fenster in den Vordergrund holen (Tastatur-Fokus).
fn focus_on_open() -> Task<Message> {
    iced::window::latest().and_then(iced::window::gain_focus::<Message>)
}

pub fn run() -> anyhow::Result<()> {
    // Pre-check: ohne Selection kein UI öffnen
    let sel = output::get_selection_via_clipboard().unwrap_or_default();
    if sel.trim().is_empty() {
        output::notify(
            "Claude Hotkey",
            "Kein Text markiert — markiere was und drück Strg+C, dann nochmal.",
        );
        return Ok(());
    }
    let _ = GRABBED_SELECTION.set(sel);

    iced::application(
        || (State::default(), focus_on_open()),
        update,
        view,
    )
        .style(style)
        .window(iced::window::Settings {
            size: Size::new(WINDOW_W as f32, WINDOW_H as f32),
            decorations: false,
            transparent: true,
            resizable: false,
            level: Level::AlwaysOnTop,
            position: Position::SpecificWith(|window_size: Size, screen_size: Size| {
                Point::new(
                    (screen_size.width - window_size.width) / 2.0,
                    screen_size.height - window_size.height - BOTTOM_MARGIN,
                )
            }),
            ..Default::default()
        })
        .run()
        .map_err(|e| anyhow::anyhow!("iced (picker): {e}"))
}
