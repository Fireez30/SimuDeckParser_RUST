pub mod cards;
pub mod effects;
pub mod series;

use crate::layout::Layout;
use crate::model::Serie;

/// Parses a simulator install: common effects, then every serie and its cards.
pub fn load_simulator(layout: &Layout) -> std::io::Result<Vec<Serie>> {
    let effects = layout
        .common_effects()
        .map(|p| effects::load_common_effects(&p))
        .unwrap_or_default();
    series::load_series(&layout.cards(), &layout.alternate_artwork(), &effects)
}
