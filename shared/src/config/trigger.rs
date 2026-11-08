use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::shock::ShockConfig;
use crate::event::EventKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pattern {
    Vibrate,
    Pulse,
    Ramp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriggerKind {
    Kill,
    Death,
    Assist,
    AbilityUsed { slot: u8 },
    AbilityReady { slot: u8 },
    Parry,
    Parried,
    PunchLanded,
    PunchTaken,
    Respawn,
    ParryFailed,
    SoulDeny,
    AllyHealed,
    AllyShielded,
    DamageGiven,
    DamageTaken,
    HealingReceived,
    HealthMid,
    HealthLow,
    GuardianDestroyed,
    WalkerDestroyed,
    ShrineDestroyed,
    BaseGuardianDestroyed,
    PatronWeakened,
    MatchWon,
    MatchLost,
}

impl TriggerKind {
    /// Every trigger without an ability slot, in display order. Key parsing and the
    /// settings UI both derive from this, so a kind missing here cannot be configured.
    pub const NAMED: [TriggerKind; 24] = [
        Self::Kill,
        Self::Death,
        Self::Assist,
        Self::Respawn,
        Self::Parry,
        Self::Parried,
        Self::ParryFailed,
        Self::PunchLanded,
        Self::PunchTaken,
        Self::DamageGiven,
        Self::DamageTaken,
        Self::HealingReceived,
        Self::HealthMid,
        Self::HealthLow,
        Self::SoulDeny,
        Self::AllyHealed,
        Self::AllyShielded,
        Self::GuardianDestroyed,
        Self::WalkerDestroyed,
        Self::ShrineDestroyed,
        Self::BaseGuardianDestroyed,
        Self::PatronWeakened,
        Self::MatchWon,
        Self::MatchLost,
    ];

    pub fn family(self) -> TriggerFamily {
        match self {
            Self::Kill => TriggerFamily::Kill,
            Self::Death => TriggerFamily::Death,
            Self::Assist => TriggerFamily::Assist,
            Self::AbilityUsed { .. } => TriggerFamily::AbilityUsed,
            Self::AbilityReady { .. } => TriggerFamily::AbilityReady,
            Self::Parry => TriggerFamily::Parry,
            Self::Parried => TriggerFamily::Parried,
            Self::PunchLanded => TriggerFamily::PunchLanded,
            Self::PunchTaken => TriggerFamily::PunchTaken,
            Self::Respawn => TriggerFamily::Respawn,
            Self::ParryFailed => TriggerFamily::ParryFailed,
            Self::SoulDeny => TriggerFamily::SoulDeny,
            Self::AllyHealed => TriggerFamily::AllyHealed,
            Self::AllyShielded => TriggerFamily::AllyShielded,
            Self::DamageGiven => TriggerFamily::DamageGiven,
            Self::DamageTaken => TriggerFamily::DamageTaken,
            Self::HealingReceived => TriggerFamily::HealingReceived,
            Self::HealthMid => TriggerFamily::HealthMid,
            Self::HealthLow => TriggerFamily::HealthLow,
            Self::GuardianDestroyed => TriggerFamily::GuardianDestroyed,
            Self::WalkerDestroyed => TriggerFamily::WalkerDestroyed,
            Self::ShrineDestroyed => TriggerFamily::ShrineDestroyed,
            Self::BaseGuardianDestroyed => TriggerFamily::BaseGuardianDestroyed,
            Self::PatronWeakened => TriggerFamily::PatronWeakened,
            Self::MatchWon => TriggerFamily::MatchWon,
            Self::MatchLost => TriggerFamily::MatchLost,
        }
    }
}

impl fmt::Display for TriggerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kill => write!(f, "kill"),
            Self::Death => write!(f, "death"),
            Self::Assist => write!(f, "assist"),
            Self::Respawn => write!(f, "respawn"),
            Self::AbilityUsed { slot } => write!(f, "ability_used:{slot}"),
            Self::AbilityReady { slot } => write!(f, "ability_ready:{slot}"),
            Self::Parry => write!(f, "parry"),
            Self::Parried => write!(f, "parried"),
            Self::PunchLanded => write!(f, "punch_landed"),
            Self::PunchTaken => write!(f, "punch_taken"),
            Self::ParryFailed => write!(f, "parry_failed"),
            Self::SoulDeny => write!(f, "soul_deny"),
            Self::AllyHealed => write!(f, "ally_healed"),
            Self::AllyShielded => write!(f, "ally_shielded"),
            Self::DamageGiven => write!(f, "damage_given"),
            Self::DamageTaken => write!(f, "damage_taken"),
            Self::HealingReceived => write!(f, "healing_received"),
            Self::HealthMid => write!(f, "health_mid"),
            Self::HealthLow => write!(f, "health_low"),
            Self::GuardianDestroyed => write!(f, "guardian_destroyed"),
            Self::WalkerDestroyed => write!(f, "walker_destroyed"),
            Self::ShrineDestroyed => write!(f, "shrine_destroyed"),
            Self::BaseGuardianDestroyed => write!(f, "base_guardian_destroyed"),
            Self::PatronWeakened => write!(f, "patron_weakened"),
            Self::MatchWon => write!(f, "match_won"),
            Self::MatchLost => write!(f, "match_lost"),
        }
    }
}

impl FromStr for TriggerKind {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if let Some(named) = Self::NAMED.into_iter().find(|kind| kind.to_string() == raw) {
            return Ok(named);
        }
        let (name, slot) = raw.split_once(':').ok_or_else(|| unknown_trigger(raw))?;
        let slot: u8 = slot.parse().map_err(|_| unknown_trigger(raw))?;
        match name {
            "ability_used" => Ok(Self::AbilityUsed { slot }),
            "ability_ready" => Ok(Self::AbilityReady { slot }),
            _ => Err(unknown_trigger(raw)),
        }
    }
}

fn unknown_trigger(raw: &str) -> String {
    format!("unknown trigger {raw}")
}

impl From<EventKind> for TriggerKind {
    fn from(kind: EventKind) -> Self {
        match kind {
            EventKind::Kill => Self::Kill,
            EventKind::Death => Self::Death,
            EventKind::Assist => Self::Assist,
            EventKind::AbilityUsed { slot } => Self::AbilityUsed { slot },
            EventKind::AbilityReady { slot } => Self::AbilityReady { slot },
            EventKind::Parry => Self::Parry,
            EventKind::Parried => Self::Parried,
            EventKind::PunchLanded => Self::PunchLanded,
            EventKind::PunchTaken => Self::PunchTaken,
            EventKind::Respawn => Self::Respawn,
            EventKind::ParryFailed => Self::ParryFailed,
            EventKind::SoulDeny => Self::SoulDeny,
            EventKind::AllyHealed => Self::AllyHealed,
            EventKind::AllyShielded => Self::AllyShielded,
            EventKind::DamageGiven => Self::DamageGiven,
            EventKind::DamageTaken => Self::DamageTaken,
            EventKind::HealingReceived => Self::HealingReceived,
            EventKind::HealthMid => Self::HealthMid,
            EventKind::HealthLow => Self::HealthLow,
            EventKind::GuardianDestroyed => Self::GuardianDestroyed,
            EventKind::WalkerDestroyed => Self::WalkerDestroyed,
            EventKind::ShrineDestroyed => Self::ShrineDestroyed,
            EventKind::BaseGuardianDestroyed => Self::BaseGuardianDestroyed,
            EventKind::PatronWeakened => Self::PatronWeakened,
            EventKind::MatchWon => Self::MatchWon,
            EventKind::MatchLost => Self::MatchLost,
        }
    }
}

impl Serialize for TriggerKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for TriggerKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TriggerVisitor;

        impl serde::de::Visitor<'_> for TriggerVisitor {
            type Value = TriggerKind;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a trigger key like \"kill\" or \"ability_used:0\"")
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<TriggerKind, E> {
                value.parse().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(TriggerVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerFamily {
    Kill,
    Death,
    Assist,
    AbilityUsed,
    AbilityReady,
    Parry,
    Parried,
    PunchLanded,
    PunchTaken,
    Respawn,
    ParryFailed,
    SoulDeny,
    AllyHealed,
    AllyShielded,
    DamageGiven,
    DamageTaken,
    HealingReceived,
    HealthMid,
    HealthLow,
    GuardianDestroyed,
    WalkerDestroyed,
    ShrineDestroyed,
    BaseGuardianDestroyed,
    PatronWeakened,
    MatchWon,
    MatchLost,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerConfig {
    pub enabled: bool,
    pub strength: f64,
    pub duration_ms: u64,
    pub retrigger_cooldown_ms: u64,
    pub pattern: Pattern,
    #[serde(default)]
    pub shock: Option<ShockConfig>,
    #[serde(default)]
    pub vibrate_devices: Option<Vec<String>>,
}

impl TriggerConfig {
    pub(crate) fn preset(strength: f64, duration_ms: u64) -> Self {
        Self {
            strength,
            duration_ms,
            ..Self::default()
        }
    }

    /// An opt-in preset: shipped disabled so new triggers never start firing on upgrade.
    pub(crate) fn opt_in(strength: f64, duration_ms: u64, retrigger_cooldown_ms: u64) -> Self {
        Self {
            enabled: false,
            retrigger_cooldown_ms,
            ..Self::preset(strength, duration_ms)
        }
    }

    pub fn scaled_strength(&self, master_gain: f64, cap: f64) -> f64 {
        (self.strength * master_gain).clamp(0.0, cap)
    }
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strength: 0.6,
            duration_ms: 800,
            retrigger_cooldown_ms: 500,
            pattern: Pattern::Vibrate,
            shock: None,
            vibrate_devices: None,
        }
    }
}

pub fn resolve_vibrate_targets(selected: Option<&[String]>, connected: &[String]) -> Vec<String> {
    match selected {
        None => connected.to_vec(),
        Some(picks) => connected
            .iter()
            .filter(|name| picks.iter().any(|pick| pick.trim() == *name))
            .cloned()
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn trigger_key_round_trips_through_parse() {
        for kind in TriggerKind::NAMED.into_iter().chain([
            TriggerKind::AbilityUsed { slot: 2 },
            TriggerKind::AbilityReady { slot: 0 },
        ]) {
            assert_eq!(kind.to_string().parse(), Ok(kind));
        }
    }

    #[test]
    fn named_triggers_are_unique_and_slotless() {
        let mut keys: Vec<String> = TriggerKind::NAMED
            .iter()
            .map(|kind| kind.to_string())
            .collect();
        assert!(keys.iter().all(|key| !key.contains(':')));
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), TriggerKind::NAMED.len());
    }

    #[test]
    fn unknown_trigger_key_fails_to_parse() {
        assert!("nonsense".parse::<TriggerKind>().is_err());
        assert!("ability_used:x".parse::<TriggerKind>().is_err());
    }

    #[test]
    fn legacy_trigger_config_without_shock_still_parses() {
        let rendered = r#"
            enabled = true
            strength = 0.7
            duration_ms = 900
            retrigger_cooldown_ms = 500
            pattern = "vibrate"
        "#;
        let rule: TriggerConfig = toml::from_str(rendered).expect("legacy config parses");
        assert!(rule.shock.is_none());
    }

    #[test]
    fn vibrate_selection_resolves_against_connected_toys() {
        let connected = vec![String::from("Loonie"), String::from("Edge")];
        assert_eq!(resolve_vibrate_targets(None, &connected), connected);
        assert_eq!(
            resolve_vibrate_targets(Some(&[String::from("Edge")]), &connected),
            ["Edge"]
        );
        assert!(resolve_vibrate_targets(Some(&[String::from("Ghost")]), &connected).is_empty());
        assert!(resolve_vibrate_targets(Some(&[]), &connected).is_empty());
    }

    #[test]
    fn exact_slot_wins_over_family_fallback() {
        let mut config = AppConfig::default();
        config.triggers.insert(
            TriggerKind::AbilityUsed { slot: 1 },
            TriggerConfig::preset(0.9, 100),
        );
        assert_eq!(
            config
                .trigger(TriggerKind::AbilityUsed { slot: 1 })
                .strength,
            0.9
        );
    }
}
