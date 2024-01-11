#[derive(Debug, Copy, Clone, Default)]
pub enum BillingCycle {
    Monthly,
    #[default]
    Annual,
}

impl From<BillingCycle> for i64 {
    fn from(billing_cycle: BillingCycle) -> Self {
        // 24 hours/day * 60 minutes/hour * 60 seconds/hour
        const SECONDS_PER_DAY: i64 = 24 * 60 * 60;
        match billing_cycle {
            // 31 days in the longest month
            BillingCycle::Monthly => 31 * SECONDS_PER_DAY,
            // 366 days in the longest year
            BillingCycle::Annual => 366 * SECONDS_PER_DAY,
        }
    }
}
