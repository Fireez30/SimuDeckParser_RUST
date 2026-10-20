//! SimuDeckParser's library: everything that is not drawing the window.
//!
//! The crate is split in two halves:
//!
//! - this library (`src/*.rs`, `src/simulator/`, `src/encoredecks/`, `src/pdf/`): reading the
//!   simulator's files, deck rules, Encore Decks, PDFs. No UI code, and every piece has tests.
//! - the app (`src/main.rs`, `src/app/`): the egui window, which calls into this library.
//!
//! Where to look:
//!
//! | Module           | What it does                                                          |
//! |------------------|-----------------------------------------------------------------------|
//! | [`model`]        | The data types: [`model::Card`], [`model::Serie`], [`model::Deck`]…   |
//! | [`simulator`]    | Reading and writing the simulator's install folder (cards, decks…)   |
//! | [`card_codes`]   | The many spellings of a card code (`-E001`, `-001SP`, lowercase…)     |
//! | [`card_index`]   | Finding loaded cards by code or by name                               |
//! | [`filters`]      | The card list's filters and sort orders                               |
//! | [`deck_builder`] | A deck being built, and the deck rules (50 cards, 4 copies…)          |
//! | [`damage_sim`]   | The damage calculator: simulating a turn of damage                    |
//! | [`encoredecks`]  | encoredecks.com: import, upload, account sign-in                     |
//! | [`pdf`]          | Printable proxies and translation sheets                              |
//! | [`settings`]     | The small `key:value` settings file                                   |
//! | [`text`]         | Text helpers: file decoding, the card text's markup, quoted names     |
//! | [`fonts`]        | The fonts embedded in the program                                     |
//!
//! See `documentation/ARCHITECTURE.md` for a guided tour.

pub mod card_codes;
pub mod card_index;
pub mod damage_sim;
pub mod deck_builder;
pub mod encoredecks;
pub mod filters;
pub mod fonts;
pub mod model;
pub mod pdf;
pub mod settings;
pub mod simulator;
pub mod text;
