//! One panel for everything that can be switched on.
//!
//! Five unlabelled function keys were already one design decision short of
//! twelve: nothing told you what existed, and the space runs out. So the
//! switches are data — overlays and log channels in one list — and the panel
//! renders whatever the list contains. Adding a switch costs a variant.
//!
//! **F1** opens it, **↑/↓** move, **Space** toggles, **P** dumps the current
//! snapshot to the log. Those four are all a person has to remember.
//!
//! Writing here is confined to the diagnostics' own switches: the panel never
//! touches match state.

use crate::overlays::OverlaySettings;
use bevy::prelude::*;
use football_domain::diagnostics::{DiagnosticChannel, DiagnosticChannels, MatchSnapshot};

pub struct DebugHubPlugin;

impl Plugin for DebugHubPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugHub>()
            .init_resource::<DiagnosticChannels>()
            .add_systems(Startup, spawn_hub_panel)
            .add_systems(
                Update,
                (drive_hub, draw_hub_panel)
                    .chain()
                    .run_if(resource_exists::<ButtonInput<KeyCode>>),
            );
    }
}

/// Something that can be turned on. Overlays draw; channels write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugSwitch {
    Velocities,
    BallFuture,
    Possession,
    OffsideLine,
    RestartSpot,
    Log(DiagnosticChannel),
}

impl DebugSwitch {
    /// Overlays first because they are what a person reaches for while
    /// watching; the log channels below them, in the order they were declared.
    pub fn all() -> Vec<DebugSwitch> {
        let mut switches = vec![
            DebugSwitch::Velocities,
            DebugSwitch::BallFuture,
            DebugSwitch::Possession,
            DebugSwitch::OffsideLine,
            DebugSwitch::RestartSpot,
        ];
        switches.extend(DiagnosticChannel::ALL.map(DebugSwitch::Log));
        switches
    }

    pub fn label(self) -> String {
        match self {
            DebugSwitch::Velocities => "Overlay: velocities".to_string(),
            DebugSwitch::BallFuture => "Overlay: ball future".to_string(),
            DebugSwitch::Possession => "Overlay: possession".to_string(),
            DebugSwitch::OffsideLine => "Overlay: offside line".to_string(),
            DebugSwitch::RestartSpot => "Overlay: restart spot".to_string(),
            DebugSwitch::Log(channel) => format!("Log: {}", channel_name(channel)),
        }
    }

    /// What it is for, or what it costs — the two things that decide whether to
    /// turn something on.
    pub fn hint(self) -> &'static str {
        match self {
            DebugSwitch::Velocities => "where every body is going",
            DebugSwitch::BallFuture => "the buffer the AI reads — this is the physics",
            DebugSwitch::Possession => "designated player, holder and pass in flight",
            DebugSwitch::OffsideLine => "the line the referee actually judged",
            DebugSwitch::RestartSpot => "where the ball goes back into play",
            DebugSwitch::Log(channel) => channel.cost(),
        }
    }

    fn is_on(self, overlays: &OverlaySettings, channels: &DiagnosticChannels) -> bool {
        match self {
            DebugSwitch::Velocities => overlays.velocities,
            DebugSwitch::BallFuture => overlays.ball_future,
            DebugSwitch::Possession => overlays.possession,
            DebugSwitch::OffsideLine => overlays.offside,
            DebugSwitch::RestartSpot => overlays.restart_spot,
            DebugSwitch::Log(channel) => channels.is_enabled(channel),
        }
    }

    fn flip(self, overlays: &mut OverlaySettings, channels: &mut DiagnosticChannels) {
        match self {
            DebugSwitch::Velocities => overlays.velocities = !overlays.velocities,
            DebugSwitch::BallFuture => overlays.ball_future = !overlays.ball_future,
            DebugSwitch::Possession => overlays.possession = !overlays.possession,
            DebugSwitch::OffsideLine => overlays.offside = !overlays.offside,
            DebugSwitch::RestartSpot => overlays.restart_spot = !overlays.restart_spot,
            DebugSwitch::Log(channel) => {
                channels.toggle(channel);
            }
        }
    }
}

fn channel_name(channel: DiagnosticChannel) -> &'static str {
    match channel {
        DiagnosticChannel::Possession => "possession",
        DiagnosticChannel::RefereeDecisions => "referee decisions",
        DiagnosticChannel::Touches => "touches",
        DiagnosticChannel::PassOutcomes => "pass outcomes",
        DiagnosticChannel::PhaseTransitions => "phase transitions",
        DiagnosticChannel::Formation => "formation",
        DiagnosticChannel::Performance => "performance",
    }
}

#[derive(Resource, Debug, Default)]
pub struct DebugHub {
    pub open: bool,
    pub selected: usize,
}

#[derive(Component)]
struct HubPanel;

fn spawn_hub_panel(mut commands: Commands) {
    commands.spawn((
        Name::new("Debug hub"),
        HubPanel,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(120.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));
}

fn drive_hub(
    keys: Res<ButtonInput<KeyCode>>,
    mut hub: ResMut<DebugHub>,
    mut overlays: ResMut<OverlaySettings>,
    mut channels: ResMut<DiagnosticChannels>,
    snapshot: Option<Res<MatchSnapshot>>,
) {
    if keys.just_pressed(KeyCode::F1) {
        hub.open = !hub.open;
    }

    // Marking a moment must not require opening a panel over the thing being
    // watched, so this one works whether the hub is open or not.
    if keys.just_pressed(KeyCode::KeyP)
        && let Some(snapshot) = snapshot
    {
        info!("--- snapshot ---");
        for line in snapshot.lines() {
            info!("{line}");
        }
    }

    if !hub.open {
        return;
    }

    let switches = DebugSwitch::all();
    if keys.just_pressed(KeyCode::ArrowDown) {
        hub.selected = (hub.selected + 1) % switches.len();
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        hub.selected = (hub.selected + switches.len() - 1) % switches.len();
    }
    if keys.just_pressed(KeyCode::Space) {
        switches[hub.selected.min(switches.len() - 1)].flip(&mut overlays, &mut channels);
    }
}

/// The panel as text. `panel_lines` is where the content is decided, so it can
/// be read without a window.
pub fn panel_lines(
    hub: &DebugHub,
    overlays: &OverlaySettings,
    channels: &DiagnosticChannels,
) -> Vec<String> {
    if !hub.open {
        return vec!["F1  debug hub".to_string()];
    }
    let mut lines = vec!["F1 close   ↑/↓ move   Space toggle   P snapshot".to_string()];
    for (index, switch) in DebugSwitch::all().into_iter().enumerate() {
        let cursor = if index == hub.selected { ">" } else { " " };
        let state = if switch.is_on(overlays, channels) {
            "ON "
        } else {
            "off"
        };
        lines.push(format!(
            "{cursor} [{state}] {:<26} {}",
            switch.label(),
            switch.hint()
        ));
    }
    lines
}

fn draw_hub_panel(
    hub: Res<DebugHub>,
    overlays: Res<OverlaySettings>,
    channels: Res<DiagnosticChannels>,
    mut panel: Query<&mut Text, With<HubPanel>>,
) {
    let Ok(mut text) = panel.single_mut() else {
        return;
    };
    let drawn = panel_lines(&hub, &overlays, &channels).join("\n");
    if text.0 != drawn {
        text.0 = drawn;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_the_hub_says_only_how_to_open_it() {
        let lines = panel_lines(
            &DebugHub::default(),
            &OverlaySettings::default(),
            &DiagnosticChannels::default(),
        );
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("F1"));
    }

    /// The point of the list being data: everything that exists is listed,
    /// including channels added later.
    #[test]
    fn open_it_lists_every_switch_there_is() {
        let hub = DebugHub {
            open: true,
            selected: 0,
        };
        let lines = panel_lines(
            &hub,
            &OverlaySettings::default(),
            &DiagnosticChannels::default(),
        );

        assert_eq!(
            lines.len(),
            DebugSwitch::all().len() + 1,
            "a switch is hidden"
        );
        for switch in DebugSwitch::all() {
            assert!(
                lines.iter().any(|line| line.contains(&switch.label())),
                "{} is not listed",
                switch.label()
            );
        }
    }

    #[test]
    fn a_switch_reports_the_state_it_is_actually_in() {
        let hub = DebugHub {
            open: true,
            selected: 0,
        };
        let mut overlays = OverlaySettings::default();
        let mut channels = DiagnosticChannels::default();

        DebugSwitch::Velocities.flip(&mut overlays, &mut channels);
        DebugSwitch::Log(DiagnosticChannel::Touches).flip(&mut overlays, &mut channels);

        let lines = panel_lines(&hub, &overlays, &channels);
        let line_for = |label: &str| {
            lines
                .iter()
                .find(|line| line.contains(label))
                .unwrap()
                .clone()
        };

        assert!(line_for("Overlay: velocities").contains("[off]"));
        assert!(line_for("Log: touches").contains("[ON ]"));
    }
}
