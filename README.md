# crosstune-generator

Generate dense crossword layouts from song metadata and lyric snippets.

## Build

```sh
cargo build --release
```

Run with the default `input.yaml`:

```sh
cargo run --release
```

Run with a specific input file:

```sh
cargo run --release -- --input path/to/input.yaml
```

Useful options:

```sh
cargo run --release -- --input input.yaml --time-limit-ms 5000 --seed 1 --branch-limit 24 --top-n 1
```

## Input Schema

The input file is YAML:

```yaml
width: 12
height: 10
max_tokens: 3
tracks:
  - name: The Adults Are Talking
    artist: The Strokes
    album: The New Abnormal
    snippets:
      - I know you think of me when you think of her
    custom:
      - answer: Adults
        clue: Second word of this song title
```

Fields:

- `width`: crossword grid width, required, positive integer.
- `height`: crossword grid height, required, positive integer.
- `max_tokens`: maximum number of word tokens to extract from titles, artists, albums, and snippets. Optional; defaults to `3`.
- `tracks`: required list of tracks.
- `tracks[].name`: song title, required.
- `tracks[].artist`: song artist, required.
- `tracks[].album`: album title, optional; use `null` or omit it when unknown.
- `tracks[].snippets`: optional list of lyric snippets.
- `tracks[].custom`: optional list of explicit clue/answer pairs.
- `tracks[].custom[].answer`: custom answer text.
- `tracks[].custom[].clue`: clue text for the custom answer.

Answers are normalized to uppercase letters for the grid.
