use crate::game::{self, Grid};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor};
use crossterm::{cursor, execute, queue, terminal};
use std::io::{self, Write};
use std::time::{Duration, Instant};

pub enum Ending {
    Extinct,
    Settled,
    Limit,
    Quit,
}

impl Ending {
    pub fn describe(&self) -> &'static str {
        match self {
            Ending::Extinct => "everything died",
            Ending::Settled => "the board settled",
            Ending::Limit => "generation limit reached",
            Ending::Quit => "quit",
        }
    }
}

pub struct Watcher {
    previous: Vec<Vec<bool>>,
}

impl Watcher {
    pub fn new() -> Self {
        Self {
            previous: Vec::new(),
        }
    }

    pub fn verdict(&mut self, grid: &Grid) -> Option<Ending> {
        if grid.population() == 0 {
            return Some(Ending::Extinct);
        }

        if self.previous.iter().any(|state| state == grid.cells()) {
            return Some(Ending::Settled);
        }

        self.previous.push(grid.cells().to_vec());
        if self.previous.len() > 2 {
            self.previous.remove(0);
        }

        None
    }
}

struct Screen;

impl Screen {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(
            io::stdout(),
            terminal::EnterAlternateScreen,
            cursor::Hide,
            terminal::Clear(terminal::ClearType::All)
        )?;
        Ok(Self)
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            ResetColor,
            cursor::Show,
            terminal::LeaveAlternateScreen
        );
        let _ = terminal::disable_raw_mode();
    }
}

enum Action {
    Quit,
    TogglePause,
    Restart,
    GoTo,
    Ignore,
}

const HINT: &str = "space pause · g goto · r restart · q quit ";
const PROMPT_HINT: &str = "enter confirm · esc cancel ";
const SKIP_CHECK: usize = 16;
const SKIP_REDRAW: Duration = Duration::from_millis(50);
const SKIP_POLL: Duration = Duration::from_millis(1);
const IDLE_POLL: Duration = Duration::from_millis(250);

fn action_for(code: KeyCode, modifiers: KeyModifiers) -> Action {
    match (code, modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::Quit,
        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => Action::Quit,
        (KeyCode::Char(' '), _) => Action::TogglePause,
        (KeyCode::Char('r'), _) => Action::Restart,
        (KeyCode::Char('g'), _) => Action::GoTo,
        _ => Action::Ignore,
    }
}

fn fast_forward(
    out: &mut impl Write,
    painter: &mut Painter,
    grid: &mut Grid,
    bytes: &[u8],
    generation: &mut usize,
    watcher: &mut Watcher,
    target: usize,
) -> io::Result<Option<Ending>> {
    if target < *generation {
        let (width, height) = (grid.width(), grid.height());
        *grid = Grid::from_bytes(bytes, width, height);
        *generation = 0;
        *watcher = Watcher::new();
    }

    let mut drawn = Instant::now();

    while *generation < target {
        grid.step();
        *generation += 1;

        if let Some(ending) = watcher.verdict(grid) {
            return Ok(Some(ending));
        }

        if !generation.is_multiple_of(SKIP_CHECK) || drawn.elapsed() < SKIP_REDRAW {
            continue;
        }
        drawn = Instant::now();

        while event::poll(SKIP_POLL)? {
            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
                && matches!(action_for(key.code, key.modifiers), Action::Quit)
            {
                return Ok(None);
            }
        }

        let status = format!("skipping to {target}");
        painter.draw(out, grid, *generation, &status, PROMPT_HINT)?;
    }

    Ok(None)
}

pub fn viewport() -> (usize, usize) {
    let (columns, rows) = terminal::size().unwrap_or((80, 24));
    (
        (columns as usize).max(1),
        (rows as usize).saturating_sub(1).max(1),
    )
}

pub fn run(
    grid: &mut Grid,
    bytes: &[u8],
    limit: Option<usize>,
    delay: Duration,
) -> io::Result<Ending> {
    let screen = Screen::enter()?;
    let outcome = drive(grid, bytes, limit, delay);
    drop(screen);
    outcome
}

fn drive(
    grid: &mut Grid,
    bytes: &[u8],
    limit: Option<usize>,
    delay: Duration,
) -> io::Result<Ending> {
    let mut out = io::stdout();
    let mut watcher = Watcher::new();
    let mut generation = 0usize;
    let mut paused = false;
    let mut finished: Option<Ending> = None;
    let mut prompt: Option<String> = None;
    let mut painter = Painter::default();
    let mut dirty = true;

    loop {
        let idle = finished.is_some() || paused || prompt.is_some();

        let typed;
        let status = match (&prompt, &finished) {
            (Some(buffer), _) => {
                typed = format!("go to generation: {buffer}");
                typed.as_str()
            }
            (None, Some(ending)) => ending.describe(),
            (None, None) if paused => "paused",
            (None, None) => "running",
        };
        let hint = if prompt.is_some() { PROMPT_HINT } else { HINT };
        if dirty {
            painter.draw(&mut out, grid, generation, status, hint)?;
            dirty = false;
        }

        let deadline = Instant::now() + if idle { IDLE_POLL } else { delay };
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            if !event::poll(remaining)? {
                break;
            }
            match event::read()? {
                Event::Resize(..) => dirty = true,
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    dirty = true;
                    if let Some(buffer) = prompt.as_mut() {
                        match key.code {
                            KeyCode::Char(digit) if digit.is_ascii_digit() => {
                                if buffer.len() < 12 {
                                    buffer.push(digit);
                                }
                            }
                            KeyCode::Backspace => {
                                buffer.pop();
                            }
                            KeyCode::Esc => prompt = None,
                            KeyCode::Enter => {
                                let target = buffer.parse::<usize>().ok();
                                prompt = None;

                                if let Some(target) = target {
                                    let target = limit.map_or(target, |limit| target.min(limit));
                                    finished = fast_forward(
                                        &mut out,
                                        &mut painter,
                                        grid,
                                        bytes,
                                        &mut generation,
                                        &mut watcher,
                                        target,
                                    )?;
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match action_for(key.code, key.modifiers) {
                        Action::Quit => return Ok(finished.unwrap_or(Ending::Quit)),
                        Action::TogglePause => paused = !paused,
                        Action::GoTo => prompt = Some(String::new()),
                        Action::Restart => {
                            let (width, height) = (grid.width(), grid.height());
                            *grid = Grid::from_bytes(bytes, width, height);
                            watcher = Watcher::new();
                            generation = 0;
                            finished = None;
                            paused = false;
                        }
                        Action::Ignore => {}
                    }
                }
                _ => {}
            }
        }

        if idle {
            continue;
        }

        if limit.is_some_and(|limit| generation >= limit) {
            finished = Some(Ending::Limit);
            continue;
        }

        grid.step();
        generation += 1;
        finished = watcher.verdict(grid);
        dirty = true;
    }
}

#[derive(Default)]
struct Painter {
    painted: Vec<String>,
    size: (usize, usize),
}

impl Painter {
    fn draw(
        &mut self,
        out: &mut impl Write,
        grid: &Grid,
        generation: usize,
        status: &str,
        hint: &str,
    ) -> io::Result<()> {
        let (columns, rows) = terminal::size()?;
        let columns = columns as usize;
        let view_rows = rows.saturating_sub(1) as usize;

        if self.size != (columns, view_rows) {
            self.size = (columns, view_rows);
            self.painted = vec![String::new(); view_rows];
            queue!(out, terminal::Clear(terminal::ClearType::All))?;
        }

        queue!(out, SetForegroundColor(Color::White))?;

        for y in 0..view_rows {
            let mut line = String::with_capacity(columns);
            if y < grid.height() {
                for x in 0..columns.min(grid.width()) {
                    line.push(if grid.get(x, y) { game::LIVE_CELL } else { ' ' });
                }
            }
            for _ in line.chars().count()..columns {
                line.push(' ');
            }

            if self.painted[y] == line {
                continue;
            }
            queue!(out, cursor::MoveTo(0, y as u16), Print(&line))?;
            self.painted[y] = line;
        }

        let left = format!(" gen {generation}   pop {}   {status}", grid.population());
        let mut bar = match columns.checked_sub(left.chars().count() + hint.chars().count() + 2) {
            Some(gap) => format!("{left}{:gap$}  {hint}", ""),
            None => left,
        };
        bar.truncate(
            bar.char_indices()
                .nth(columns)
                .map_or(bar.len(), |(index, _)| index),
        );

        queue!(
            out,
            cursor::MoveTo(0, view_rows as u16),
            SetForegroundColor(Color::DarkGrey),
            Print(bar),
            ResetColor
        )?;

        out.flush()
    }
}
