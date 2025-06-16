use std::{cmp, sync::Arc};

use cosmic_text::{fontdb::Source, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
use image::{Pixel, Rgba, RgbaImage};
use vt100::Cell;

static_toml::static_toml! {
    static COLORS = include_toml!("../ayu-dark.toml");
}

pub struct TerminalRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl TerminalRenderer {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new_with_fonts([Source::Binary(Arc::new(include_bytes!(
                "../../../font/JetBrainsMono-VariableFont_wght.ttf"
            )))]),
            swash_cache: SwashCache::new(),
        }
    }

    // for cols, use the --width parameter in table
    pub fn render(&mut self, input: &str, rows: u16, cols: u16) -> RgbaImage {
        let rows = rows * 2;

        let font_size = 22.0;
        let line_height = 14.0;
        let metrics = Metrics::new(font_size, line_height);
        let mut text_buffer = Buffer::new(&mut self.font_system, metrics);
        let mut text_buffer = text_buffer.borrow_with(&mut self.font_system);

        let mut screen = vt100::Parser::new(rows, cols, 0);
        screen.process(input.as_bytes());

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

            spans.push((String::from("\n"), attrs()));
        }

        text_buffer.set_rich_text(
            spans.iter().map(|(c, a)| (c.as_str(), a.clone())),
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

        let mut image_buffer = RgbaImage::new((max_x + min_x) as u32, (max_y + min_y) as u32);
        text_buffer.draw(&mut self.swash_cache, text_color, |x, y, w, h, color| {
            let Ok(x) = u32::try_from(x) else { return };
            let Ok(y) = u32::try_from(y) else { return };
            let color = Rgba::from_slice(&color.as_rgba()).clone();
            for x in x..(x + w) {
                for y in y..(y + h) {
                    image_buffer.get_pixel_mut(x, y).blend(&color);
                }
            }
        });

        image_buffer
    }

    fn attrs_from_cell(cell: &Cell) -> Attrs {
        let mut attrs = Attrs::new().family(Family::Name("JetBrains Mono"));

        let fgcolor = match cell.fgcolor() {
            vt100::Color::Default => None,
            vt100::Color::Idx(30) => Some(Color(COLORS.colors.normal.black as u32)),
            vt100::Color::Idx(_) => None,
            vt100::Color::Rgb(r, g, b) => Some(Color::rgb(r, g, b)),
        };

        attrs
    }
}

#[cfg(test)]
mod tests {
    use image::codecs::png::PngEncoder;

    use super::*;

    #[test]
    fn test_render() {
        let mut renderer = TerminalRenderer::new();
        let input = include_str!("../../../version.ansi");
        let rows = input.lines().count();
        let rows = u16::try_from(rows).unwrap();
        let cols = 100;
        let image = renderer.render(input, rows, cols);

        let file = std::fs::File::create("./version.png").unwrap();
        let encoder = PngEncoder::new(file);
        image.write_with_encoder(encoder).unwrap();
    }
}
