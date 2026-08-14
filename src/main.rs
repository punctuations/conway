mod game;
mod view;

use clap::builder::styling::{ AnsiColor, Effects, Styles };
use clap::{ ArgAction, Args, CommandFactory, Parser, Subcommand };
use game::Grid;
use std::fs;
use std::io::{ self, IsTerminal, Read, Write };
use std::path::{ Path, PathBuf };
use std::process::ExitCode;
use std::time::Duration;
use view::{ Ending, Watcher };

const RESET: &str = "\u{1b}[0m";
const BOLD: &str = "\u{1b}[1m";
const DIM: &str = "\u{1b}[90m";
const HEAD: &str = "\u{1b}[1;32m";
const NAME: &str = "\u{1b}[36m";
const LIVE: &str = "\u{1b}[97m";

const GLIDER: [[bool; 3]; 3] = [
    [false, true, false],
    [false, false, true],
    [true, true, true],
];

const BLOCK_W: usize = 4;
const BLOCK_H: usize = 2;
const DESC_COL: usize = 33;

fn art_lines() -> Vec<String> {
    let mut lines = Vec::new();

    for row in GLIDER.iter() {
        for _ in 0..BLOCK_H {
            let mut line = String::new();
            for alive in row.iter() {
                let cell = if *alive { game::LIVE_CELL } else { ' ' };
                for _ in 0..BLOCK_W {
                    line.push(cell);
                }
            }
            lines.push(line);
        }
    }

    lines
}

fn colorize(line: &str) -> String {
    let mut out = String::new();
    let mut live = false;

    for cell in line.chars() {
        let alive = cell == game::LIVE_CELL;
        if alive != live {
            out.push_str(if alive { LIVE } else { RESET });
            live = alive;
        }
        out.push(cell);
    }

    if live {
        out.push_str(RESET);
    }
    out
}

fn entry(name: &str, description: &str) -> String {
    let pad = DESC_COL.saturating_sub(2 + name.chars().count()).max(1);
    format!("  {NAME}{name}{RESET}{:pad$}{description}", "")
}

fn help_text() -> String {
    let side = [
        format!("{BOLD}conway v{}{RESET}", env!("CARGO_PKG_VERSION")),
        "Turn any file into Conway's Game of Life".to_string(),
        String::new(),
        format!("{HEAD}usage:{RESET}"),
        format!("  {NAME}conway{RESET} <file> [options]"),
        format!("  cat <file> | {NAME}conway{RESET}"),
    ];

    let art = art_lines();
    let top = art.len().saturating_sub(side.len()) / 2;
    let blank = " ".repeat(3 * BLOCK_W);

    let mut out = String::new();
    for index in 0..art.len().max(top + side.len()) {
        let line = art.get(index).map(String::as_str).unwrap_or(&blank);
        let text = index.checked_sub(top).and_then(|i| side.get(i));
        match text {
            Some(text) if !text.is_empty() => {
                out.push_str(&format!("{}   {text}\n", colorize(line)));
            }
            _ => {
                out.push_str(&colorize(line.trim_end()));
                out.push('\n');
            }
        }
    }

    out.push_str(&format!("\n{HEAD}commands:{RESET}\n"));
    out.push_str(&entry("run [file]", "seed a grid from a file and run it"));
    out.push('\n');
    out.push_str(&entry("seed [file]", "print the starting grid, don't simulate"));
    out.push('\n');
    out.push_str(&entry("help [command]", "this message"));
    out.push('\n');

    out.push_str(&format!("\n{HEAD}options:{RESET}\n"));
    out.push_str(
        &entry("-n, --generations <N>", "stop after N generations (default: until the board dies)")
    );
    out.push('\n');
    out.push_str(&entry("-d, --delay <MS>", "milliseconds between generations (default 120)"));
    out.push('\n');
    out.push_str(&entry("-w, --width <COLS>", "grid width in cells (default: terminal width)"));
    out.push('\n');
    out.push_str(&entry("-H, --height <ROWS>", "grid height in cells (default: terminal height)"));
    out.push('\n');
    out.push_str(&entry("--raw", "treat piped input as data, even if it looks like a path"));
    out.push('\n');

    out.push_str(
        &format!(
            "\n{DIM} need help with a specific command? run{RESET} {NAME}conway help <command>{RESET}\n"
        )
    );
    out
}

const SUB_HELP: &str =
    "\
{about}

usage:
  {usage}

arguments:
{positionals}

options:
{options}

 back to the command list? run `conway help`
";

#[derive(Parser)]
#[command(
    name = "conway",
    version,
    about = "Turn any file into Conway's Game of Life",
    override_help = help_text(),
    styles = help_styles(),
    disable_help_subcommand = true,
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[arg(value_name = "FILE", help = "file to seed from, or - for stdin")]
    file: Option<PathBuf>,

    #[command(flatten)]
    grid: GridArgs,
}

#[derive(Args, Clone)]
struct HelpFlag {
    #[arg(short = 'h', long = "help", action = ArgAction::Help, help = "this message")]
    help: Option<bool>,
}

#[derive(Args, Clone)]
struct GridArgs {
    #[arg(
        short = 'n',
        long,
        value_name = "N",
        help = "stop after N generations (default: run until the board dies)"
    )]
    generations: Option<usize>,

    #[arg(
        short = 'd',
        long,
        value_name = "MS",
        default_value_t = 120,
        help = "milliseconds between generations"
    )]
    delay: u64,

    #[arg(
        short = 'w',
        long,
        value_name = "COLS",
        help = "grid width in cells (default: terminal width)"
    )]
    width: Option<usize>,

    #[arg(
        short = 'H',
        long,
        value_name = "ROWS",
        help = "grid height in cells (default: terminal height)"
    )]
    height: Option<usize>,

    #[arg(long, help = "treat piped input as data, even if it looks like a path")]
    raw: bool,
}

#[derive(Subcommand)]
enum Command {
    #[command(
        about = "seed a grid from a file and run it",
        help_template = SUB_HELP,
        disable_help_flag = true
    )] Run {
        #[arg(value_name = "FILE", help = "file to seed from, or - for stdin")]
        file: Option<PathBuf>,

        #[command(flatten)]
        grid: GridArgs,

        #[command(flatten)]
        help_flag: HelpFlag,
    },

    #[command(
        about = "print the starting grid, don't simulate",
        help_template = SUB_HELP,
        disable_help_flag = true
    )] Seed {
        #[arg(value_name = "FILE", help = "file to seed from, or - for stdin")]
        file: Option<PathBuf>,

        #[command(flatten)]
        grid: GridArgs,

        #[command(flatten)]
        help_flag: HelpFlag,
    },

    #[command(about = "this message", help_template = SUB_HELP, disable_help_flag = true)] Help {
        #[arg(value_name = "COMMAND", help = "command to explain")]
        command: Option<String>,

        #[command(flatten)]
        help_flag: HelpFlag,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match (cli.command, cli.file) {
        (Some(Command::Help { command, .. }), _) => print_help(command.as_deref()),
        (None, None) if !io::stdin().is_terminal() => simulate(None, &cli.grid),
        (None, None) => print_help(None),
        (Some(Command::Run { file, grid, .. }), _) => simulate(file.as_deref(), &grid),
        (Some(Command::Seed { file, grid, .. }), _) => seed(file.as_deref(), &grid),
        (None, Some(file)) => simulate(Some(&file), &cli.grid),
    }
}

fn help_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Green.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Cyan.on_default())
        .placeholder(AnsiColor::Cyan.on_default())
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .invalid(AnsiColor::Yellow.on_default())
}

fn print_help(topic: Option<&str>) -> ExitCode {
    let mut cli = Cli::command();
    cli.build();

    let Some(topic) = topic else {
        return finish(cli.print_help());
    };

    let Some(sub) = cli.get_subcommands_mut().find(|sub| sub.get_name() == topic) else {
        eprintln!("conway: no such command: {topic}");
        eprintln!(" run `conway help` for the command list");
        return ExitCode::FAILURE;
    };

    finish(sub.print_help())
}

fn reads_stdin(path: Option<&Path>) -> bool {
    path.is_none_or(|path| path.as_os_str() == "-")
}

const MAX_PATH_INPUT: usize = 4096;

fn path_from(buffer: &[u8]) -> Option<PathBuf> {
    if buffer.len() > MAX_PATH_INPUT {
        return None;
    }

    let text = std::str::from_utf8(buffer).ok()?.trim();
    if text.is_empty() || text.contains('\n') {
        return None;
    }

    let path = PathBuf::from(text);
    path.is_file().then_some(path)
}

fn read_seed(path: Option<&Path>, raw: bool) -> Result<Vec<u8>, ExitCode> {
    let (label, bytes) = if reads_stdin(path) {
        if io::stdin().is_terminal() {
            eprintln!("conway: no input file, and stdin is a terminal");
            eprintln!(" run `conway help` for usage");
            return Err(ExitCode::FAILURE);
        }

        let mut buffer = Vec::new();
        if let Err(error) = io::stdin().lock().read_to_end(&mut buffer) {
            eprintln!("conway: stdin: {error}");
            return Err(ExitCode::FAILURE);
        }

        match path_from(&buffer).filter(|_| !raw) {
            Some(path) =>
                match fs::read(&path) {
                    Ok(bytes) => (path.display().to_string(), bytes),
                    Err(error) => {
                        eprintln!("conway: {}: {}", path.display(), error);
                        return Err(ExitCode::FAILURE);
                    }
                }
            None => ("stdin".to_string(), buffer),
        }
    } else {
        let path = path.unwrap_or(Path::new("-"));
        match fs::read(path) {
            Ok(bytes) => (path.display().to_string(), bytes),
            Err(error) => {
                eprintln!("conway: {}: {}", path.display(), error);
                return Err(ExitCode::FAILURE);
            }
        }
    };

    if bytes.is_empty() {
        eprintln!("conway: {label}: no data to seed from");
        return Err(ExitCode::FAILURE);
    }

    Ok(bytes)
}

fn finish(result: io::Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("conway: {error}");
            ExitCode::FAILURE
        }
    }
}

fn seed(path: Option<&Path>, args: &GridArgs) -> ExitCode {
    let bytes = match read_seed(path, args.raw) {
        Ok(bytes) => bytes,
        Err(code) => {
            return code;
        }
    };

    let (width, height) = game::dimensions_for(bytes.len(), args.width, args.height);
    let grid = Grid::from_bytes(&bytes, width, height);

    let stdout = io::stdout();
    let mut out = stdout.lock();

    finish(
        (|| {
            writeln!(out, "{} bytes -> {}x{}", bytes.len(), grid.width(), grid.height())?;
            writeln!(out, "\ngeneration 0 ({} alive)\n{grid}", grid.population())
        })()
    )
}

fn simulate(path: Option<&Path>, args: &GridArgs) -> ExitCode {
    let bytes = match read_seed(path, args.raw) {
        Ok(bytes) => bytes,
        Err(code) => {
            return code;
        }
    };

    let interactive = io::stdout().is_terminal();
    let (width, height) = if interactive {
        let (columns, rows) = view::viewport();
        (args.width.unwrap_or(columns), args.height.unwrap_or(rows))
    } else {
        game::dimensions_for(bytes.len(), args.width, args.height)
    };

    let mut grid = Grid::from_bytes(&bytes, width, height);

    if interactive {
        let delay = Duration::from_millis(args.delay);
        return match view::run(&mut grid, &bytes, args.generations, delay) {
            Ok(_) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("conway: {error}");
                ExitCode::FAILURE
            }
        };
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();

    finish(
        (|| {
            writeln!(out, "{} bytes -> {}x{}", bytes.len(), grid.width(), grid.height())?;
            writeln!(out, "\ngeneration 0 ({} alive)\n{grid}", grid.population())?;

            let mut watcher = Watcher::new();
            let mut generation = 0usize;

            let ending = loop {
                if args.generations.is_some_and(|limit| generation >= limit) {
                    break Ending::Limit;
                }

                grid.step();
                generation += 1;
                writeln!(out, "\ngeneration {generation} ({} alive)\n{grid}", grid.population())?;

                if let Some(ending) = watcher.verdict(&grid) {
                    break ending;
                }
            };

            writeln!(out, "\nstopped after {generation} generations: {}", ending.describe())
        })()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(name: &str) -> String {
        format!("{}/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn a_lone_existing_path_is_followed() {
        let text = manifest("Cargo.toml");
        assert_eq!(path_from(text.as_bytes()), Some(PathBuf::from(&text)));
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let text = format!("  {}\n", manifest("Cargo.toml"));
        assert_eq!(path_from(text.as_bytes()), Some(PathBuf::from(manifest("Cargo.toml"))));
    }

    #[test]
    fn multiple_lines_stay_data() {
        let text = format!("{}\n{}", manifest("Cargo.toml"), manifest("Cargo.lock"));
        assert_eq!(path_from(text.as_bytes()), None);
    }

    #[test]
    fn directories_are_not_followed() {
        assert_eq!(path_from(env!("CARGO_MANIFEST_DIR").as_bytes()), None);
    }

    #[test]
    fn missing_paths_stay_data() {
        assert_eq!(path_from(b"/definitely/not/here.bin"), None);
    }

    #[test]
    fn empty_and_binary_input_stays_data() {
        assert_eq!(path_from(b""), None);
        assert_eq!(path_from(b"   \n"), None);
        assert_eq!(path_from(&[0xff, 0xfe, 0x00, 0x01]), None);
    }

    #[test]
    fn oversized_input_stays_data() {
        let text = manifest("Cargo.toml");
        let padded = format!("{text}{}", " ".repeat(MAX_PATH_INPUT));
        assert_eq!(path_from(padded.as_bytes()), None);
    }
}
