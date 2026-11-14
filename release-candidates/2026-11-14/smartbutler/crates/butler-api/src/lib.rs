use butler_core::ButlerEngine;
use butler_voice::{VoiceConfig, VoiceEngine};
pub struct ButlerRuntime {
    pub core: ButlerEngine,
    pub voice: VoiceEngine,
}
impl ButlerRuntime {
    pub fn new(core: ButlerEngine) -> Self {
        Self {
            core,
            voice: VoiceEngine::new(VoiceConfig::from_env()),
        }
    }
}
