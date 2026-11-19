# BestBuddy native VoiceEngine comparison — minimal integration

**Target:** BestBuddy 0.6.12 (windows). This patch is intentionally narrow and is not a complete port of Python VoiceEngine.

## Existing implementations preserved

The application already has native Rust speech handling (sherpa-onnx ASR/VAD/KWS/TTS, microphone/audio pipeline, always-on mode, interruption), local phrasebook and profile memory, personality cues, prefetch, retrieval from cached sources, plugin command dispatch, and optional local/remote LLM fallback. A Python helper in production would add runtime and distribution complexity without proving better audio latency. No speech-model architecture, timings or platform build scripts were replaced.

## Applied

- Shared `buddy-core::offline_qa` ports a deliberately small, deterministic subset of Python VoiceEngine's capital-city knowledge and spoken arithmetic. No Python runtime or model download required. Preserves the prior command/action routing and LLM fallback for unrecognized questions.
- Always-on microphone startup, manual TALK, mic diagnostic, roundtrip and delivered voice events now check completed onboarding **and** explicit microphone **and** voice-processing consent. Revocation stops capture and discards queued events.
- Registration exposes both consent choices; the splash does not delay 12 seconds on a missing voice permission.

## Existing limitations, not silently 'fixed'

- This source ZIP contains **no downloaded speech model weights or native speech libraries**. Use the existing platform setup/build tools to prepare them. Voice requires a real microphone and speaker for end-to-end validation.
- The uploaded VoiceEngine FIX7 ZIP is a **patch/integration overlay**, not a complete standalone install: its `voiceengine/smart.py` imports analysis/prefetch modules absent from that archive. A wholesale feature-parity evaluation cannot be justified from that ZIP alone.
- Android's existing `buddy-core::protect_user_bytes` returns an explicit 'encrypted personal-data storage is not implemented' error on non-Windows platforms. Therefore **Android saved onboarding is not functional** without a separate secure-storage implementation. The privacy failure has not been bypassed with plaintext. Android also prompts for OS RECORD_AUDIO permission in its existing MainActivity at startup, even though Rust capture is now gated by in-app consent.
- Neither executable nor speech-device end-to-end latency was measured in this environment. No claims of improved 200–400 ms voice latency are made.

## Minimal Windows 11 smoke test

From an extracted Windows source ZIP, run `tools\SETUP_WINDOWS.cmd`, then `tools\RUN_WINDOWS.cmd`. Enter profile, select both microphone/voice-processing choices, finish onboarding, and confirm the voice runtime begins; without consent TALK and microphone diagnostic must refuse. Test voice: “What is the capital of France?”, “What is 7 plus 5?”, “How is the weather today?”, and interruption during playback. Revoke consent from Settings and confirm listening stops. Check the diagnostics and `logs` if any step fails.

**Build validation status:** This session had Node.js but no Rust/Cargo toolchain or Windows audio hardware. Only source-level/static checks can be made here; Windows and Android native builds are **not** claimed to have passed for this patch. The previous 0.6.12 validation report applies only to the unmodified baseline.
