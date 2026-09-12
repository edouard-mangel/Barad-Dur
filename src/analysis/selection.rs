/// The categories one calculation computes. Commands build one at their
/// boundary — from CLI filters, or as a fixed policy — so the calculation
/// itself never reads arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CategorySelection {
    pub health: bool,
    pub team: bool,
    pub evolution: bool,
    pub hygiene: bool,
    pub coupling: bool,
    pub deps: bool,
}

impl CategorySelection {
    /// `gate` scores every category, always.
    pub const GATE: Self = Self {
        health: true,
        team: true,
        evolution: true,
        hygiene: true,
        coupling: true,
        deps: false,
    };

    /// `backfill` records scores and entity-trend hotspot rows only, so it
    /// skips the Coupling category; the coupling evidence is still derived
    /// once per calculation, for the history counts and the rows.
    pub const BACKFILL: Self = Self {
        health: true,
        team: true,
        evolution: true,
        hygiene: true,
        coupling: false,
        deps: false,
    };

    /// `analyze`'s filter rule: no filter runs every category; any filter
    /// runs exactly the named ones. There is no Coupling filter, so a
    /// filtered run never includes it. Dependencies join only on explicit
    /// opt-in, in either case.
    pub fn from_filters(
        health: bool,
        team: bool,
        evolution: bool,
        hygiene: bool,
        deps: bool,
    ) -> Self {
        let unfiltered = !(health || team || evolution || hygiene);
        Self {
            health: unfiltered || health,
            team: unfiltered || team,
            evolution: unfiltered || evolution,
            hygiene: unfiltered || hygiene,
            coupling: unfiltered,
            deps,
        }
    }
}
