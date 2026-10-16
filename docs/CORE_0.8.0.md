# BestBuddy 0.8.0 — English core candidate

Prepared 16 October 2026. This is a local source candidate, not a signed installer, APK, or device-accepted release. This candidate is published from a clean source snapshot, without local repository history, caches, model weights or personal application data.

## Baseline and scope

The later cleaned Windows and Android 0.6.12 source folders were used. They differ from the older same-version archives, including the addition of `buddy-intelligence`. The current source manifest identifies this published snapshot. Models, dependency caches and generated build output are excluded.

Existing capability/plugin implementations and UI layout remain in place. Plugin development is deferred. Workspace crate versions advance together to 0.8.0; embedded plugin manifest versions remain 0.6.12. Existing registration does not enforce compatibility by app version. The Windows preflight was adjusted to allow those independent manifest versions.

## Implemented

- English, CPU-first resource profiles: conservative 4 GB default, opt-in 8 GB profile through `BUDDY_MEMORY_GB=8` before launching the Windows app. Android uses the conservative profile unless configured by its launch environment. Total RAM is a target class, not a guarantee of available memory.
- Compact profile: at most two inference threads while leaving a core where possible, 2,048-token local model context, 128 output tokens, 1,600-character serialized reference context and two retrieval sources. Standard profile: at most four threads, 4,096-token model context, 192 output tokens, 3,600-character references and four sources. The tokenizer is runtime-owned; character limits are not exact token counts.
- Cached normalized bundled-knowledge lookup, preserving its existing first-match semantics.
- Cached lexical retrieval with BM25 scoring, conservative term-coverage gates, deterministic ranking and one excerpt per document in model reference context. No embedding model is necessary.
- Overlapping excerpts for long paragraphs, bounded to 200 documents and 128 excerpts per document. An excerpt contains at most 800 Unicode characters. Documents beyond those index limits are not searched; the existing transcript import limit is also 200 documents.
- Retrieval prewarming at startup/import; invalidation on transcript replacement, clear and consent-related removal. Private learned transcripts are eligible only with both learning and personalization consent.
- Bounded local conversation history and analyzer intent/tone metadata in local model context. Ordinary history and private learned transcripts are excluded from online provider context. The current user message still goes to the online provider when the user has configured online completion.
- Final model replies replace composer drafts in repeat/history state. This fixes repeating a draft that the user never saw.
- Shared response composition for typed conversation, live voice (through the existing frontend command), WAV diagnostics and manual roundtrip diagnostics.
- One active conversation, no unbounded conversation queue, a 30-second whole-turn deadline, cancellation and local-model process cleanup. Deadline/interruption cleanup happens before releasing the conversation slot. Talk/wake/interruption supersede pending model work. The UI discards stale voice replies.
- JSON-preserving reference-context limits and bounded completion length. Explicit actions and attributed deterministic/retrieval answers retain priority over model fallback.
- Android learning-panel iteration compatibility fix, required for its TypeScript build.

## Voice and model pack

The existing staged English pack remains the baseline: Whisper base.en int8 recognition, Piper Lessac medium speech output, Silero VAD, Zipformer Hey Buddy keyword spotting and GTCRN denoising. The setup/staging tools verify the existing pinned hashes. These source candidates do **not** contain the model weights or native dependency archives. Run the platform setup to obtain and stage them before packaging. Do not interpret a source ZIP as an installer with bundled models.

The pinned sherpa-onnx 1.13.8 was the latest published release verified during this work. Native libraries must match the Rust bindings. Published download links:

- [sherpa-onnx 1.13.8 compiled runtime assets](https://github.com/k2-fsa/sherpa-onnx/releases/tag/v1.13.8)
- [Current sherpa-onnx release listing](https://github.com/k2-fsa/sherpa-onnx/releases)
- [Whisper base.en model archive](https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.en.tar.bz2)
- [Piper Lessac English model archive](https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_US-lessac-medium.tar.bz2)
- [Zipformer keyword model archive](https://github.com/k2-fsa/sherpa-onnx/releases/download/kws-models/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01.tar.bz2)
- [Silero VAD](https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx)
- [GTCRN denoising model](https://github.com/k2-fsa/sherpa-onnx/releases/download/speech-enhancement-models/gtcrn_simple.onnx)
- [Optional compact Qwen3 0.6B GGUF weights](https://huggingface.co/Qwen/Qwen3-0.6B-GGUF)
- [Optional Qwen3 1.7B GGUF weights](https://huggingface.co/ggml-org/Qwen3-1.7B-GGUF)
- [Current compiled llama.cpp runtime releases](https://github.com/ggml-org/llama.cpp/releases)

The existing Windows preparation flow selects an official CPU runtime asset with an upstream SHA-256 digest and verifies model hashes. Larger models remain optional; the default engine can analyze, retrieve and speak without a local generative model. Model licenses and redistribution conditions must be checked before shipping a model pack.

## Important remaining limits

- Android has offline voice, analyzer, deterministic conversation and retrieval. The supplied baseline has no embedded Android GGUF runtime. The candidate explicitly rejects the desktop-model preparation path on Android. Native on-device generative replies require a separate Android-native inference integration; downloading a desktop llama-server does not provide that integration.
- Token streaming, sentence-by-sentence generative TTS, embedding-based semantic search and speculative decoding are not implemented in this candidate. Existing complete-response delivery is retained.
- Resource profiles are engineering limits, not measured 4 GB/8 GB performance certification. No GPU/NPU backend is required or automatically selected. Sustained memory, thermal behavior and battery use require your target hardware.
- The existing 0.6.12 validation records are historical. Current checks are recorded separately in `VALIDATION_0.8.0.md`.
- Device microphone, audible playback, wake reliability, accents, interruption, Android lifecycle, permissions and installer-contained assets remain for your machine testing. No signed distribution artifacts or store submissions were produced.

## Device check sequence

1. Extract the candidate into a short path. Run `tools/SETUP_WINDOWS.cmd` or `tools/SETUP_ANDROID.cmd` for its platform. These are build/setup actions, not publication approval.
2. Launch with no optional generative model. Grant microphone and voice-processing consent deliberately; confirm English wake, recognition and speech output.
3. Ask a bundled factual question, an arithmetic question and a follow-up. Import a permitted transcript; ask a grounded question and inspect its source attribution. Clear learned data and confirm the answer disappears.
4. On Windows, optionally prepare the 0.6B model, check an open conversation and “repeat that,” then interrupt with Talk or the wake phrase. Test the 1.7B option only after checking free memory on an 8 GB device.
5. Record peak application memory, cold/warm response times and behavior after a sustained session. Confirm profile export/erase and restart behavior.

## Research direction

The first optimization is reducing unnecessary work through caching, bounded context and deterministic routes. [BM25](https://nlp.stanford.edu/IR-book/html/htmledition/okapi-bm25-a-non-binary-model-1.html) supplies the lexical ranking basis. Later experiments can evaluate [llama.cpp speculative decoding](https://github.com/ggml-org/llama.cpp/blob/master/docs/speculative.md) and the energy/thermal questions raised by [PELM, September 2026](https://arxiv.org/abs/2609.09662). Their reported gains are not BestBuddy benchmark results.
