# Lovelock HUD events for deadass

The companion reads two HUD scripts from Deadlock's `console.log`:

- `[DEADASS]` records from deadass's own Panorama addon (`mod/`)
- `[DEADLOCK_DEATH_HOOK]` records from [Lovelock](https://gamebanana.com/mods/718568) by asteriaOW

Lovelock reads much more of the HUD than deadass's addon. `dist/pak01_dir.vpk` is
Lovelock's addon with this directory's watchers (`hud/*.js`) appended. Use it instead
of either mod's own VPK, not alongside them: all three replace the same HUD layout file.

## Events

| Trigger | Source |
|---|---|
| kill, death, assist, respawn | Lovelock |
| ability cast / ready, per slot | Lovelock (slots shifted to deadass's 0-based keys) |
| parry, parry failed (you got stunned) | Lovelock |
| damage dealt / taken, healing received (passive regen under 5 HP dropped) | Lovelock |
| health dropped to mid / low | Lovelock |
| soul denied, healed an ally, shielded an ally | Lovelock |
| enemy Guardian / Walker / Shrine / Base Guardian destroyed, Patron weakened | Lovelock |
| match won / lost | Lovelock |
| big soul pickup (breakable boxes), small soul pickup (trooper last hits) | `hud/soul_pickups.js` |

New triggers are disabled until you turn them on in the companion.

## Install

1. Put `dist/pak01_dir.vpk` in your addons folder, or import it with a mod manager.
2. Add `-condebug` to Deadlock's launch options so `console.log` is written.
3. In the companion, set the game data source to **mod**.

## Rebuild

Download Lovelock from its GameBanana page, extract its `pak01_dir.vpk`, then:

```sh
python3 lovelock/build_vpk.py path/to/lovelock/pak01_dir.vpk
```

This rewrites `dist/pak01_dir.vpk` with every script in `hud/`. It needs only Python 3.

## Credits

Lovelock's HUD script (`panorama/scripts/death_http_bridge.vjs_c` in the VPK) and its
layout are asteriaOW's work, included unmodified apart from the appended watchers. They
are not covered by this repository's license.
