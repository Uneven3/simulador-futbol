//! What the diagnostics report about the present, as pure data.
//!
//! One snapshot, two sinks: the HUD draws it and the console writes it.
//! Producers fill their section and never format for a particular sink — that
//! is the whole point. Both surfaces must show the same numbers, and the only
//! way to guarantee that is to have them read the same values instead of
//! formatting twice.

use bevy_ecs::prelude::*;

/// One labelled value.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub label: String,
    pub value: String,
    /// Continuous values move every tick. Change-triggered output skips them,
    /// or a drifting float would emit a line per tick and bury the transitions.
    pub volatile: bool,
}

impl Field {
    /// A discrete value: worth a line when it changes.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            volatile: false,
        }
    }

    /// A continuous value: shown, never a reason to emit on its own.
    pub fn volatile(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            volatile: true,
            ..Self::new(label, value)
        }
    }
}

/// Fixed slots, so the report's order never depends on system execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionId {
    Scoreboard,
    Possession,
    Passing,
    Restart,
}

impl SectionId {
    pub const COUNT: usize = 4;
    pub const ALL: [SectionId; Self::COUNT] = [
        SectionId::Scoreboard,
        SectionId::Possession,
        SectionId::Passing,
        SectionId::Restart,
    ];

    pub fn title(self) -> &'static str {
        match self {
            SectionId::Scoreboard => "match",
            SectionId::Possession => "possession",
            SectionId::Passing => "passing",
            SectionId::Restart => "restart",
        }
    }

    fn index(self) -> usize {
        match self {
            SectionId::Scoreboard => 0,
            SectionId::Possession => 1,
            SectionId::Passing => 2,
            SectionId::Restart => 3,
        }
    }
}

/// The whole diagnostic picture of the present. A section absent from the
/// snapshot does not render: a producer with nothing to say leaves its slot
/// empty rather than reporting zeros.
#[derive(Resource, Debug, Default)]
pub struct MatchSnapshot {
    sections: [Option<Vec<Field>>; SectionId::COUNT],
}

impl MatchSnapshot {
    pub fn set(&mut self, id: SectionId, fields: Vec<Field>) {
        self.sections[id.index()] = Some(fields);
    }

    /// Drops a section, for a producer whose subject is absent — the mirror of
    /// `set`, so nothing lingers stale.
    pub fn clear(&mut self, id: SectionId) {
        self.sections[id.index()] = None;
    }

    pub fn fields(&self, id: SectionId) -> Option<&[Field]> {
        self.sections[id.index()].as_deref()
    }

    /// One section as `title: label=value  label=value`.
    pub fn line(&self, id: SectionId) -> Option<String> {
        self.render(id, false)
    }

    /// The same, with continuous values dropped: what change detection compares,
    /// so only a discrete transition triggers a line.
    pub fn stable_line(&self, id: SectionId) -> Option<String> {
        self.render(id, true)
    }

    pub fn lines(&self) -> Vec<String> {
        SectionId::ALL
            .into_iter()
            .filter_map(|id| self.line(id))
            .collect()
    }

    fn render(&self, id: SectionId, stable_only: bool) -> Option<String> {
        let fields = self.fields(id)?;
        let body: Vec<String> = fields
            .iter()
            .filter(|field| !(stable_only && field.volatile))
            .map(|field| format!("{}={}", field.label, field.value))
            .collect();
        if body.is_empty() {
            return None;
        }
        Some(format!("{}: {}", id.title(), body.join("  ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_section_renders_nothing() {
        let snapshot = MatchSnapshot::default();
        assert_eq!(snapshot.line(SectionId::Scoreboard), None);
        assert!(snapshot.lines().is_empty());
    }

    /// Change detection has to ignore values that move on their own, or every
    /// tick looks like a change.
    #[test]
    fn a_continuous_value_never_triggers_a_line() {
        let mut snapshot = MatchSnapshot::default();
        snapshot.set(
            SectionId::Scoreboard,
            vec![
                Field::new("score", "0-0"),
                Field::volatile("elapsed", "12s"),
            ],
        );
        let before = snapshot.stable_line(SectionId::Scoreboard);

        snapshot.set(
            SectionId::Scoreboard,
            vec![
                Field::new("score", "0-0"),
                Field::volatile("elapsed", "13s"),
            ],
        );

        assert_eq!(before, snapshot.stable_line(SectionId::Scoreboard));
        assert_ne!(before, snapshot.line(SectionId::Scoreboard));
    }

    #[test]
    fn a_cleared_section_stops_reporting_instead_of_going_stale() {
        let mut snapshot = MatchSnapshot::default();
        snapshot.set(SectionId::Restart, vec![Field::new("awarded", "Corner")]);
        assert!(snapshot.line(SectionId::Restart).is_some());

        snapshot.clear(SectionId::Restart);
        assert_eq!(snapshot.line(SectionId::Restart), None);
    }
}
