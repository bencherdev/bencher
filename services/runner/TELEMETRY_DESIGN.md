# Bare Metal Runner Telemetry Design

Bencher's bare metal runners report host and process health to the API server over a lightweight UDP
telemetry sideband. This document describes that protocol. It complements the job-execution design
in [DESIGN.md](./DESIGN.md), which covers the runner architecture, the WebSocket control channel, and
the authentication model this builds on.

## Availability

This feature is available only on Bencher Plus Enterprise plans. It is currently distributed
privately; contact plus@bencher.dev for more details.

## Overview

Runners maintain a persistent WebSocket control channel (see
[DESIGN.md](./DESIGN.md#websocket-channel)) for job assignment, roughly 1/sec liveness heartbeats,
and usage billing. Telemetry is a separate, additive channel for host health (CPU, memory, load,
temperature, runner state, and lifetime counters) used for fleet observability. It is never the
source of truth for liveness or billing; those stay on the WebSocket.

Telemetry is a fire-and-forget UDP stream of periodic, self-contained snapshots. Occasional loss is
acceptable because the next snapshot follows within one interval. The protocol follows the UDP usage
guidelines of BCP 145 / RFC 8085: it sends no traffic when there is nothing to send, at most one
datagram per interval (far below one per RTT), randomized jitter to avoid fleet synchronization,
backoff when the collector is unreachable, and UDP checksums stay enabled.

```
   Runner Agent                               Bencher API
 ┌───────────────┐   WebSocket (control)   ┌──────────────────┐
 │ job execution │◀───────────────────────▶│ /v0/runners/.../ │
 │  (DESIGN.md)  │   jobs, heartbeat,      │     channel      │
 │               │   billing               │                  │
 │  ┌─────────┐  │                         │                  │
 │  │telemetry│  │   UDP datagram ~15s     │ ┌──────────────┐ │
 │  │ sampler │──┼────────────────────────▶│ │ UDP listener │ │
 │  └─────────┘  │   (host-health snapshot)│ └──────┬───────┘ │
 └───────────────┘                         └────────┼─────────┘
   housekeeping cores only                          ▼
   (benchmark cores untouched)              server metrics pipeline
```

## Design Principles

- **Observability, not control**: Telemetry never gates job execution, liveness, or billing. When
  the telemetry channel is down, nothing about job execution changes.
- **Self-contained snapshots**: Every datagram fully describes current state. Loss, duplication,
  reordering, and delay are tolerated, since the next snapshot supersedes a lost one.
- **Cheap to ignore**: The collector rejects junk (wrong magic or version, bad length) in a few bytes
  before any cryptographic work.
- **Reuse existing identity**: Authentication reuses the runner's identity and key (see
  [DESIGN.md](./DESIGN.md#authentication)). No new credential, no new provisioning, no new
  rotation or revocation path.
- **Never perturb measurement**: Sampling and sending run on the housekeeping cores (cores 0/1),
  never the isolated benchmark cores, matching the WebSocket heartbeat discipline.
- **Off by default**: Both listener and sender are opt-in via config.

## Relationship to the WebSocket Channel

| Aspect      | WebSocket control channel         | UDP telemetry sideband               |
| ----------- | --------------------------------- | ------------------------------------ |
| Purpose     | Job assignment, liveness, billing | Host-health observability            |
| Transport   | WebSocket (TLS), persistent       | UDP datagrams, connectionless        |
| Reliability | Ordered, reliable, ACKed          | Best-effort, no ACK, no retransmit   |
| Direction   | Bidirectional                     | Runner to server only                |
| Cadence     | ~1/sec heartbeat during a job     | ~1 per 15s, always (idle or active)  |
| Auth        | Bearer runner key                 | Per-datagram authentication tag      |
| When down   | Jobs fail or time out             | A gap in dashboards; jobs unaffected |

## Datagram Format

A datagram is a fixed 36-byte header followed by a CBOR payload, protected per the configured
security mode (see [Transport Security](#transport-security)). Multi-byte header fields are network
byte order.

### Header (36 bytes)

| Offset | Size | Field       | Notes                                                                |
| ------ | ---- | ----------- | -------------------------------------------------------------------- |
| 0      | 2    | magic       | `0x4254` ("BT"); collector drops anything else before crypto         |
| 2      | 1    | version     | Framing version (currently `1`); unknown version dropped and counted |
| 3      | 1    | type        | Message type (currently `1`, host-health snapshot)                   |
| 4      | 2    | flags       | bit0 encrypted, bit1 timed_out, bit2 thermal_throttle, bit3 final    |
| 6      | 2    | reserved    | Must be zero                                                         |
| 8      | 16   | runner_uuid | The runner's identifier (raw 16 bytes); demux key and auth identity  |
| 24     | 4    | epoch       | Random per process start; fences sequence across restarts            |
| 28     | 8    | sequence    | Monotonic per process; replay/dedup and nonce material               |

### Payload (CBOR map, integer keys, all optional)

Every field is optional, so senders emit only what they have and the collector tolerates any
subset. Fractional values are scaled integers to avoid floats on the wire. `state` maps to the
runner-state metric the server already tracks.

| Key   | Field                          | Type     | Notes                              |
| ----- | ------------------------------ | -------- | ---------------------------------- |
| 1     | timestamp_unix_ms              | uint     | Send time (freshness check)        |
| 2     | uptime_seconds                 | uint     |                                    |
| 3     | state                          | uint     | 0 idle, 1 active, 2 updating       |
| 4     | job_uuid                       | 16 bytes | Present iff active                 |
| 5     | agent_version                  | text     | semver, matches the server version |
| 7,8,9 | load_{1,5,15}min_milli         | uint     | loadavg times 1000                 |
| 10    | cpu_percent_centi              | uint     | 0 to 10000                         |
| 11    | mem_used_bytes                 | uint     |                                    |
| 12    | mem_total_bytes                | uint     |                                    |
| 13    | cpu_temp_milli_celsius         | uint     | sentinel means unavailable         |
| 14    | jobs_completed                 | uint     | lifetime                           |
| 15    | jobs_failed                    | uint     | lifetime                           |
| 16    | housekeeping_cpu_percent_centi | uint     | detects CPU isolation breaking     |

New metrics arrive as new keys; the collector ignores unknown keys (see
[Versioning](#wire-versioning-and-evolution)).

## Size and Fragmentation

Datagrams are capped at **1232 bytes** (the 1280-byte IPv6 minimum MTU minus the 40-byte IPv6 and
8-byte UDP headers) with the Don't-Fragment bit set. Telemetry never relies on IP fragmentation: a
snapshot that would exceed the cap drops lower-priority optional fields rather than splitting across
datagrams. A typical snapshot is about 150 bytes, so the cap is ample headroom and makes "never
fragment" structural.

## Reporting Cadence

- **Interval**: 15 seconds (configurable). Frequent enough to catch a thermal event or a wedged host
  well within the 90s WebSocket heartbeat-timeout window, and coarse enough that even 500 runners
  produce only about 33 datagrams/sec aggregate.
- **Jitter**: each interval waits a uniform plus or minus 20% (12 to 18s for a 15s interval),
  resampled every tick, so a fleet that boots from one image does not phase-lock onto the collector.
- **Backoff**: on a local send error or ICMP "unreachable," the sender applies decorrelated-jitter
  backoff, `min(300s, uniform(interval, 3x previous))`, reset on the first success.
- **No idle keepalive**: the loop is data-driven; a stopped agent sends nothing.

This is distinct from the control channel: the WebSocket reconnect uses a flat 5s plus 0 to 5s
jitter and the billing heartbeat is about 1/sec. Telemetry is slower, separate, and never
retransmits a specific snapshot, since a re-sent snapshot would already be stale.

## Authentication and Replay

Telemetry reuses the runner credential rather than introducing a new one. The per-runner telemetry
key is derived once:

```
K_tel = HKDF-SHA-256(
    ikm  = runner key,           // 30 alphanumeric chars after the bencher_runner_ prefix
    salt = "bencher-brtp-v1",
    info = runner identifier (16 bytes),
    len  = 32 bytes)
```

- The server derives and caches `K_tel` per runner **at key create/rotate time**, the only moment it
  holds the plaintext runner key (it otherwise stores only the key's SHA-256 hash). The runner
  derives the identical `K_tel` locally.
- The cleartext `runner_uuid` in the header selects the key with a single lookup, with no trial
  decryption.
- Each datagram carries a tag: an authenticated-encryption tag in encrypted modes, or a keyed MAC
  over header plus payload in MAC mode.
- **Replay**: the collector tracks the highest `(epoch, sequence)` per runner plus a small sliding
  window, and a 120s timestamp freshness window. Duplicate or stale datagrams are dropped, which is
  harmless because snapshots are idempotent.
- **Rotation and revocation are inherited**: rotating the runner key
  (`POST /v0/runners/{runner}/key`) changes `K_tel` atomically, instantly orphaning captured
  traffic; archiving a runner removes its key. Same lifecycle as
  [DESIGN.md](./DESIGN.md#authentication), with no new surface.
- **Keyed from the credential, not its hash**: `K_tel` is derived from the runner key itself, never
  from its stored SHA-256 hash, so a database read cannot forge telemetry. This keeps the "key
  itself never stored" property intact.

Dropped, unauthenticated, or replayed datagrams are counted in the server's metrics with a reason
attribute for visibility.

## Transport Security

Telemetry supports three security modes, all keyed by the same `K_tel` and selected by
configuration:

| Mode | Protection                             | Method                              | Use when                         |
| ---- | -------------------------------------- | ----------------------------------- | -------------------------------- |
| M    | Authenticity and integrity (cleartext) | Keyed MAC over the datagram         | Trusted or private network       |
| A    | Adds confidentiality                   | Authenticated encryption            | Untrusted network without DTLS   |
| D    | Adds server auth and forward secrecy   | DTLS 1.3, pre-shared key plus ECDHE | Bencher Cloud (internet-exposed) |

Bencher Cloud uses Mode D (DTLS 1.3). The runner authenticates with `K_tel` as a pre-shared key
(identity = the runner identifier) and validates the collector's certificate against the Web PKI or a
pinned fingerprint. The same `K_tel` and the same header and replay logic apply in every mode, so the
mode is purely a deployment choice.

## Wire Versioning and Evolution

Two independent levels:

- **Framing version** (1 byte): bumped only on a header-layout change. The collector keeps a decode
  ladder for known versions and drops and counts unknown ones.
- **Payload fields**: adding a metric needs no version bump. New senders include a new key; old
  collectors ignore unknown keys; old senders omit it and new collectors treat it as absent. Keys are
  never renumbered or reused.

Runner and server share a single version, and runners self-update to the server's version between
jobs (see [DESIGN.md](./DESIGN.md#self-update)), so version skew across the fleet is transient: a
single poll cycle during a rolling deploy, not indefinite. The channel is one-way, so it carries no
in-band negotiation; a minimum-version requirement is signaled out of band over the WebSocket, which
already carries the runner's version.

## Server-Side Ingestion

A single background task binds one UDP socket once the server is running. For each datagram it checks
the magic, version, and length, looks up `K_tel` by the `runner_uuid` in the header, verifies the
tag, applies the replay check, and updates an in-memory per-runner view, then feeds the server's
existing metrics pipeline. There is no per-datagram database write and no new storage system. The
listener never replies on the data path, so it cannot be used as a UDP amplifier.

## Deployment Scope

Bare-metal runners are dedicated, CPU-pinned Firecracker hosts (see [DESIGN.md](./DESIGN.md)), so
fleets are curated, not elastic: tens to low hundreds on Bencher Cloud, typically 1 to 20
self-hosted. Runners send outbound-only UDP to one collector address and port, friendly to NAT and
egress firewalls, and because identity is in the authenticated header rather than the 5-tuple, a NAT
rebinding does not disrupt demux or auth. A single listener handles the whole fleet with orders of
magnitude of headroom. The feature is gated to Bencher Plus and off by default.

## Configuration

Telemetry settings live in the runner configuration:

| Field                | Value        | Notes                          |
| -------------------- | ------------ | ------------------------------ |
| enabled              | on/off       | Off by default                 |
| bind                 | address:port | Where the collector listens    |
| report_interval_secs | seconds      | Default 15                     |
| mode                 | M / A / D    | Security mode                  |
| server_cert          | certificate  | For Mode D (pinned or Web PKI) |

## Host Metrics Source

Snapshots are assembled from host metrics the runner already collects: CPU, memory, load average,
and temperature. Sampling runs on the housekeeping cores so the isolated benchmark cores are never
touched.

## Design Decisions

- **Telemetry is not liveness or billing**: it rides a separate channel so observability can be lossy
  without risking job correctness or billing accuracy.
- **UDP rather than a second WebSocket**: fire-and-forget periodic snapshots want no head-of-line
  blocking, no per-report ACK, and minimal cost on housekeeping cores.
- **CBOR rather than a bespoke binary body**: additive, forward and backward compatible evolution
  for free.
- **A key derived from the runner credential rather than a new credential**: zero new provisioning,
  inheriting the existing rotation and revocation story.
