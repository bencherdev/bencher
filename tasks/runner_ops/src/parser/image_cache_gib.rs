use bencher_json::RunnerResourceId;
use serde::Deserialize;

use super::spacing::missing;

/// The byte cap of a runner's image cache in whole GiB, a `u32` so that its bytes fit in a `u64`; 0 keeps no cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct ImageCacheGib(u32);

impl ImageCacheGib {
    pub fn bytes(self) -> u64 {
        u64::from(self.0) << 30
    }
}

/// Every runner with no image cache size.
pub fn image_cache_gib_problems(gibs: &[(RunnerResourceId, Option<ImageCacheGib>)]) -> Vec<String> {
    missing(gibs, "image_cache_gib")
}

#[cfg(test)]
pub fn gib(gib: u32) -> ImageCacheGib {
    ImageCacheGib(gib)
}
