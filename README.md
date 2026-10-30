# Demonstrator skeleton

The project we continue in during the Demonstrator session (30.10.).
It already connects to the relay: `cargo run` prints `connecting to …` and
then `session …`. In the session we go on in `main`.

| Path | What |
|---|---|
| `Cargo.toml`, `Cargo.lock` | The project and its crates: tungstenite, rustls (ring, TLS 1.2 and 1.3), serde, serde_json, qrcode |
| `TOKEN.md` | Where to get your relay token, and how to set it |
| `src/main.rs` | `connect()` connects to the relay with your token and says hello; we go on in `main` |
| `src/protocol.rs` | The relay's messages as Rust types (serde) |
| `extra/quiz.rs`, `extra/questions.json` | The quiz from Rust 2, for the second part |
| `catch-up/step-07-main.rs` … `step-10-main.rs`, `step-13-main.rs` | The complete `src/main.rs` after that step of the live coding (steps 7 to 10 for "Print A", step 13 is the final quiz version) |

Your program needs two environment variables: `INPUT_MUX_URL` (the relay,
`https://…`) and `INPUT_MUX_TOKEN` (your personal token). `TOKEN.md` says
where to get the token and how to set both.

## Fallen behind?

Copy the file of the step we are at over your `src/main.rs`, then go on
from there:

```sh
cp catch-up/step-NN-main.rs src/main.rs
```

On Windows: `copy catch-up\step-NN-main.rs src\main.rs`. PowerShell overwrites
without asking; cmd.exe asks "Overwrite src\main.rs?" (answer Y).

## Second part: the quiz with the phone

Before the quiz part, copy the quiz module and the questions into place:

```sh
cp extra/quiz.rs src/ && cp extra/questions.json .
```

On Windows: `copy extra\quiz.rs src\` and `copy extra\questions.json .`
(cmd.exe asks before overwriting: answer Y).

## QR code does not scan?

In a dark terminal the QR code comes out inverted, and some phone cameras
do not read that. Switch to a light terminal profile, or type the printed
link on the phone. Or change the code before `.build()`:
`.dark_color(Dense1x2::Light).light_color(Dense1x2::Dark)`.

On the phone, tap the screen once ("Tap to start"); only then does your
program print `p1 connected`.

## Other problems

- `could not find Cargo.toml`: your terminal is in the wrong folder. `cd`
  into the folder that contains `Cargo.toml` (not its parent).
- The build fails with `os error 5`, `225` or `4551`, or the program
  reports `invalid peer certificate`: your antivirus interferes (blocking
  build scripts or scanning HTTPS). Tell us.

## Git

`.gitignore` keeps `target/` out of git; keep it when you put the project
into your group's repository.
