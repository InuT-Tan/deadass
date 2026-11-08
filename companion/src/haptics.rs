use deadass_shared::{AppConfig, GameEvent, Pattern, TriggerKind, now_ms};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HapticCommand {
    pub strength: f64,
    pub duration_ms: u64,
    pub pattern: Pattern,
}

impl HapticCommand {
    pub fn from_trigger(config: &AppConfig, kind: TriggerKind) -> Option<Self> {
        let rule = config.trigger(kind);
        if !rule.enabled {
            return None;
        }
        Some(Self {
            strength: rule.scaled_strength(config.master_gain, config.max_strength_cap),
            duration_ms: rule.duration_ms,
            pattern: rule.pattern,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuppressReason {
    Disabled,
    MutedWhileDead,
    Cooldown,
    ReadyRightAfterCast,
}

/// HUD cooldown state flickers on multi-stage abilities (Rem's Naptime reported ready
/// 0.3 s after casting, Lil Helpers 0.1 s, Tag Along 0.5 s); the shortest real
/// cooldown seen was ~5 s. A "ready" this soon after the slot's cast is not one.
const READY_AFTER_CAST_MS: u64 = 2500;

impl fmt::Display for SuppressReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => write!(f, "disabled in config"),
            Self::MutedWhileDead => write!(f, "muted while dead"),
            Self::Cooldown => write!(f, "retrigger cooldown"),
            Self::ReadyRightAfterCast => write!(f, "ready right after cast"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GateDecision {
    Fire(HapticCommand),
    Suppress(SuppressReason),
}

#[derive(Default)]
pub struct HapticGate {
    last_fired_ms: HashMap<TriggerKind, u64>,
    dead_since_ms: Option<u64>,
    last_cast_ms: HashMap<u8, u64>,
}

impl HapticGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn decide(&mut self, config: &AppConfig, event: GameEvent) -> GateDecision {
        self.track_liveness(event);
        if self.ready_right_after_cast(event) {
            return GateDecision::Suppress(SuppressReason::ReadyRightAfterCast);
        }
        if config.mute_while_dead && self.dead_since_ms.is_some() {
            return GateDecision::Suppress(SuppressReason::MutedWhileDead);
        }
        let trigger = TriggerKind::from(event.kind);
        let Some(command) = HapticCommand::from_trigger(config, trigger) else {
            return GateDecision::Suppress(SuppressReason::Disabled);
        };
        let now = now_ms().max(event.wall_time_ms);
        if self.fired_recently(trigger, now, config.trigger(trigger).retrigger_cooldown_ms) {
            return GateDecision::Suppress(SuppressReason::Cooldown);
        }
        self.last_fired_ms.insert(trigger, now);
        GateDecision::Fire(command)
    }

    /// Records casts whether or not their trigger fires; both timestamps come from
    /// the same HUD script clock.
    fn ready_right_after_cast(&mut self, event: GameEvent) -> bool {
        use deadass_shared::EventKind;
        match event.kind {
            EventKind::AbilityUsed { slot } => {
                self.last_cast_ms.insert(slot, event.wall_time_ms);
                false
            }
            EventKind::AbilityReady { slot } => self
                .last_cast_ms
                .get(&slot)
                .is_some_and(|cast| event.wall_time_ms.saturating_sub(*cast) < READY_AFTER_CAST_MS),
            _ => false,
        }
    }

    fn track_liveness(&mut self, event: GameEvent) {
        use deadass_shared::EventKind;
        match event.kind {
            EventKind::Death => self.dead_since_ms = Some(event.wall_time_ms),
            EventKind::Respawn => self.dead_since_ms = None,
            _ => {}
        }
    }

    fn fired_recently(&self, trigger: TriggerKind, now: u64, cooldown_ms: u64) -> bool {
        self.last_fired_ms
            .get(&trigger)
            .is_some_and(|last| now.saturating_sub(*last) < cooldown_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadass_shared::EventKind;

    fn kill_at(wall_time_ms: u64) -> GameEvent {
        GameEvent::new(1, wall_time_ms, EventKind::Kill)
    }

    fn death_at(wall_time_ms: u64) -> GameEvent {
        GameEvent::new(1, wall_time_ms, EventKind::Death)
    }

    fn respawn_at(wall_time_ms: u64) -> GameEvent {
        GameEvent::new(2, wall_time_ms, EventKind::Respawn)
    }

    #[test]
    fn second_immediate_kill_is_debounced() {
        let config = AppConfig::default();
        let mut gate = HapticGate::new();
        assert!(matches!(
            gate.decide(&config, kill_at(1000)),
            GateDecision::Fire(_)
        ));
        assert!(matches!(
            gate.decide(&config, kill_at(1001)),
            GateDecision::Suppress(SuppressReason::Cooldown)
        ));
    }

    #[test]
    fn ready_right_after_a_cast_of_the_same_slot_is_suppressed() {
        // Rem's Naptime as logged: used at 8.3 s, "ready" at 8.6 s, real ready much later.
        let config = AppConfig::default();
        let mut gate = HapticGate::new();
        let at = |ms, kind| GameEvent::new(1, ms, kind);
        gate.decide(&config, at(8_300, EventKind::AbilityUsed { slot: 3 }));
        assert_eq!(
            gate.decide(&config, at(8_600, EventKind::AbilityReady { slot: 3 })),
            GateDecision::Suppress(SuppressReason::ReadyRightAfterCast)
        );
        assert!(matches!(
            gate.decide(&config, at(8_700, EventKind::AbilityReady { slot: 1 })),
            GateDecision::Fire(_)
        ));
        assert!(matches!(
            gate.decide(&config, at(60_000, EventKind::AbilityReady { slot: 3 })),
            GateDecision::Fire(_)
        ));
    }

    #[test]
    fn a_disabled_cast_still_arms_the_ready_flicker_guard() {
        let mut config = AppConfig::default();
        config
            .triggers
            .get_mut(&TriggerKind::AbilityUsed { slot: 2 })
            .unwrap()
            .enabled = false;
        let mut gate = HapticGate::new();
        gate.decide(
            &config,
            GameEvent::new(1, 1_000, EventKind::AbilityUsed { slot: 2 }),
        );
        assert_eq!(
            gate.decide(
                &config,
                GameEvent::new(2, 1_100, EventKind::AbilityReady { slot: 2 })
            ),
            GateDecision::Suppress(SuppressReason::ReadyRightAfterCast)
        );
    }

    #[test]
    fn mute_while_dead_suppresses_until_respawn() {
        let config = AppConfig {
            mute_while_dead: true,
            ..AppConfig::default()
        };
        let mut gate = HapticGate::new();
        gate.decide(&config, death_at(1000));
        assert_eq!(
            gate.decide(&config, kill_at(2000)),
            GateDecision::Suppress(SuppressReason::MutedWhileDead)
        );
        gate.decide(&config, respawn_at(3000));
        assert!(matches!(
            gate.decide(&config, kill_at(4000)),
            GateDecision::Fire(_)
        ));
    }
}
