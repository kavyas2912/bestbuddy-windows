//! Explicit CPU-first budgets; total system RAM is not free application memory.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceBudget {
    pub tier: &'static str,
    pub language: &'static str,
    pub inference_threads: usize,
    pub context_tokens: usize,
    pub reply_tokens: usize,
    pub context_chars: usize,
    pub retrieval_hits: usize,
    pub completion_seconds: u64,
}
impl ResourceBudget {
    pub fn for_memory_gb(gb: u64, cores: usize) -> Self {
        let standard = gb >= 8;
        Self {
            tier: if standard {
                "standard-8gb"
            } else {
                "compact-4gb"
            },
            language: "en",
            inference_threads: cores
                .saturating_sub(1)
                .clamp(1, if standard { 4 } else { 2 }),
            context_tokens: if standard { 4096 } else { 2048 },
            reply_tokens: if standard { 192 } else { 128 },
            context_chars: if standard { 3600 } else { 1600 },
            retrieval_hits: if standard { 4 } else { 2 },
            completion_seconds: 30,
        }
    }
    pub fn from_env() -> Self {
        let gb = std::env::var("BUDDY_MEMORY_GB")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4);
        Self::for_memory_gb(
            gb,
            std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(2),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budgets_leave_a_core_and_bound_small_devices() {
        let small = ResourceBudget::for_memory_gb(4, 8);
        assert_eq!(small.inference_threads, 2);
        assert_eq!(small.context_tokens, 2048);
        assert_eq!(ResourceBudget::for_memory_gb(8, 1).inference_threads, 1);
        assert_eq!(ResourceBudget::for_memory_gb(8, 8).inference_threads, 4);
    }
}
