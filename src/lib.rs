extern crate freetype;

use std::convert::From;

#[derive(Debug)]
pub enum Error {
    Io(),
    Freetype(freetype::Error),
}

impl From<freetype::Error> for Error {
    fn from(error: freetype::Error) -> Self {
        Error::Freetype(error)
    }
}

pub struct Font {
    face: freetype::Face,
}

impl Font {
    pub fn load(path: &str) -> Result<Font, Error> {
        use freetype::Library;
        let lib = Library::init().unwrap();
        let face = lib.new_face(path, 0)?;
        Ok(Font { face: face })
    }

    pub fn generate(&mut self, name: &str, size: isize, subset: &str, epd_crate: &str) -> String {
        let mut subset = subset.chars().collect::<Vec<_>>();
        subset.sort();
        // Set the resultion to 72dpi so that a point equals a pixel.
        self.face.set_char_size(0, size * 64, 72, 72).unwrap();
        // Generate all glyphs.
        let mut glyphs = Vec::new();
        for c in subset.iter() {
            glyphs.push(self.generate_glyph(*c, epd_crate));
        }
        // Generate the font.
        let size = self.face.size_metrics().unwrap();
        format!(
            "pub const {}: {}::gui::font::Font = {}::gui::font::Font {{
    ascender: {},
    descender: {},
    glyphs: &[
        {}
    ],
    get_glyph_index: {},
}};
",
            name,
            epd_crate,
            epd_crate,
            (size.ascender + 63) / 64,
            -(size.descender + 63) / 64,
            glyphs.join(",\n        "),
            Self::generate_get_glyph_index(subset),
        )
    }

    fn generate_get_glyph_index(chars: Vec<char>) -> String {
        let mut code = "".to_string();
        let mut run_start = chars[0] as u32;
        let mut run_length = 1;
        for i in 1..chars.len() {
            let c = chars[i] as u32;
            if c == run_start + run_length {
                run_length += 1;
            } else {
                code += &Self::generate_get_glyph_index_range(
                    run_start,
                    run_length,
                    i - run_length as usize,
                );
                run_start = c;
                run_length = 1;
            }
        }
        code += &Self::generate_get_glyph_index_range(
            run_start,
            run_length,
            chars.len() - run_length as usize,
        );

        format!(
            "|c: char| -> Option<usize> {{
        let c = c as usize;
        {}None
    }}",
            code
        )
    }

    fn generate_get_glyph_index_range(
        run_start: u32,
        run_length: u32,
        start_index: usize,
    ) -> String {
        if run_length == 1 {
            format!(
                "if c == {} {{
            return Some({});
        }}
        ",
                run_start, start_index
            )
        } else {
            format!(
                "if c >= {} && c < {} {{
            return Some({} + c - {});
        }}
        ",
                run_start,
                run_start + run_length,
                start_index,
                run_start
            )
        }
    }

    fn generate_glyph(&mut self, c: char, epd_crate: &str) -> String {
        self.face
            .load_char(
                c as usize,
                freetype::face::LoadFlag::RENDER | freetype::face::LoadFlag::TARGET_MONO,
            )
            .unwrap();
        let glyph = self.face.glyph();
        let image = Self::generate_rle_image(&glyph.bitmap(), epd_crate);
        //assert!(glyph.bitmap_left() >= 0);
        assert!(glyph.bitmap_top() >= 0);
        format!(
            "{}::gui::font::Glyph {{
                image: {},
                image_left: {},
                image_top: {},
                advance: {},
        }}",
            epd_crate,
            image,
            glyph.bitmap_left(),
            glyph.bitmap_top(),
            (glyph.advance().x + 63) / 64
        )
    }

    fn generate_rle_image(bm: &freetype::Bitmap, epd_crate: &str) -> String {
        let buffer = bm.buffer();
        let pitch = bm.pitch() as usize;
        let width = bm.width() as usize;
        let height = bm.rows() as usize;

        let mut data = vec![0u16; height + 1];
        data[0] = data.len() as u16;

        for y in 0..height {
            let row = &buffer[y * pitch..(y + 1) * pitch];

            Self::generate_rle(&mut data, row, width);
            data[y + 1] = data.len() as u16;
        }

        let mut data_text = "[".to_string();
        for i in 0..data.len() {
            if (i & 15) == 0 {
                data_text += "\n                        ";
            }
            data_text += &format!("{},", data[i]);
            if i & 15 != 15 && i != data.len() - 1 {
                data_text += " ";
            }
        }
        data_text += "\n                    ]";
        format!(
            "{}::gui::image::RLEImage {{
                    data: &{},
                    width: {},
                    height: {},
                }}",
            epd_crate, data_text, width, height
        )
    }

    fn generate_rle(output: &mut Vec<u16>, row: &[u8], width: usize) {
        let mut run_color = (row[0] & 0x80) >> 7;
        let mut run_length = 0;

        let mut bits = 0;
        for i in 0..width {
            let byte = row[i / 8];
            let bit = (byte >> (7 - bits)) & 1;
            if bit == run_color {
                run_length += 1;
            } else {
                output.push(((run_color as u16) << 15) | run_length);
                run_length = 1;
                run_color = bit;
            }
            bits += 1;
            if bits == 8 {
                bits = 0;
            }
        }
        output.push(((run_color as u16) << 15) | run_length);
    }
}
