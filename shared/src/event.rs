use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    Kill,
    Death,
    Assist,
    AbilityUsed { slot: u8 },
    AbilityReady { slot: u8 },
    Parry, // Successful parry
    Parried,
    PunchLanded,
    PunchTaken,
    Respawn,
    // Emitted only by the Lovelock HUD script.
    ParryFailed, // Own parry ended with the local player stunned
    SoulDeny,
    AllyHealed,
    AllyShielded,
    DamageGiven,
    DamageTaken,
    HealingReceived,
    HealthMid,
    HealthLow,
    GuardianDestroyed, // Enemy objectives only
    WalkerDestroyed,
    ShrineDestroyed,
    BaseGuardianDestroyed,
    PatronWeakened,
    MatchWon,
    MatchLost,
    // Emitted by deadass's own soul-pickup HUD watcher.
    SoulPickupLarge, // Box souls
    SoulPickupSmall, // Trooper last hits
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GameEvent {
    pub sequence: u64,
    pub wall_time_ms: u64,
    pub kind: EventKind,
}

impl GameEvent {
    pub fn new(sequence: u64, wall_time_ms: u64, kind: EventKind) -> Self {
        Self {
            sequence,
            wall_time_ms,
            kind,
        }
    }
}
