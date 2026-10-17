//! Reading and writing the Weiss Schwarz Simulator's install folder.
//!
//! ```text
//! <root>/CardContent/Cards/<serie>/SingleSetData.txt   series and their sets   series_file.rs
//! <root>/CardContent/Cards/<set folder>/CardData.txt   the cards of a set      card_file.rs
//! <root>/CardContent/Cards/<set folder>/*.jpg          the cards' artwork      card_images.rs
//! <root>/<Game>_Data/StreamingAssets/CommonEffects…    shared effect texts     common_effects.rs
//! <root>/Decks/<deck>.txt                              decks                   deck_files.rs
//! <root>/Sleeves/*.jpg|png                             sleeve images           sleeves.rs
//! ```
//!
//! [`folder::SimulatorFolder`] knows where each of these lives; [`load_series`] reads all the
//! cards at once.

pub mod card_file;
pub mod card_images;
pub mod common_effects;
pub mod deck_files;
pub mod folder;
pub mod series_file;
pub mod sleeves;

use crate::model::Serie;
use folder::SimulatorFolder;

/// Reads every card of a simulator install: the common effects first (cards refer to them),
/// then every serie with its sets and cards. Takes a few seconds on a full install: run it off
/// the UI thread.
pub fn load_series(folder: &SimulatorFolder) -> std::io::Result<Vec<Serie>> {
    let effects = folder
        .common_effects()
        .map(|path| common_effects::load_common_effects(&path))
        .unwrap_or_default();
    series_file::load_series(&folder.cards(), &folder.alternate_artwork(), &effects)
}
