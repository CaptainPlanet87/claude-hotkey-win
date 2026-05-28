//! Pille — kleines floating Window unten zentral.
//! Iced ohne Layer-Shell (auf Windows nicht verfügbar) — wir nutzen
//! borderless top-most Window mit Position::SpecificWith.

use iced::theme::Style as Appearance;
use iced::widget::{button, container, text};
use iced::window::{Level, Position};
use iced::{Background, Border, Color, Element, Length, Point, Shadow, Size, Task, Theme};
use std::process::Command;

const PILL_W: u32 = 200;
const PILL_H: u32 = 70;
const PILL_BUTTON_W: f32 = 140.0;
const PILL_BUTTON_H: f32 = 44.0;
const BOTTOM_MARGIN: f32 = 80.0;

fn self_exe() -> std::path::PathBuf {
    std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("claude-hotkey.exe"))
}

#[derive(Default)]
struct PillState;

#[derive(Debug, Clone)]
enum Message {
    Clicked,
}

fn update(_state: &mut PillState, message: Message) -> Task<Message> {
    match message {
        Message::Clicked => {
            log::info!("[pill] Klick → spawne Picker");
            let _ = Command::new(self_exe()).arg("--picker").spawn();
            Task::none()
        }
    }
}

fn view(_state: &PillState) -> Element<'_, Message> {
    let label = text("Claude")
        .size(16)
        .color(Color::from_rgb(0.95, 0.95, 0.95));
    let bg = container(label)
        .center_x(Length::Fill)
        .center_y(Length::Fill);

    let btn = button(bg)
        .width(Length::Fixed(PILL_BUTTON_W))
        .height(Length::Fixed(PILL_BUTTON_H))
        .padding(0)
        .style(|_theme: &Theme, status| button::Style {
            background: Some(Background::Color(match status {
                button::Status::Hovered => Color::from_rgb(0.18, 0.46, 0.70),
                _ => Color::from_rgba(0.17, 0.17, 0.17, 0.95),
            })),
            text_color: Color::WHITE,
            border: Border {
                color: Color::from_rgb(0.4, 0.4, 0.4),
                width: 1.0,
                radius: 22.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 8.0,
            },
            snap: false,
        })
        .on_press(Message::Clicked);

    container(btn)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn style(_state: &PillState, _theme: &Theme) -> Appearance {
    Appearance {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    }
}

pub fn run() -> anyhow::Result<()> {
    iced::application(PillState::default, update, view)
        .style(style)
        .window(iced::window::Settings {
            size: Size::new(PILL_W as f32, PILL_H as f32),
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
        .map_err(|e| anyhow::anyhow!("iced (pill): {e}"))
}
