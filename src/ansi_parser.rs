const INDEXED_COLORS: [(u8, u8, u8); 8] = [
    // Black
    (0, 0, 0),
    // Red
    (255, 0, 0),
    // Green
    (0, 255, 0),
    // Yellow
    (255, 255, 0),
    // Blue
    (0, 0, 255),
    // Purple
    (255, 0, 255),
    // Cyan
    (0, 255, 255),
    // White
    (255, 255, 255),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl Color {
    pub fn to_rgba(&self) -> (u8, u8, u8, u8) {
        match self {
            Color::Default => (255, 255, 255, 255),
            Color::Indexed(i) => {
                let i = *i;
                if i < 16 {
                    // Standard colors
                    let (r, g, b) = INDEXED_COLORS[i as usize];
                    (r, g, b, 255)
                } else {
                    panic!("Unsupported indexed color: {}", i);
                }
            }
            Color::Rgb(r, g, b) => (*r, *g, *b, 255),
        }
    }

    pub fn to_rgb(&self) -> (u8, u8, u8) {
        let (r, g, b, _) = self.to_rgba();
        (r, g, b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fg: Color::Default,
            bg: Color::Default,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Ground,
    Esc,
    Csi,
}

pub struct AnsiParser {
    state: State,
    buf: [u8; 64],
    len: usize,
    pub style: Style,
}

impl AnsiParser {
    pub fn new() -> Self {
        Self {
            state: State::Ground,
            buf: [0; 64],
            len: 0,
            style: Style::default(),
        }
    }

    /// Feeds one byte.
    /// Returns Some(byte) only for printable non-escape text.
    pub fn push(&mut self, b: u8) -> Option<u8> {
        match self.state {
            State::Ground => {
                if b == 0x1b {
                    self.state = State::Esc;
                    None
                } else {
                    Some(b)
                }
            }
            State::Esc => {
                if b == b'[' {
                    self.state = State::Csi;
                    self.len = 0;
                } else {
                    self.state = State::Ground;
                }
                None
            }
            State::Csi => {
                if b == b'm' {
                    self.apply_sgr();
                    self.state = State::Ground;
                    self.len = 0;
                } else if self.len < self.buf.len() {
                    self.buf[self.len] = b;
                    self.len += 1;
                } else {
                    // overflow: drop sequence
                    self.state = State::Ground;
                    self.len = 0;
                }
                None
            }
        }
    }

    fn apply_sgr(&mut self) {
        let s = match core::str::from_utf8(&self.buf[..self.len]) {
            Ok(s) => s,
            Err(_) => return,
        };

        // Empty SGR = reset
        if s.is_empty() {
            self.style = Style::default();
            return;
        }

        let mut nums = [0u16; 16];
        let mut count = 0;

        for part in s.split(';') {
            if count >= nums.len() {
                break;
            }
            nums[count] = part.parse::<u16>().unwrap_or(0);
            count += 1;
        }

        let mut i = 0;
        while i < count {
            match nums[i] {
                0 => {
                    self.style = Style::default();
                    i += 1;
                }

                39 => {
                    self.style.fg = Color::Default;
                    i += 1;
                }
                49 => {
                    self.style.bg = Color::Default;
                    i += 1;
                }

                30..=37 => {
                    self.style.fg = Color::Indexed((nums[i] - 30) as u8);
                    i += 1;
                }
                90..=97 => {
                    self.style.fg = Color::Indexed((nums[i] - 90 + 8) as u8);
                    i += 1;
                }

                40..=47 => {
                    self.style.bg = Color::Indexed((nums[i] - 40) as u8);
                    i += 1;
                }
                100..=107 => {
                    self.style.bg = Color::Indexed((nums[i] - 100 + 8) as u8);
                    i += 1;
                }

                38 | 48 => {
                    let is_fg = nums[i] == 38;

                    if i + 1 < count {
                        match nums[i + 1] {
                            5 if i + 2 < count => {
                                let c = Color::Indexed(nums[i + 2] as u8);
                                if is_fg {
                                    self.style.fg = c;
                                } else {
                                    self.style.bg = c;
                                }
                                i += 3;
                            }
                            2 if i + 4 < count => {
                                let c = Color::Rgb(
                                    nums[i + 2] as u8,
                                    nums[i + 3] as u8,
                                    nums[i + 4] as u8,
                                );
                                if is_fg {
                                    self.style.fg = c;
                                } else {
                                    self.style.bg = c;
                                }
                                i += 5;
                            }
                            _ => {
                                i += 1;
                            }
                        }
                    } else {
                        i += 1;
                    }
                }

                _ => {
                    i += 1;
                }
            }
        }
    }
}
