# 👾 conway

Conway is a terminal implementation of
[Conway's Game of Life](https://en.wikipedia.org/wiki/Conway's_Game_of_Life)
("GoL") that uses the contents of arbitrary files as the initial state.

`conway README.md`

Each byte is expanded into eight bits, with each bit corresponding to a cell in
the initial grid. The simulation then evolves according to the standard GoL
rules.

`file -> bytes -> bits -> cell -> generations`

### Installation

Install directly from the repo

```
cargo install --git https://github.com/punctuations/conway
```

Or build locally

```
git clone https://github.com/punctuations/conway
cd conway
cargo build --release

# Install to PATH
cargo install --path .
```

### Usage

`conway <file>`

Any file can be used as input

- `conway README.md`
- `conway image.png`
- `conway /usr/bin/ls`
- `which ls | conway`
- `cat "Hello world!" | conway`

The initial state is deterministic.

### How it works

The input file is read as raw byte stream and converted to a bit stream.

For example: `0xA6 -> 10100110`

The bit stream is mapped row-major onto the simulation grid. Then grid is then
updated using Conway's GoL rules.

The terminal size determines the visible portion of the grid.
