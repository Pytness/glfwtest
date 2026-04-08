pub struct FontEntry {
    pub name: String,
    pub bytes: Vec<u8>,
}

pub struct FontRegistry {
    fonts: Vec<FontEntry>,
}

impl FontRegistry {
    pub fn new() -> Self {
        FontRegistry { fonts: Vec::new() }
    }

    pub fn register_font(&mut self, name: &str, bytes: &[u8]) {
        self.fonts.push(FontEntry {
            name: name.to_string(),
            bytes: bytes.into(),
        });
    }

    pub fn get_fonts(&self) -> &[FontEntry] {
        &self.fonts
    }
}
