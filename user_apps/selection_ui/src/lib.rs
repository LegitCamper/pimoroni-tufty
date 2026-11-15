#![no_std]

extern crate alloc;

use embedded_graphics::{
    Drawable,
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_6X10, FONT_10X20},
    },
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use tufty::{
    Buttons,
    display::Display,
    input::KeyCode,
    selection::{ROWS, navigate, visible_range},
};

const INK: Rgb565 = Rgb565::new(2, 5, 7);
const PANEL: Rgb565 = Rgb565::new(4, 10, 12);
const MINT: Rgb565 = Rgb565::new(15, 60, 24);
const MUTED: Rgb565 = Rgb565::new(18, 39, 22);

#[derive(Debug)]
pub enum SelectionUiError<DisplayError> {
    SelectionListEmpty,
    DisplayError(DisplayError),
}

pub struct SelectionUi<'a> {
    selection: usize,
    items: &'a [&'a str],
    title: &'a str,
    hint: &'a str,
}

impl<'a> SelectionUi<'a> {
    pub fn new(items: &'a [&'a str]) -> Self {
        Self {
            selection: 0,
            items,
            title: "GAME LIBRARY",
            hint: "Add ROMs to the content pack",
        }
    }

    pub fn with_title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    pub fn with_hint(mut self, hint: &'a str) -> Self {
        self.hint = hint;
        self
    }

    pub fn run_selection_ui(
        &mut self,
        display: &mut Display,
    ) -> Result<Option<usize>, SelectionUiError<()>> {
        self.draw(display).map_err(SelectionUiError::DisplayError)?;
        let mut previous = tufty::buttons();
        let mut repeat_at = tufty::get_ms() + 400;
        loop {
            let held = tufty::buttons();
            let mut pressed = held & !previous;
            let now = tufty::get_ms();
            if held != previous {
                repeat_at = now + 400;
            } else if now >= repeat_at {
                pressed |= held & (Buttons::UP | Buttons::DOWN);
                repeat_at = now + 100;
            }
            previous = held;
            if pressed.intersects(Buttons::B | Buttons::HOME) {
                return Ok(None);
            }
            let key = if pressed.intersects(Buttons::A | Buttons::START) {
                KeyCode::Enter
            } else if pressed.contains(Buttons::DOWN) {
                KeyCode::Down
            } else if pressed.contains(Buttons::UP) {
                KeyCode::Up
            } else if pressed.intersects(Buttons::RIGHT | Buttons::C) {
                KeyCode::Right
            } else if pressed.contains(Buttons::LEFT) {
                KeyCode::Left
            } else {
                tufty::sleep(10);
                continue;
            };
            if self.items.is_empty() {
                continue;
            }
            if let Some(selected) = self.update(display, key)? {
                display.clear(INK).map_err(SelectionUiError::DisplayError)?;
                return Ok(Some(selected));
            }
        }
    }

    pub fn update(
        &mut self,
        display: &mut Display,
        key: KeyCode,
    ) -> Result<Option<usize>, SelectionUiError<()>> {
        if self.items.is_empty() {
            return Err(SelectionUiError::SelectionListEmpty);
        }
        let delta = match key {
            KeyCode::Down => 1,
            KeyCode::Up => -1,
            KeyCode::Right => ROWS as isize,
            KeyCode::Left => -(ROWS as isize),
            KeyCode::Enter => return Ok(Some(self.selection)),
            _ => return Ok(None),
        };
        self.selection = navigate(self.items.len(), self.selection, delta);
        self.draw(display).map_err(SelectionUiError::DisplayError)?;
        Ok(None)
    }

    pub fn draw<D: DrawTarget<Color = Rgb565>>(&self, display: &mut D) -> Result<(), D::Error> {
        display.clear(INK)?;
        for column in (0..320).step_by(16) {
            Rectangle::new(Point::new(column, 0), Size::new(1, 48))
                .into_styled(PrimitiveStyle::with_fill(PANEL))
                .draw(display)?;
        }
        Text::new(
            "TUFTY / ARCADE",
            Point::new(12, 13),
            MonoTextStyle::new(&FONT_6X10, MINT),
        )
        .draw(display)?;
        let mut label = alloc::string::String::new();
        let _ = tufty::filename::write_label(&mut label, self.title, 28);
        Text::new(
            &label,
            Point::new(12, 36),
            MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE),
        )
        .draw(display)?;
        Rectangle::new(Point::new(12, 45), Size::new(296, 2))
            .into_styled(PrimitiveStyle::with_fill(MINT))
            .draw(display)?;
        if self.items.is_empty() {
            Text::new(
                "NO CARTRIDGES",
                Point::new(36, 112),
                MonoTextStyle::new(&FONT_10X20, MINT),
            )
            .draw(display)?;
            Text::new(
                self.hint,
                Point::new(12, 142),
                MonoTextStyle::new(&FONT_6X10, MUTED),
            )
            .draw(display)?;
        } else {
            for (row, index) in visible_range(self.items.len(), self.selection).enumerate() {
                let top = 54 + row as i32 * 29;
                let selected = index == self.selection;
                let (background, foreground) = if selected {
                    (MINT, INK)
                } else {
                    (PANEL, Rgb565::WHITE)
                };
                Rectangle::new(Point::new(12, top), Size::new(296, 25))
                    .into_styled(PrimitiveStyle::with_fill(background))
                    .draw(display)?;
                Text::new(
                    if selected { ">" } else { ":" },
                    Point::new(18, top + 18),
                    MonoTextStyle::new(&FONT_10X20, foreground),
                )
                .draw(display)?;
                label.clear();
                let filename = self.items[index]
                    .rsplit('/')
                    .next()
                    .unwrap_or(self.items[index]);
                let _ = tufty::filename::write_label(&mut label, filename, 26);
                Text::new(
                    &label,
                    Point::new(34, top + 18),
                    MonoTextStyle::new(&FONT_10X20, foreground),
                )
                .draw(display)?;
            }
        }
        let page = if self.items.is_empty() {
            0
        } else {
            self.selection / ROWS + 1
        };
        let status = alloc::format!(
            "{} GAMES   PAGE {}/{}",
            self.items.len(),
            page,
            self.items.len().div_ceil(ROWS)
        );
        Text::new(
            &status,
            Point::new(12, 211),
            MonoTextStyle::new(&FONT_6X10, MUTED),
        )
        .draw(display)?;
        Text::new(
            "A PLAY  B BACK  C PAGE  UP/DOWN MOVE",
            Point::new(12, 232),
            MonoTextStyle::new(&FONT_6X10, MINT),
        )
        .draw(display)?;
        Ok(())
    }
}

pub fn notice(display: &mut Display, title: &str, message: &str) {
    let _ = display.clear(INK);
    let _ = Text::new(
        title,
        Point::new(12, 58),
        MonoTextStyle::new(&FONT_10X20, MINT),
    )
    .draw(display);
    let mut wrapped = alloc::string::String::new();
    for line in message.lines() {
        let mut column = 0;
        for word in line.split_whitespace() {
            if column != 0 && column + 1 + word.len() > 48 {
                wrapped.push('\n');
                column = 0;
            }
            if column != 0 {
                wrapped.push(' ');
                column += 1;
            }
            wrapped.push_str(word);
            column += word.len();
        }
        wrapped.push('\n');
    }
    let _ = Text::new(
        &wrapped,
        Point::new(12, 90),
        MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE),
    )
    .draw(display);
    let _ = Text::new(
        "A CONTINUE   B/HOME BACK",
        Point::new(12, 222),
        MonoTextStyle::new(&FONT_6X10, MUTED),
    )
    .draw(display);
}

pub fn draw_text_center<S>(display: &mut Display, text: &str, style: S) -> Result<Point, ()>
where
    S: embedded_graphics::text::renderer::TextRenderer<Color = Rgb565>,
{
    Text::with_alignment(
        text,
        display.bounding_box().center(),
        style,
        embedded_graphics::text::Alignment::Center,
    )
    .draw(display)
}
