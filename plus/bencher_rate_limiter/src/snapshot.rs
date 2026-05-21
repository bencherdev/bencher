use std::{collections::HashMap, hash::Hash};

use serde::{Deserialize, Serialize};

pub type EpochBucket = u64;

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowSnapshot<K: Eq + Hash> {
    pub events: HashMap<K, Vec<(EpochBucket, u32)>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimiterSnapshot<K: Eq + Hash> {
    pub minute: WindowSnapshot<K>,
    pub hour: WindowSnapshot<K>,
    pub day: WindowSnapshot<K>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BandwidthSnapshot<K: Eq + Hash> {
    pub events: HashMap<K, Vec<(EpochBucket, u64)>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_snapshot_round_trip() {
        let mut events = HashMap::new();
        events.insert(42u32, vec![(100, 5), (101, 3)]);
        let snapshot = WindowSnapshot { events };
        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: WindowSnapshot<u32> = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.events[&42], [(100, 5), (101, 3)]);
    }

    #[test]
    fn rate_limiter_snapshot_round_trip() {
        let snapshot = RateLimiterSnapshot::<u32> {
            minute: WindowSnapshot {
                events: HashMap::new(),
            },
            hour: WindowSnapshot {
                events: HashMap::new(),
            },
            day: WindowSnapshot {
                events: HashMap::from([(1, vec![(50, 10)])]),
            },
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: RateLimiterSnapshot<u32> = serde_json::from_str(&json).unwrap();
        assert!(restored.minute.events.is_empty());
        assert_eq!(restored.day.events[&1], [(50, 10)]);
    }

    #[test]
    fn bandwidth_snapshot_round_trip() {
        let snapshot = BandwidthSnapshot::<u32> {
            events: HashMap::from([(1, vec![(100, 1024), (101, 2048)])]),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: BandwidthSnapshot<u32> = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.events[&1].len(), 2);
        assert_eq!(restored.events[&1][0], (100, 1024));
        assert_eq!(restored.events[&1][1], (101, 2048));
    }
}
