# Bencher CLI AGENTS.md

## Skill Sync

When updating the `bencher run` CLI (adding, removing, or renaming flags, adapters, threshold models, or subcommands), also update the Bencher skill files in `plugins/bencher/skills/bencher/`. The key files to check:

- `plugins/bencher/skills/bencher/reference.md` - Complete flag and command reference
- `plugins/bencher/skills/bencher/local-runs.md` - Adapter table and run examples
- `plugins/bencher/skills/bencher/thresholds.md` - Threshold model table and flag docs
- `plugins/bencher/skills/bencher/bare-metal.md` - Bare metal flags and examples
- `plugins/bencher/skills/bencher/ci.md` - CI integration examples
- `plugins/bencher/skills/bencher/SKILL.md` - Auth docs and common options
