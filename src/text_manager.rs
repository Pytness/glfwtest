pub struct TextManager {
    font_width: i32,
    font_height: i32,
    rows: i32,
    cols: i32,
}

impl TextManager {
    pub fn new(font_width: i32, font_height: i32, window_width: i32, window_height: i32) -> Self {
        let cols = window_width / font_width;
        let rows = window_height / font_height;

        Self {
            font_width,
            font_height,
            rows,
            cols,
        }
    }

    pub fn get_cell_position(&self, row: i32, col: i32) -> (i32, i32) {
        let x = col * self.font_width;
        let y = row * self.font_height;

        (x, y)
    }

    pub fn get_cell_box(&self, row: i32, col: i32) -> (i32, i32, i32, i32) {
        let (x, y) = self.get_cell_position(row, col);
        (x, y, self.font_width, self.font_height)
    }
}
