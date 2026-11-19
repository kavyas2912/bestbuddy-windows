# BestBuddy Windows 1.6.26 — private test

Install with the ordinary x64 setup EXE, or extract the complete portable ZIP and run BestBuddy.exe. This is a private test package, not the older unsigned Microsoft Store MSIX. Keep the included models and runtime folders beside the portable executable. The package includes Qwen3-1.7B, Whisper base.en and the Piper LJSpeech voice; no model download, API key or model selection is required for the default English conversation.

The executable icon is the approved Macaw, verified from the compiled PE resource. Launch plays a short original Macaw-style chirrup before the spoken greeting and briefing offer. Quiet mode suppresses them. Existing news, Market Watch, Day Book, briefing, media, safety choices and consent controls are retained.

Conversation keeps bounded dialogue context, retrieves attributed recent public sources, and avoids asking an identical recent model question. Delivered entertainment is recorded across restarts and recalled only on an explicit request. An exhausted finite library is reported rather than automatically replayed. Semantic uniqueness of arbitrary generated model text is not guaranteed.

Native regression results: 64 passed; 10 hardware/service checks ignored. TypeScript checking, production frontend build, frontend regression checks and release preflight pass. The actual CPU model remembers a supplied hobby in a two-turn test; actual synthesized voice was decoded through Whisper. These checks do not replace listening and microphone acceptance on the target computer. Browser screenshots verify layout only.

The setup EXE is not publisher-signed. Windows may show an unknown-publisher or SmartScreen prompt. The portable ZIP offers testing without installing an MSIX certificate. WebView2 is required; the installer can fetch Microsoft's runtime when missing. First installation on a computer without WebView2 therefore needs internet for that Windows component, while the included companion models need no download.

Source builds: run tools/BUILD_PRIVATE_WINDOWS.ps1. DEFAULT_RESOURCES.json and tools/restore_defaults.py restore and verify the large external assets. Source archives omit their large payloads and all private signing material. See THIRD_PARTY_NOTICES.md and licenses/ for third-party terms.
