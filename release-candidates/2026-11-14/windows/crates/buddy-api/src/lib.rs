use buddy_core::BuddyEngine;
use buddy_voice::{VoiceConfig, VoiceEngine};
pub struct BuddyRuntime {
    pub core: BuddyEngine,
    pub voice: VoiceEngine,
}
impl BuddyRuntime {
    pub fn new(core: BuddyEngine) -> Self {
        Self {
            core,
            voice: VoiceEngine::new(VoiceConfig::from_env()),
        }
    }
}
