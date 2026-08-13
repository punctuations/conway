use std::fmt;

pub const DEFAULT_WIDTH: usize = 64;
pub const MAX_AUTO_HEIGHT: usize = 32;

pub const LIVE_CELL: char = '█';
pub const DEAD_CELL: char = '·';

pub struct Grid {
    width: usize,
    height: usize,
    cells: Vec<bool>,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![false; width * height],
        }
    }

    pub fn from_bytes(bytes: &[u8], width: usize, height: usize) -> Self {
        let mut grid = Self::new(width, height);
        let bits = bytes.len() * 8;
        let cells = width * height;

        if bits == 0 || cells == 0 {
            return grid;
        }

        if bits > cells {
            let mut chunks = Vec::with_capacity(cells);
            let mut total = 0;

            for index in 0..cells {
                let start = index * bits / cells;
                let end = ((index + 1) * bits / cells).max(start + 1);
                let ones = ones_between(bytes, start, end);
                total += ones;
                chunks.push((ones, end - start));
            }

            for (index, (ones, span)) in chunks.into_iter().enumerate() {
                let alive = denser_than_average(ones, span, total, bits);
                grid.set(index % width, index / width, alive);
            }

            return grid;
        }

        let (source_width, source_height) = source_dimensions(bits, width, height);

        for y in 0..height {
            let source_y = y * source_height / height;
            for x in 0..width {
                let source_x = x * source_width / width;
                grid.set(x, y, bit_at(bytes, source_y * source_width + source_x));
            }
        }

        grid
    }

    pub fn step(&mut self) {
        let mut next = vec![false; self.cells.len()];

        for y in 0..self.height {
            for x in 0..self.width {
                let neighbors = self.live_neighbors(x, y);
                next[y * self.width + x] =
                    matches!((self.get(x, y), neighbors), (true, 2) | (true, 3) | (false, 3));
            }
        }

        self.cells = next;
    }

    pub fn live_neighbors(&self, x: usize, y: usize) -> usize {
        let mut count = 0;

        for dy in [-1isize, 0, 1] {
            for dx in [-1isize, 0, 1] {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (Some(nx), Some(ny)) = (offset(x, dx, self.width), offset(y, dy, self.height))
                else {
                    continue;
                };
                if self.get(nx, ny) {
                    count += 1;
                }
            }
        }

        count
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn population(&self) -> usize {
        self.cells.iter().filter(|cell| **cell).count()
    }

    pub fn cells(&self) -> &[bool] {
        &self.cells
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        self.cells[y * self.width + x]
    }

    pub fn set(&mut self, x: usize, y: usize, alive: bool) {
        self.cells[y * self.width + x] = alive;
    }
}

impl fmt::Display for Grid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for y in 0..self.height {
            if y > 0 {
                writeln!(f)?;
            }
            for x in 0..self.width {
                let cell = if self.get(x, y) { LIVE_CELL } else { DEAD_CELL };
                write!(f, "{cell}")?;
            }
        }
        Ok(())
    }
}

fn bit_at(bytes: &[u8], index: usize) -> bool {
    match bytes.get(index / 8) {
        Some(byte) => byte >> (7 - index % 8) & 1 == 1,
        None => false,
    }
}

fn denser_than_average(ones: usize, span: usize, total_ones: usize, total_bits: usize) -> bool {
    let here = ones as u128 * total_bits as u128;
    let average = total_ones as u128 * span as u128;

    if here != average {
        return here > average;
    }

    ones * 2 >= span
}

fn ones_between(bytes: &[u8], start: usize, end: usize) -> usize {
    let mut count = 0;
    let mut index = start;

    while index < end && !index.is_multiple_of(8) {
        count += usize::from(bit_at(bytes, index));
        index += 1;
    }

    while index + 8 <= end {
        if let Some(byte) = bytes.get(index / 8) {
            count += byte.count_ones() as usize;
        }
        index += 8;
    }

    while index < end {
        count += usize::from(bit_at(bytes, index));
        index += 1;
    }

    count
}

fn source_dimensions(bits: usize, width: usize, height: usize) -> (usize, usize) {
    let aspect = width as f64 / height as f64;
    let source_width = ((bits as f64 * aspect).sqrt().ceil() as usize)
        .clamp(1, bits)
        .min(width);
    let source_height = bits.div_ceil(source_width).min(height).max(1);
    (source_width, source_height)
}

fn offset(value: usize, delta: isize, limit: usize) -> Option<usize> {
    let next = value as isize + delta;

    if limit >= 3 {
        return Some(next.rem_euclid(limit as isize) as usize);
    }

    (0..limit as isize).contains(&next).then_some(next as usize)
}

pub fn dimensions_for(
    byte_count: usize,
    width: Option<usize>,
    height: Option<usize>,
) -> (usize, usize) {
    let width = width.unwrap_or(DEFAULT_WIDTH).max(1);
    let height = height
        .unwrap_or_else(|| {
            byte_count
                .saturating_mul(8)
                .div_ceil(width)
                .clamp(1, MAX_AUTO_HEIGHT)
        })
        .max(1);
    (width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_rows(rows: &[&str]) -> Grid {
        let mut grid = Grid::new(rows[0].len(), rows.len());
        for (y, row) in rows.iter().enumerate() {
            for (x, cell) in row.chars().enumerate() {
                grid.set(x, y, cell == '#');
            }
        }
        grid
    }

    fn to_rows(grid: &Grid) -> Vec<String> {
        (0..grid.height())
            .map(|y| {
                (0..grid.width())
                    .map(|x| if grid.get(x, y) { '#' } else { '.' })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn block_is_a_still_life() {
        let mut grid = from_rows(&["......", ".##...", ".##...", "......", "......"]);
        let before = to_rows(&grid);
        grid.step();
        assert_eq!(to_rows(&grid), before);
    }

    #[test]
    fn blinker_has_period_two() {
        let mut grid = from_rows(&[".....", ".....", ".###.", ".....", "....."]);
        let start = to_rows(&grid);

        grid.step();
        assert_eq!(to_rows(&grid), vec![".....", "..#..", "..#..", "..#..", "....."]);

        grid.step();
        assert_eq!(to_rows(&grid), start);
    }

    #[test]
    fn glider_translates_by_one_cell_every_four_generations() {
        let mut grid = Grid::new(12, 12);
        for (x, y) in [(2, 1), (3, 2), (1, 3), (2, 3), (3, 3)] {
            grid.set(x, y, true);
        }

        for _ in 0..4 {
            grid.step();
        }

        let expected: Vec<(usize, usize)> = [(2, 1), (3, 2), (1, 3), (2, 3), (3, 3)]
            .iter()
            .map(|(x, y)| (x + 1, y + 1))
            .collect();

        for y in 0..grid.height() {
            for x in 0..grid.width() {
                assert_eq!(grid.get(x, y), expected.contains(&(x, y)), "at ({x}, {y})");
            }
        }
    }

    #[test]
    fn edges_wrap_around() {
        let mut grid = Grid::new(5, 5);
        grid.set(0, 0, true);
        grid.set(4, 0, true);
        grid.set(0, 4, true);

        assert_eq!(grid.live_neighbors(4, 4), 3);
    }

    #[test]
    fn narrow_grids_do_not_double_count_neighbors() {
        let mut grid = Grid::new(2, 2);
        grid.set(0, 0, true);
        grid.set(1, 0, true);

        assert_eq!(grid.live_neighbors(0, 1), 2);
    }

    #[test]
    fn bytes_seed_cells_most_significant_bit_first() {
        let grid = Grid::from_bytes(&[0b1010_0000], 8, 1);
        assert_eq!(to_rows(&grid), vec!["#.#....."]);
    }

    #[test]
    fn a_full_file_fills_the_grid() {
        let grid = Grid::from_bytes(&[0xff, 0xff, 0xff], 4, 2);
        assert_eq!(to_rows(&grid), vec!["####", "####"]);
    }

    #[test]
    fn an_exact_fit_maps_one_bit_per_cell() {
        let grid = Grid::from_bytes(&[0b1010_1010, 0b1100_0011], 4, 4);
        assert_eq!(to_rows(&grid), vec!["#.#.", "#.#.", "##..", "..##"]);
    }

    #[test]
    fn large_files_map_their_layout_onto_the_grid() {
        let mut bytes = vec![0x00; 500];
        bytes.extend(std::iter::repeat_n(0xff, 500));
        let grid = Grid::from_bytes(&bytes, 10, 10);

        for y in 0..5 {
            for x in 0..10 {
                assert!(!grid.get(x, y), "expected dead at ({x}, {y})");
            }
        }
        for y in 5..10 {
            for x in 0..10 {
                assert!(grid.get(x, y), "expected alive at ({x}, {y})");
            }
        }
    }

    #[test]
    fn a_sparse_file_still_lights_up_its_denser_regions() {
        let mut bytes = vec![0x00; 1000];
        bytes.extend(std::iter::repeat_n(0x01, 1000));

        let grid = Grid::from_bytes(&bytes, 10, 10);

        assert!((0..5).all(|y| (0..10).all(|x| !grid.get(x, y))));
        assert!((5..10).all(|y| (0..10).all(|x| grid.get(x, y))));
    }

    #[test]
    fn bytes_past_the_screen_still_change_the_grid() {
        let head = vec![0xa5; 90];

        let mut quiet = head.clone();
        quiet.extend(std::iter::repeat_n(0x00, 900));

        let mut loud = head.clone();
        loud.extend(std::iter::repeat_n(0xff, 900));

        let quiet = Grid::from_bytes(&quiet, 30, 24);
        let loud = Grid::from_bytes(&loud, 30, 24);

        assert_ne!(quiet.cells(), loud.cells());
    }

    #[test]
    fn small_seeds_are_magnified_across_the_grid() {
        let grid = Grid::from_bytes(&[0xff], 16, 4);
        assert!((0..grid.width()).all(|x| grid.get(x, 0)));
    }

    #[test]
    fn a_single_byte_reaches_every_row_and_column() {
        let grid = Grid::from_bytes(&[0b1011_0010], 40, 20);

        let live_rows = (0..grid.height())
            .filter(|y| (0..grid.width()).any(|x| grid.get(x, *y)))
            .count();
        let live_columns = (0..grid.width())
            .filter(|x| (0..grid.height()).any(|y| grid.get(*x, y)))
            .count();

        assert_eq!(live_rows, grid.height());
        assert!(live_columns > grid.width() / 2, "only {live_columns} columns");
    }

    #[test]
    fn auto_height_is_capped_but_explicit_height_is_not() {
        let (width, height) = dimensions_for(100_000, None, None);
        assert_eq!(width, DEFAULT_WIDTH);
        assert_eq!(height, MAX_AUTO_HEIGHT);

        let (_, tall) = dimensions_for(100_000, None, Some(500));
        assert_eq!(tall, 500);
    }

    #[test]
    fn small_files_get_a_short_grid() {
        let (_, height) = dimensions_for(16, None, None);
        assert_eq!(height, 2);
    }
}
