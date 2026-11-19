# BestBuddy included offline resources — 17 November 2026

Default conversation: Qwen3-1.7B, Apache-2.0. Android LiteRT-LM conversion revision d6678881da6181864bbc893c80e10aa1ee0d3731; desktop GGUF revision daeb8e2d528a760970442092f6bf1e55c3b659eb. Original model: https://huggingface.co/Qwen/Qwen3-1.7B

Speech recognition: OpenAI Whisper base.en, MIT; Sherpa-ONNX int8 conversion. Speech synthesis: Piper LJSpeech high, US English female, conversion revision 6356d891ee71f4bed94a2b44ec29c010c39c0733. The LJSpeech dataset is public domain. This is a trained synthetic voice, not an accent-free human recording. Model card: https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_US/ljspeech/high/MODEL_CARD

Inference: Sherpa-ONNX 1.13.8 (Apache-2.0), ONNX Runtime (MIT), llama.cpp b12428 (MIT), LiteRT-LM (Apache-2.0). Piper phonemization wrapper is MIT; eSpeak-NG code/data carry GPL-3.0 terms. Their licence texts accompany this distribution. Retain these notices and the corresponding upstream source when redistributing the speech binaries/data. The app source, build scripts and checksum-pinned resource restoration manifest accompany the test package.

Corresponding speech source and build definitions:
- https://github.com/k2-fsa/sherpa-onnx/tree/v1.13.8
- https://github.com/csukuangfj/piper-phonemize/tree/f3ff95afc03640bc1399e113e83361192a2fafb4
- https://github.com/espeak-ng/espeak-ng
- https://github.com/microsoft/onnxruntime
- https://github.com/ggml-org/llama.cpp/tree/b12428
- https://github.com/google-ai-edge/LiteRT-LM

Existing optional wake-word, denoising and VAD resources retain their upstream licences and provenance in tools/voice-model-manifest.json. Lessac and Kokoro benchmark resources are not the included default voice. The Macaw-style welcome chirrups are original synthesized audio created for this app. The Macaw launcher art is the approved project asset.

The default English conversation and speech do not require an account, API key or runtime model download. Online research remains separately consent controlled. Other languages may use explicitly selected system voices and existing advanced providers. Third-party software is supplied under its own licence and warranty terms.
