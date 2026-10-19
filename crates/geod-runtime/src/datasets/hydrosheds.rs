//! Finite reviewed release index for the DIR product only. Do not silently add
//! new tiles or substitute a different release: persisted source receipts bind
//! the version and product declaration independently of upstream availability.
// Publisher: https://data.hydrosheds.org/file/hydrosheds-v2/download-index.json?v=20260830
// Read 2026-10-12; original index SHA256:
// 28a3b61e2c6ade29fc4e8eed5c409a6704b73dad36e80324ca2d90934aecba6d
// Reviewed receipt: docs/research/hydrosheds-v2-index-20261012.json
pub(super) const FLOW_DIRECTION_TILES: [&str; 100] = [
    "n00w050", "n00w060", "n00w070", "n00w080", "n00w090", "n00w100", "n10w060", "n10w070",
    "n10w080", "n10w090", "n10w100", "n10w110", "n10w120", "n20w080", "n20w090", "n20w100",
    "n20w110", "n20w120", "n30w070", "n30w080", "n30w090", "n30w100", "n30w110", "n30w120",
    "n30w130", "n40w060", "n40w070", "n40w080", "n40w090", "n40w100", "n40w110", "n40w120",
    "n40w130", "n50w060", "n50w070", "n50w080", "n50w090", "n50w100", "n50w110", "n50w120",
    "n50w130", "n50w140", "n50w150", "n50w160", "n50w170", "n60w070", "n60w080", "n60w090",
    "n60w100", "n60w110", "n60w120", "n60w130", "n60w140", "n60w150", "n60w160", "n60w170",
    "n70w070", "n70w080", "n70w090", "n70w100", "n70w110", "n70w120", "n70w130", "n70w140",
    "n70w150", "n70w160", "n70w170", "n80w070", "n80w080", "n80w090", "n80w100", "n80w110",
    "s10w040", "s10w050", "s10w060", "s10w070", "s10w080", "s10w090", "s10w100", "s20w040",
    "s20w050", "s20w060", "s20w070", "s20w080", "s30w050", "s30w060", "s30w070", "s30w080",
    "s30w090", "s40w060", "s40w070", "s40w080", "s40w090", "s50w070", "s50w080", "s60w040",
    "s60w050", "s60w060", "s60w070", "s60w080",
];
