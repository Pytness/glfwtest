pub struct CellPosition {
    pub x: i32,
    pub y: i32,
}

pub struct CellBox {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

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

    pub fn get_cell_position(&self, row: i32, col: i32) -> CellPosition {
        let x = col * self.font_width;
        // Add font_height to y to account for rendering starting from the baseline,
        // so we want to position the text such that it fits within the cell.
        let y = row * self.font_height + self.font_height;

        CellPosition { x, y }
    }

    pub fn get_cell_box(&self, row: i32, col: i32) -> CellBox {
        let CellPosition { x, y } = self.get_cell_position(row, col);

        CellBox {
            x,
            y,
            width: self.font_width,
            height: self.font_height,
        }
    }
}
