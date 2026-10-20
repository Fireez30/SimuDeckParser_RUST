# How SimuDeckParser is built

A guided tour of the code, for developers who know how to program but not necessarily Rust.
Read it top to bottom once; afterwards, the "Where is…?" table is the quickest way in.

The tool was first written in C++ with Qt, then ported to Rust. A few comments mention "the
original tool": they explain why some behaviour (such as the order card code spellings are
tried in) was kept exactly as it was.

## The big picture

The program reads the files of a Weiss Schwarz Simulator install (cards, decks, sleeves) and
shows them in a window. It can also build and edit decks, import and publish decks on
[encoredecks.com](https://www.encoredecks.com), and print decks as PDFs.

The code is split in two halves:

| Half        | Files                                                   | Contains                         |
|-------------|---------------------------------------------------------|----------------------------------|
| The library | `src/lib.rs`, `src/*.rs`, `src/simulator/`, `src/encoredecks/`, `src/pdf/` | Everything that is not drawing: file formats, deck rules, network, PDFs. Unit-tested. |
| The app     | `src/main.rs`, `src/app/`                               | The window, built with [egui](https://github.com/emilk/egui). |

The app calls the library; the library never calls the app. When you change *what the program
does* (a file format, a matching rule), it is almost always in the library; when you change
*how it looks or reacts*, it is in `src/app/`.

## Where is…?

```text
src/
├── lib.rs                  The library's table of contents.
├── model.rs                The data types: Card, Set, Serie, Deck, Color, CardType, Trigger.
├── text.rs                 Decoding text files, the card text markup (<b>, <sprite…>), quoted names.
├── card_codes.rs           The many spellings of a card code (KS/W49-001 vs KS/W49-E001…).
├── card_index.rs           Finding loaded cards by code (CardIndex) or by name (NameIndex).
├── filters.rs              The card list's filters, and the two sort orders.
├── deck_builder.rs         A deck being built (DeckList) and the deck rules (50 cards, 4 copies…).
├── damage_sim.rs           The damage calculator: playing a turn of attacks many times.
├── settings.rs             The settings file, and every key it holds.
├── fonts.rs                The fonts compiled into the program.
├── simulator/              The simulator's install folder:
│   ├── folder.rs             where each kind of file lives; finding installs on this computer
│   ├── series_file.rs        SingleSetData.txt: the series and their sets
│   ├── card_file.rs          CardData.txt: the cards of a set
│   ├── common_effects.rs     effect texts shared by many cards
│   ├── card_images.rs        finding a card's picture
│   ├── deck_files.rs         reading and writing decks
│   └── sleeves.rs            sleeve images, and adding new ones
├── encoredecks/            encoredecks.com:
│   ├── decks.rs              reading decks, importing them into the simulator
│   ├── upload.rs             publishing a deck
│   ├── account.rs            signing in, checking a session
│   └── credentials.rs        keeping the session in the system's credential store
├── pdf/                    Printable PDFs: mod.rs lays out proxies and translation sheets,
│                           document.rs / font.rs / artwork.rs do the drawing.
├── main.rs                 Opens the window.
└── app/                    The window:
    ├── mod.rs                App: the state of the whole window, drawing a frame, navigation.
    ├── background.rs         Task: running slow work on another thread.
    ├── setup_screen.rs       First screen: picking the simulator folder, loading the cards.
    ├── card_browser.rs       The Cards screen (also used inside the deck editor).
    ├── card_view/            Showing one card: summary, effect text, detail pane, related cards.
    ├── decks_screen/         The Decks screen, and its Import / Upload / PDF dialogs.
    ├── deck_editor.rs        Building or editing a deck.
    ├── deck_preview.rs       An open deck's stats and card groups (Decks and Encore screens).
    ├── sleeve_picker.rs      The sleeve picker popup.
    ├── encore_screen.rs      The Encore Deck screen: the linked account's decks.
    ├── damage_screen.rs      The Damage screen: setting up a turn and simulating its damage.
    ├── settings_screen.rs    The Settings screen.
    ├── account.rs            Linking the Encore Decks account (Settings).
    ├── theme/                Colours, fonts and the Material 3 components (buttons, chips…).
    ├── assets.rs             The icons compiled into the program.
    └── screenshots.rs        A test that clicks through every screen and saves screenshots.
tests/simulator.rs          End-to-end test on a fake simulator folder.
```

## How the data flows

**Startup.** `App::with_settings` (`app/mod.rs`) reads the settings. If a simulator folder is
saved, it starts loading it; otherwise the setup screen asks for one, and looks for installs on
the computer in the background (`simulator::folder::detect_installs`).

**Loading the cards.** `simulator::load_series` reads every `SingleSetData.txt` and
`CardData.txt` of the install (series are parsed in parallel, one thread per CPU core) and
returns a `Vec<Serie>`: every card, grouped by serie and set. The app keeps it as
`Arc<Vec<Serie>>`, shared by all screens (see "Ownership" below for `Arc`).

**Opening a deck.** A deck file only holds card codes (`deck_files::DeckFile`). To show it, the
codes are matched to loaded cards with `CardIndex::resolve_deck`, which tries the other
spellings of each code (`card_codes.rs`) and returns a `model::Deck`: its cards, plus the codes
that matched nothing (`Deck::unmatched`). `deck_preview::Summary` then counts types, levels,
colours and triggers for the stat tiles.

**Importing from Encore Decks.** `encoredecks::decks::import` downloads the deck's JSON, matches
its codes the same way, and saves it with `deck_files::save_new_deck`, which never overwrites a
file (a taken name gets a ` (2)` suffix).

**Uploading.** `encoredecks::upload::upload` looks every card up on the site (it stores decks as
its own card ids), and publishes nothing unless all 50 cards were found.

**Simulating damage.** The Damage screen builds a `damage_sim::Turn` (the soul of the three
characters on stage, the sources of damage in order, the trigger cards left in the player's deck,
and the levels of the deck minus the stage and the climax for Musashi effects) and an `Opponent`
(their deck, waiting room, stock, level and clock), then runs `damage_sim::simulate` in a
background task. Nothing is read from the cards' texts: the user enters the soul and the damage.
The result is a histogram of the total damage, from which the screen shows the average, the
chance to kill and the chance of each total.

**Printing.** `pdf::proxies` and `pdf::translation` build the PDF bytes; the export dialog
writes them to the Downloads folder.

## How the window works

### Immediate mode

egui is an *immediate mode* UI library. There are no button objects that you create once and
attach callbacks to. Instead, the whole window is drawn again on every frame, by
`App::ui` (`app/mod.rs`), from the app's state:

```rust
if theme::filled(None, "Save").show(ui).clicked() {
    self.save();
}
```

`show(ui)` draws the button *for this frame* and returns a `Response` that says whether it was
clicked. So each screen is a plain struct holding its state (the open deck, the search text…)
with a `show(&mut self, ui, …)` method that draws it and reacts to clicks. Text fields are bound
to a `String` of the state, which egui edits in place.

### Acting after drawing

You will often see this pattern:

```rust
let mut clicked = None;
theme::list_pane(ui, "deck_list", |ui| clicked = self.deck_list(ui));
if let Some(path) = clicked {
    self.pick(&path);
}
```

The click is noted in a variable, and acted on after the drawing code. This is because of
Rust's borrowing rules (see below): while the closure `|ui| …` uses part of `self`, the rest of
the code cannot modify `self` at the same time. Collecting first and acting after avoids the
conflict, and keeps drawing and state changes apart.

### Slow work

Anything slow (the network, reading all the cards, writing a PDF) must not run inside `ui`,
or the window would freeze. It runs on a background thread with `background::Task`:

```rust
// Start: in a field of the screen.
self.importing = Some(Task::spawn(ctx, move || import(&link, &series, &dir)));

// Every frame: once it is done, `finished` empties the field and returns the result.
if let Some(result) = finished(&mut self.importing) {
    match result {
        Ok(deck) => …,
        Err(e) => …,
    }
}
```

While the field holds a task, screens show a spinner and disable the buttons that would start
another one.

### Telling the user

Screens do not show error dialogs themselves: they put a `Notice` (`app/mod.rs`) in their
`notice` field, and the app picks it up at the end of the frame. `Notice::Info` becomes a
snackbar at the bottom of the window; `Notice::Error` an error dialog. An error caused by an
expired Encore Decks session also unlinks the account.

### Splitting `App` across files

The `App` struct is defined in `app/mod.rs`, but some of its methods are in other files
(`setup_screen.rs`, `settings_screen.rs`, `account.rs`): Rust allows an `impl App { … }` block
in any file of the module, much like partial classes in C#. The Decks screen does the same with
its dialogs.

## Rust in ten minutes, for this code base

Only what you need to read this project; [the Rust book](https://doc.rust-lang.org/book/) has
the rest.

**Variables** are immutable unless declared `let mut`. Function parameters too: `&mut self`
means the method may modify the struct, `&self` that it only reads it.

**`Option<T>`** is a value that may be missing: `Some(value)` or `None`. Rust has no `null`.
`Option<Account>` is "maybe an account".

**`Result<T>`** is the outcome of something that can fail: `Ok(value)` or `Err(error)`. Errors
are values, not exceptions. The `?` operator means "if this is an error, return it to my
caller now":

```rust
let bytes = std::fs::read(path)?; // returns the error if the file can't be read
```

This project uses the [`anyhow`](https://docs.rs/anyhow) library for errors:
`anyhow::Result<T>` can hold any error, `.context("reading the deck")` adds an explanation, and
`format!("{e:#}")` prints the error with its explanations ("reading the deck: file not found").

**`match`** is a `switch` that must handle every case; **`if let`** handles one case:

```rust
match result {
    Ok(deck) => show(deck),
    Err(e) => report(e),
}
if let Some(account) = &self.account { … } // runs only when there is an account
let Some(deck) = picked else { return };   // "let-else": stop here when there is none
```

**Enums** can carry data in each variant: `Notice::Error { title, message, … }`,
`Rich::Text(text, style)`. They are the Rust way to say "one of these shapes".

**Structs and `impl`**: a struct holds data, an `impl` block adds its methods. `Self` is the
type being implemented. `#[derive(Debug, Clone, Default)]` above a struct generates common code
automatically (printing, copying, an empty value).

**Traits** are interfaces. `card_browser::Picker` is one: the deck editor implements it, and
the card browser calls it. `&mut dyn Picker` is "a reference to any type that implements
`Picker`"; `impl FnOnce(&mut Ui)` as a parameter type means "any function or closure with this
signature".

**Closures** are anonymous functions: `|card| card.level` takes `card` and returns its level.
`move || …` makes the closure take ownership of what it uses, which is required to send it to
another thread.

**Iterator chains** replace most loops over collections:

```rust
let names: Vec<String> = decks
    .iter()                     // go over the decks
    .filter(|d| !d.ai)          // keep the user's decks
    .map(|d| d.name.clone())    // take their names
    .collect();                 // gather them in a Vec
```

**Ownership and borrowing.** Every value has one owner, and is freed when its owner goes away.
Other code *borrows* it with a reference: `&value` to read, `&mut value` to modify. The rule
the compiler enforces is: many readers, or one writer, never both at once. That is why
`.clone()` appears where two places need their own copy, and why the UI code collects clicks
first and acts after drawing. `Arc<T>` is a shared, reference-counted pointer: it lets several
owners (the app and its background threads) use the same cards without copying them.

**Lifetimes** (`'a`, `'o`) are names for "how long a borrowed reference is valid". You can
mostly ignore them when reading: `CardIndex<'a>` just means the index borrows the cards and
must not outlive them.

**Modules and visibility.** Each file is a module; `mod name;` declares a child module, `use`
imports names. Items are private to their module unless marked `pub` (public), `pub(super)`
(visible to the parent module), or `pub(crate)` / `pub(in crate::app)` (visible in that part of
the program).

**Macros** end with `!`: `format!("{name} has {n} cards")` builds a string (like f-strings),
`include_bytes!("file")` embeds a file in the program at build time.

**Tests** live next to the code, in a `#[cfg(test)] mod tests { … }` block at the end of each
file; each `#[test]` function is one test. `#[ignore]` marks the slow ones (network, real
install, screenshots), which only run when asked.

## Building, running, testing

```sh
cargo run                        # build and open the window
cargo test                       # every fast test
cargo test -- --ignored          # also the network, real-install and screenshot tests
cargo clippy --all-targets       # the linter: should print no warning
cargo fmt                        # format the code
```

Useful variables for the ignored tests:

- `SIMULATOR_ROOT=/path/to/simulator` runs the real-install tests (read-only) and screenshots
  on a real install instead of a fake one;
- `SCREENSHOT_DIR=/some/dir` is where the screenshot test saves its pictures
  (`target/screenshots` by default). It renders every screen in both themes: run it before and
  after a change to compare.

## Common changes

- **A new setting:** add its key in `settings.rs`, read it in `App::with_settings`
  (`app/mod.rs`), write it with `App::save_setting`.
- **A button on the open deck:** add it in `DecksScreen::deck_page`
  (`app/decks_screen/mod.rs`); if it opens something, add a `DeckAction` variant and handle it
  after drawing, in `DecksScreen::show`.
- **A card code that does not match:** the spellings tried are in `card_codes.rs`
  (`key_variants` for the simulator, `encore_spellings` for Encore Decks); add a case to their
  tests first.
- **A card file line that is not read:** `simulator/card_file.rs`, function `parse_pass`.
- **A colour or a size:** `app/theme/`. Screens use colour roles (`scheme(ui).primary`), never
  raw colours.

## Weiss Schwarz vocabulary

- **Serie**: a franchise (KonoSuba, Hololive…). Spelled "serie" in the code, as the original
  tool did.
- **Set**: a product of a serie (a booster box, a trial deck), with a code prefix (`KS/W49`).
- **Card code**: `KS/W49-E001`. English cards add an `E` after the dash; foils add letters at
  the end (`-001SP`).
- **Character, Event, Climax**: the card types. A deck has exactly 50 cards, at most 8 climaxes,
  and at most 4 cards with the same name.
- **Level, cost, power, soul**: a card's stats. **Traits** are tags such as `<Magic>`.
- **Trigger**: the icon(s) on a card that act when it is revealed (soul, book, door…).
- **Climax combo (CX combo)**: an effect that needs a specific climax.
- **Sleeve**: the card-back image the simulator draws a deck with.
- **Encore Decks**: encoredecks.com, a site to share decks.
