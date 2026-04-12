use std::sync::LazyLock;

use freetype::face::LoadFlag;
use freetype::{Face, GlyphSlot, Library};

const DEFAULT_DPI: u32 = 96;

static FT_LIB: LazyLock<Library> =
    LazyLock::new(|| Library::init().expect("failed to initialize FreeType library"));
pub struct FontEntry {
    pub name: String,
    pub bytes: Vec<u8>,
    pub ft_face: Face,
}

pub struct FontRegistry {
    fonts: Vec<FontEntry>,
}

impl FontRegistry {
    pub fn new() -> Self {
        FontRegistry { fonts: Vec::new() }
    }

    pub fn register_font(&mut self, name: &str, bytes: &[u8]) {
        let bytes = bytes.to_vec();

        let ft_face = FT_LIB
            .new_memory_face(bytes.clone(), 0)
            .expect("failed to load freetype face");

        ft_face.has_color().then(|| {
            println!("Font '{}' has color glyphs", name);
        });

        self.fonts.push(FontEntry {
            name: name.to_string(),
            bytes: bytes.into(),
            ft_face,
        });
    }

    pub fn get_fonts(&self) -> &[FontEntry] {
        &self.fonts
    }

    /// Find and set best matching fixed size for the given pixel size.
    fn set_color_size(&self, ft_face: &Face, pixel_size: isize, dpi: Option<u32>) {
        let raw = ft_face.raw();
        let num = unsafe { (*raw).num_fixed_sizes };

        if num == 0 {
            println!(
                "Font '{}' does not have fixed sizes, skipping color size setting",
                ft_face.family_name().unwrap_or("unknown".to_string())
            );
            return;
        }

        println!(
            "Font '{}' has {} fixed sizes, selecting best match for pixel size {}",
            ft_face.family_name().unwrap_or("unknown".to_string()),
            num,
            pixel_size
        );

        let availables_sizes =
            unsafe { std::slice::from_raw_parts((*raw).available_sizes, num as usize) };

        let mut best_diff = isize::MAX;
        let mut best_match_index = 0;

        for (i, size) in availables_sizes.iter().enumerate() {
            println!(
                "Available size {}: width={}, height={}, pixel_size={}",
                i, size.width, size.height, size.y_ppem
            );
            let diff = (pixel_size - size.width as isize).abs();

            if diff < best_diff {
                best_diff = diff;
                best_match_index = i;
            }
        }

        ft_face
            .select_size(best_match_index as i32)
            .expect("failed to select color size");
    }

    /// Sets the character size for all registered fonts.
    pub fn set_char_size(&self, char_size: isize, dpi: Option<u32>) {
        let char_size = char_size * 64;
        let dpi = dpi.unwrap_or(DEFAULT_DPI);

        for font in &self.fonts {
            println!(
                "Setting char size for font '{}': char_size={}, dpi={}",
                font.name, char_size, dpi
            );

            if !font.ft_face.has_color() {
                font.ft_face
                    .set_char_size(0, char_size, dpi, dpi)
                    .expect("failed to set char size");
            } else {
                self.set_color_size(&font.ft_face, char_size, Some(dpi));
                println!(
                    "Skipping char size setting for font '{}' because it has color glyphs",
                    font.name
                );
            }
        }
    }

    pub fn get_char_index(&self, char_code: char) -> Option<(u32, &Face)> {
        for entry in self.fonts.iter() {
            let glyph_id = entry.ft_face.get_char_index(char_code as usize);
            println!(
                "Font '{}': char code '{}' (U+{:04X}) maps to glyph ID {:?}",
                entry.name, char_code, char_code as u32, glyph_id
            );

            if let Some(glyph_id) = glyph_id {
                if glyph_id != 0 {
                    println!(
                        "Found glyph ID {} for char code '{}' in font '{}'",
                        glyph_id, char_code, entry.name
                    );
                    return Some((glyph_id, &entry.ft_face));
                }
            }
        }

        None
    }

    pub fn load_glyph(&self, glyph_id: u32, load_flags: LoadFlag) -> Option<&GlyphSlot> {
        let mut glyph: Option<&GlyphSlot> = None;

        for entry in self.fonts.iter() {
            let r = entry.ft_face.load_glyph(glyph_id, load_flags);

            if let Ok(_) = r {
                let g = entry.ft_face.glyph();
                let metrics = g.metrics();
                println!(
                    "Loaded glyph {} from font '{}', metrics: width={}, height={}, horiAdvance={}, vertAdvance={}",
                    glyph_id,
                    entry.name,
                    metrics.width,
                    metrics.height,
                    metrics.horiAdvance,
                    metrics.vertAdvance
                );
                glyph = Some(entry.ft_face.glyph());
                break;
            }
        }

        return glyph;
    }

    pub fn load_glyph_by_char(
        &self,
        char_code: char,
        mut load_flags: LoadFlag,
    ) -> Option<&GlyphSlot> {
        if let Some((glyph_id, face)) = self.get_char_index(char_code) {
            println!(
                "Loading glyph ID {} for char code '{}' from font '{}'",
                glyph_id,
                char_code,
                face.family_name().unwrap_or("unknown".to_string())
            );

            if face.has_color() {
                load_flags |= LoadFlag::COLOR;
            }

            return face
                .load_glyph(glyph_id, load_flags)
                .ok()
                .map(|_| face.glyph());
        }

        None
    }

    pub fn size_metrics(&self) -> Option<freetype::ffi::FT_Size_Metrics> {
        let font = self.fonts.first()?;

        font.ft_face.size_metrics()
    }
}
