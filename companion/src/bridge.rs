use deadass_shared::{EventKind, GameEvent};
use serde::Deserialize;

pub const BRIDGE_RECORD_PREFIX: &str = "[DEADASS]";
pub const BRIDGE_SCHEMA: u32 = 1;
/// Lovelock's HUD script (`death_http_bridge.js`); same schema 1 envelope, richer event set.
pub const LOVELOCK_RECORD_PREFIX: &str = "[DEADLOCK_DEATH_HOOK]";
pub const LOVELOCK_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSignal {
    HookReady,
    Game(GameEvent),
}

#[derive(Deserialize)]
struct WireRecord {
    schema: u32,
    sequence: u64,
    client_time_ms: u64,
    #[serde(flatten)]
    kind: WireKind,
}

#[derive(Deserialize)]
#[serde(tag = "event")]
enum WireKind {
    #[serde(rename = "hook_ready")]
    HookReady {},
    #[serde(rename = "kill")]
    Kill {},
    #[serde(rename = "death")]
    Death {},
    #[serde(rename = "assist")]
    Assist {},
    #[serde(rename = "respawn")]
    Respawn {},
    #[serde(rename = "ability_used")]
    AbilityUsed { ability_slot: u8 },
    #[serde(rename = "ability_ready")]
    AbilityReady { ability_slot: u8 },
}

#[derive(Deserialize)]
struct LovelockRecord {
    schema: u32,
    // Only action events carry a sequence; hook_ready does not.
    #[serde(default)]
    sequence: u64,
    client_time_ms: u64,
    #[serde(flatten)]
    kind: LovelockKind,
}

// Diagnostic events (`*_diag`, `*_unclassified`, `ability_catalog`) are deliberately absent:
// they fail to deserialize and the line is dropped.
#[derive(Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum LovelockKind {
    HookReady {},
    LocalPlayerKill {},
    LocalPlayerDeath {},
    LocalPlayerAssist {},
    LocalPlayerRespawn {},
    AbilityUsed {
        ability_slot: u8,
    },
    AbilityCooldownReady {
        ability_slot: u8,
    },
    ParrySuccess {},
    ParryFail {},
    SoulDeny {},
    AllyHealed {},
    AllyShielded {},
    DamageGiven {},
    DamageTaken {},
    HealingReceived {},
    /// `detection` is `health_band:<n>`: 1 when the HUD enters mid health, 2 for low.
    DamageTakenIntensity {
        detection: String,
    },
    ObjectiveGuardian {},
    ObjectiveWalker {},
    ObjectiveShrine {},
    ObjectiveBaseGuardian {},
    ObjectivePatronWeakened {},
    GameWon {},
    GameLost {},
}

pub fn parse_bridge_line(line: &str) -> Option<ModSignal> {
    if let Some(at) = line.find(BRIDGE_RECORD_PREFIX) {
        return parse_deadass(&line[at + BRIDGE_RECORD_PREFIX.len()..]);
    }
    let at = line.find(LOVELOCK_RECORD_PREFIX)?;
    parse_lovelock(&line[at + LOVELOCK_RECORD_PREFIX.len()..])
}

fn parse_lovelock(payload: &str) -> Option<ModSignal> {
    let record: LovelockRecord = serde_json::from_str(payload).ok()?;
    if record.schema != LOVELOCK_SCHEMA {
        return None;
    }
    let kind = match record.kind {
        LovelockKind::HookReady {} => return Some(ModSignal::HookReady),
        LovelockKind::LocalPlayerKill {} => EventKind::Kill,
        LovelockKind::LocalPlayerDeath {} => EventKind::Death,
        LovelockKind::LocalPlayerAssist {} => EventKind::Assist,
        LovelockKind::LocalPlayerRespawn {} => EventKind::Respawn,
        LovelockKind::AbilityUsed { ability_slot } => EventKind::AbilityUsed {
            slot: lovelock_slot(ability_slot)?,
        },
        LovelockKind::AbilityCooldownReady { ability_slot } => EventKind::AbilityReady {
            slot: lovelock_slot(ability_slot)?,
        },
        LovelockKind::ParrySuccess {} => EventKind::Parry,
        LovelockKind::ParryFail {} => EventKind::ParryFailed,
        LovelockKind::SoulDeny {} => EventKind::SoulDeny,
        LovelockKind::AllyHealed {} => EventKind::AllyHealed,
        LovelockKind::AllyShielded {} => EventKind::AllyShielded,
        LovelockKind::DamageGiven {} => EventKind::DamageGiven,
        LovelockKind::DamageTaken {} => EventKind::DamageTaken,
        LovelockKind::HealingReceived {} => EventKind::HealingReceived,
        LovelockKind::DamageTakenIntensity { detection } => match detection.as_str() {
            "health_band:1" => EventKind::HealthMid,
            "health_band:2" => EventKind::HealthLow,
            _ => return None,
        },
        LovelockKind::ObjectiveGuardian {} => EventKind::GuardianDestroyed,
        LovelockKind::ObjectiveWalker {} => EventKind::WalkerDestroyed,
        LovelockKind::ObjectiveShrine {} => EventKind::ShrineDestroyed,
        LovelockKind::ObjectiveBaseGuardian {} => EventKind::BaseGuardianDestroyed,
        LovelockKind::ObjectivePatronWeakened {} => EventKind::PatronWeakened,
        LovelockKind::GameWon {} => EventKind::MatchWon,
        LovelockKind::GameLost {} => EventKind::MatchLost,
    };
    Some(ModSignal::Game(GameEvent::new(
        record.sequence,
        record.client_time_ms,
        kind,
    )))
}

/// Lovelock numbers ability slots from 1 (`slotIndex + 1`); deadass trigger keys start at 0.
fn lovelock_slot(slot: u8) -> Option<u8> {
    slot.checked_sub(1)
}

fn parse_deadass(payload: &str) -> Option<ModSignal> {
    let record: WireRecord = serde_json::from_str(payload).ok()?;
    if record.schema != BRIDGE_SCHEMA {
        return None;
    }
    let game = |kind| ModSignal::Game(GameEvent::new(record.sequence, record.client_time_ms, kind));
    match record.kind {
        WireKind::HookReady {} => Some(ModSignal::HookReady),
        WireKind::Kill {} => Some(game(EventKind::Kill)),
        WireKind::Death {} => Some(game(EventKind::Death)),
        WireKind::Assist {} => Some(game(EventKind::Assist)),
        WireKind::Respawn {} => Some(game(EventKind::Respawn)),
        WireKind::AbilityUsed { ability_slot } => {
            Some(game(EventKind::AbilityUsed { slot: ability_slot }))
        }
        WireKind::AbilityReady { ability_slot } => {
            Some(game(EventKind::AbilityReady { slot: ability_slot }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(event: &str) -> String {
        format!(
            "[DEADASS]{{\"schema\":1,\"event\":\"{event}\",\"mod_version\":\"0.1.0\",\"session_id\":\"abc\",\"sequence\":7,\"client_time_ms\":4242}}"
        )
    }

    #[test]
    fn kill_maps_to_game_event() {
        let Some(ModSignal::Game(event)) = parse_bridge_line(&record("kill")) else {
            panic!("kill line must parse");
        };
        assert_eq!(event.kind, EventKind::Kill);
        assert_eq!(event.sequence, 7);
        assert_eq!(event.wall_time_ms, 4242);
    }

    #[test]
    fn hook_ready_is_not_a_game_event() {
        assert_eq!(
            parse_bridge_line(&record("hook_ready")),
            Some(ModSignal::HookReady)
        );
    }

    #[test]
    fn mismatched_schema_is_rejected() {
        let line = "[DEADASS]{\"schema\":999,\"event\":\"kill\",\"session_id\":\"abc\",\"sequence\":1,\"client_time_ms\":1}";
        assert_eq!(parse_bridge_line(line), None);
    }

    fn lovelock(body: &str) -> String {
        format!(
            "11/07 19:00:00 [Panorama] [DEADLOCK_DEATH_HOOK]{{\"schema\":1,\"mod_version\":\"1.0.0\",\"session_id\":\"s\",\"client_time_ms\":4242,{body}}}"
        )
    }

    fn lovelock_kind(body: &str) -> Option<EventKind> {
        match parse_bridge_line(&lovelock(body)) {
            Some(ModSignal::Game(event)) => Some(event.kind),
            _ => None,
        }
    }

    #[test]
    fn lovelock_events_map_to_game_events() {
        let cases = [
            (
                r#""event":"local_player_kill","sequence":3,"detection":"kill_streak","kills_before":0,"kills_after":1"#,
                EventKind::Kill,
            ),
            (
                r#""event":"local_player_respawn","sequence":3"#,
                EventKind::Respawn,
            ),
            (
                r#""event":"ability_used","sequence":3,"ability_slot":1,"ability_name":"x","detection":"activated""#,
                EventKind::AbilityUsed { slot: 0 },
            ),
            (
                r#""event":"ability_cooldown_ready","sequence":3,"ability_slot":4"#,
                EventKind::AbilityReady { slot: 3 },
            ),
            (
                r#""event":"parry_success","sequence":3,"detection":"enemy_stunned_after_parry""#,
                EventKind::Parry,
            ),
            (
                r#""event":"parry_fail","sequence":3"#,
                EventKind::ParryFailed,
            ),
            (
                r#""event":"damage_taken","sequence":3,"amount":120,"health":480"#,
                EventKind::DamageTaken,
            ),
            (
                r#""event":"damage_taken_intensity","sequence":3,"detection":"health_band:2""#,
                EventKind::HealthLow,
            ),
            (
                r#""event":"objective_patron_weakened","sequence":3"#,
                EventKind::PatronWeakened,
            ),
            (
                r#""event":"game_lost","sequence":3,"detection":"match_end:enemy_team_victory""#,
                EventKind::MatchLost,
            ),
        ];
        for (body, kind) in cases {
            assert_eq!(lovelock_kind(body), Some(kind), "{body}");
        }
    }

    #[test]
    fn lovelock_record_keeps_sequence_and_time() {
        let Some(ModSignal::Game(event)) =
            parse_bridge_line(&lovelock(r#""event":"soul_deny","sequence":9"#))
        else {
            panic!("soul_deny must parse");
        };
        assert_eq!(
            (event.kind, event.sequence, event.wall_time_ms),
            (EventKind::SoulDeny, 9, 4242)
        );
    }

    #[test]
    fn lovelock_hook_ready_and_diagnostics() {
        assert_eq!(
            parse_bridge_line(&lovelock(r#""event":"hook_ready","poll_interval_ms":100"#)),
            Some(ModSignal::HookReady)
        );
        for body in [
            r#""event":"objectives_map_diag","count":3"#,
            r#""event":"ability_catalog","abilities":[]"#,
            r#""event":"damage_taken_intensity","sequence":1,"detection":"health_band:7""#,
            r#""event":"ability_used","sequence":1,"ability_slot":0"#,
        ] {
            assert_eq!(parse_bridge_line(&lovelock(body)), None, "{body}");
        }
    }

    #[test]
    fn lovelock_schema_mismatch_is_rejected() {
        let line = r#"[DEADLOCK_DEATH_HOOK]{"schema":2,"event":"game_won","client_time_ms":1}"#;
        assert_eq!(parse_bridge_line(line), None);
    }

    #[test]
    fn console_log_wrapping_does_not_break_prefix_search() {
        let line = format!("[123.45] Panorama: {}", record("death"));
        let Some(ModSignal::Game(event)) = parse_bridge_line(&line) else {
            panic!("wrapped line must parse");
        };
        assert_eq!(event.kind, EventKind::Death);
    }
}
