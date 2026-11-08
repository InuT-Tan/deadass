#!/usr/bin/env python3
"""Merge a haptic profile's trigger tables into the deadass companion's config.toml.

    python3 lovelock/support_profile.py [PROFILE.toml] [--config PATH]

PROFILE defaults to support-profile.toml next to this script. Only enabled, strength,
duration_ms, retrigger_cooldown_ms and pattern are copied; everything else in your
config (connection settings, per-trigger toy selection, shock settings) is kept. A
timestamped backup is written first. A running companion picks up the change live.
Needs Python 3.11+.
"""
import argparse, datetime, os, shutil, sys, tomllib

FIELDS = ("enabled", "strength", "duration_ms", "retrigger_cooldown_ms", "pattern")
PATTERNS = {"vibrate", "pulse", "ramp"}


def default_config_path():
    # Mirrors dirs::config_dir() in the companion (companion/src/config_store.rs).
    if sys.platform == "win32":
        base = os.environ["APPDATA"]
    elif sys.platform == "darwin":
        base = os.path.expanduser("~/Library/Application Support")
    else:
        base = os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config")
    return os.path.join(base, "deadass", "config.toml")


def load_profile(path):
    triggers = tomllib.load(open(path, "rb")).get("triggers", {})
    for key, rule in triggers.items():
        missing = [field for field in FIELDS if field not in rule]
        if missing:
            sys.exit(f"{path}: trigger {key} lacks {missing}")
        if rule["pattern"] not in PATTERNS:
            sys.exit(f"{path}: trigger {key} has unknown pattern {rule['pattern']!r}")
        if not 0.0 <= rule["strength"] <= 1.0:
            sys.exit(f"{path}: trigger {key} strength must be within 0..1")
    return triggers


def toml_value(value):
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return repr(value)
    if isinstance(value, str):
        return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'
    if isinstance(value, list):
        return "[" + ", ".join(toml_value(item) for item in value) + "]"
    raise TypeError(type(value))


def table_key(key):
    return key if key.replace("_", "").isalnum() else f'"{key}"'


def render_triggers(triggers):
    out = []
    for key, rule in triggers.items():
        out.append(f"[triggers.{table_key(key)}]")
        subtables = {k: v for k, v in rule.items() if isinstance(v, dict)}
        out.extend(f"{k} = {toml_value(v)}" for k, v in rule.items() if k not in subtables)
        for name, sub in subtables.items():
            out.append(f"\n[triggers.{table_key(key)}.{name}]")
            out.extend(f"{k} = {toml_value(v)}" for k, v in sub.items())
        out.append("")
    return "\n".join(out)


def main():
    here = os.path.dirname(os.path.realpath(__file__))
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("profile", nargs="?", default=os.path.join(here, "support-profile.toml"))
    parser.add_argument("--config", default=default_config_path())
    args = parser.parse_args()

    profile = load_profile(args.profile)
    if not os.path.exists(args.config):
        sys.exit(f"{args.config} not found; start the companion once so it writes its config")
    raw = open(args.config, encoding="utf8").read()
    config = tomllib.loads(raw)
    triggers = config.setdefault("triggers", {})
    for key, rule in profile.items():
        triggers.setdefault(key, {}).update({field: rule[field] for field in FIELDS})

    # Top-level settings are kept verbatim; only the trigger tables are rewritten.
    cut = raw.find("\n[triggers.")
    head = raw if cut < 0 else raw[: cut + 1]
    rendered = head.rstrip("\n") + "\n\n" + render_triggers(triggers)
    if tomllib.loads(rendered) != config:
        sys.exit("re-rendered config does not round-trip; not writing")

    backup = f"{args.config}.{datetime.datetime.now():%Y%m%d-%H%M%S}.bak"
    shutil.copy2(args.config, backup)
    tmp = args.config + ".tmp"
    with open(tmp, "w", encoding="utf8") as f:
        f.write(rendered)
    os.replace(tmp, args.config)
    print(f"applied {len(profile)} triggers to {args.config}; backup at {backup}")


if __name__ == "__main__":
    main()
