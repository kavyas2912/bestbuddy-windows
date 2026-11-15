# R6 — saving and offline setup · 20 October 2026

Each onboarding stage has Save stage and Next. Save stage writes an encrypted resumable draft on this device; passwords and API keys are excluded. Next also saves when profile storage is enabled. Final Review saves the profile with an app password of at least eight characters. Optional permissions and additional models can remain off.

No additional model is a valid choice. Built-in commands, phrasebook and knowledge responses remain available offline; these do not equal a generative language model. Online fallback is an explicit choice, independent of saved API keys. Editing an existing profile retains its model choices.

The compatible download list contains Qwen, SmolLM2 and Phi GGUF choices up to 3 GB. Live Hugging Face download counts order this curated compatible list; popularity is not quality ranking and the list is not an exhaustive search of every Hub model. Downloads are pinned and checksum verified. Tokens are optional for public models.

Desktop discovery checks running Ollama (11434), LM Studio (1234) and llama.cpp (8080), plus GGUF files in standard Hugging Face / LM Studio caches. It does not launch those apps, scan every disk or establish model provenance. Custom service ports are not detected. Cached files are not copied.

Language and voice preferences are saved. English remains the bundled fallback. Optional voices must be downloaded before use. A selected voice that is not installed is retained as a preference and does not replace the current installed voice. English recognition is the baseline; arbitrary multilingual recognition is not implemented.

Android includes a light onboarding voice and the full English recognition/speaking fallback archive (about 250 MB before compression), independent of optional language downloads. Native local generative inference is not enabled. Android native compilation is checked; real phone audio, battery use and latency still require acceptance testing.
