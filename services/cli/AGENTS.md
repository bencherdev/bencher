# Bencher CLI CLAUDE.md

## Skill Sync

When updating the `bencher run` CLI (adding, removing, or renaming flags, adapters, threshold models, or subcommands), also update the Bencher skill files in `skills/bencher/`. The key files to check:

- `skills/bencher/reference.md` - Complete flag and command reference
- `skills/bencher/local-runs.md` - Adapter table and run examples
- `skills/bencher/thresholds.md` - Threshold model table and flag docs
- `skills/bencher/bare-metal.md` - Bare metal flags and examples
- `skills/bencher/ci.md` - CI integration examples
- `skills/bencher/SKILL.md` - Auth docs and common options
