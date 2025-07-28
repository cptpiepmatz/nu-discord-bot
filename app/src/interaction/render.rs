use std::{cmp, sync::Arc};

use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, fontdb::Source,
};
use image::{Pixel, Rgba, RgbaImage};
use tracing::warn;
use vt100::Cell;

static_toml::static_toml! {
    static COLORS = include_toml!("../ayu-dark.toml");
}

#[derive(Debug)]
pub struct TerminalRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl TerminalRenderer {
    pub const MAX_ROWS: u16 = 64;

    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new_with_fonts([Source::Binary(Arc::new(include_bytes!(
                "../../../font/JetBrainsMono-VariableFont_wght.ttf"
            )))]),
            swash_cache: SwashCache::new(),
        }
    }

    // for cols, use the --width parameter in table
    pub fn render(&mut self, input: &str) -> RgbaImage {
        let rows = cmp::min((input.lines().count()) as u16, Self::MAX_ROWS);
        let cols = console::strip_ansi_codes(input)
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0) as u16;

        let font_size = 22.0;
        let line_height = 28.0;
        let metrics = Metrics::new(font_size, line_height);
        let mut text_buffer = Buffer::new(&mut self.font_system, metrics);
        let mut text_buffer = text_buffer.borrow_with(&mut self.font_system);

        let mut screen = vt100::Parser::new(rows, cols, 0);
        let padded = input
            .lines()
            .map(|line| {
                // this is some delicate machinery here, we need to figure out the width of a line
                // without colors, but Rust sees the escape characters too, so we need to properly 
                // calculate the width including them
                let len = line.chars().count();
                let stripped_len = console::strip_ansi_codes(line).chars().count();
                format!("{:<width$}", line, width = cols as usize + (len - stripped_len))
            })
            .collect::<String>();
        screen.process(padded.as_bytes());

        let attrs = || Attrs::new().family(Family::Name("JetBrains Mono"));

        let screen = screen.screen();
        let mut spans = Vec::with_capacity(rows as usize * cols as usize);
        for row in 0..rows {
            for col in 0..cols {
                let cell = screen.cell(row, col).expect("inside screen");
                if cell.has_contents() {
                    spans.push((cell.contents(), Self::attrs_from_cell(cell)));
                }
            }

            spans.push(("\n", attrs()));
        }

        text_buffer.set_rich_text(
            spans.iter().map(|(c, a)| (c.as_ref(), a.clone())),
            &attrs(),
            Shaping::Basic,
            None,
        );
        let text_color = Color::rgb(255, 255, 255);

        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = 0;
        let mut max_y = 0;
        text_buffer.draw(&mut self.swash_cache, text_color, |x, y, w, h, _| {
            min_x = cmp::min(min_x, x);
            min_y = cmp::min(min_y, y);
            max_x = cmp::max(max_x, x + w as i32);
            max_y = cmp::max(max_y, y + h as i32);
        });

        const MARGIN: i32 = 10;
        let mut image_buffer = RgbaImage::from_pixel(
            (max_x - min_x + MARGIN * 2) as u32,
            (max_y - min_y + MARGIN * 2) as u32,
            Rgba(((COLORS.colors.primary.background as u32) << 8 | 0xFF).to_be_bytes()),
        );
        text_buffer.draw(&mut self.swash_cache, text_color, |x, y, w, h, color| {
            let w = w as i32;
            let h = h as i32;
            let color = Rgba::from_slice(&color.as_rgba()).clone();
            for x in x..(x + w) {
                for y in y..(y + h) {
                    let x = (x + MARGIN - min_x) as u32;
                    let y = (y + MARGIN - min_y) as u32;
                    match image_buffer.get_pixel_mut_checked(x, y) {
                        Some(pixel) => pixel.blend(&color),
                        None => warn!("tried to access pixel out of bounds"),
                    }
                }
            }
        });

        image_buffer
    }

    fn attrs_from_cell(cell: &Cell) -> Attrs {
        let color = match cell.fgcolor() {
            vt100::Color::Idx(00) => Color(COLORS.colors.normal.black as u32),
            vt100::Color::Idx(01) => Color(COLORS.colors.normal.red as u32),
            vt100::Color::Idx(02) => Color(COLORS.colors.normal.green as u32),
            vt100::Color::Idx(03) => Color(COLORS.colors.normal.yellow as u32),
            vt100::Color::Idx(04) => Color(COLORS.colors.normal.blue as u32),
            vt100::Color::Idx(05) => Color(COLORS.colors.normal.magenta as u32),
            vt100::Color::Idx(06) => Color(COLORS.colors.normal.cyan as u32),
            vt100::Color::Idx(07) => Color(COLORS.colors.normal.white as u32),
            vt100::Color::Idx(08) => Color(COLORS.colors.bright.black as u32),
            vt100::Color::Idx(09) => Color(COLORS.colors.bright.red as u32),
            vt100::Color::Idx(10) => Color(COLORS.colors.bright.green as u32),
            vt100::Color::Idx(11) => Color(COLORS.colors.bright.yellow as u32),
            vt100::Color::Idx(12) => Color(COLORS.colors.bright.blue as u32),
            vt100::Color::Idx(13) => Color(COLORS.colors.bright.magenta as u32),
            vt100::Color::Idx(14) => Color(COLORS.colors.bright.cyan as u32),
            vt100::Color::Idx(15) => Color(COLORS.colors.bright.white as u32),
            vt100::Color::Rgb(r, g, b) => Color::rgb(r, g, b),
            vt100::Color::Default | vt100::Color::Idx(_) => {
                Color(COLORS.colors.primary.foreground as u32)
            }
        };

        Attrs::new()
            .family(Family::Name("JetBrains Mono"))
            .color(color)
    }
}
