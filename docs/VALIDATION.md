# Validation — BestBuddy 0.8.0 candidate

16 October 2026. These checks apply to the new local candidates, not the historical 0.6.12 ZIPs.

## Passed

- Windows locked/offline workspace tests: **70 passed, 10 ignored, zero failures**, including core/intelligence integration, retrieval rebuild/clear, long-paragraph retrieval, final-response repeat/history, context limits and conversation cancellation/deadline/cleanup tests. Documentation tests also completed successfully.
- Windows workspace/all-targets compilation was checked during integration; final workspace tests rebuilt the changed host.
- Android arm64 locked/offline workspace library check: passed using installed NDK 30.0.16248370. This is Rust cross-compilation checking, not an APK build/link or a device run.
- Windows and Android frontend TypeScript checks, production builds and reminder regressions passed independently after the voice cancellation-ordering change.
- Rust workspace formatting checks passed for both candidates.
- No new Rust runtime packages or dependency-version upgrades were introduced; existing Cargo/npm lockfiles were retained with owned version metadata updated.

## Environment corrections

Initial Windows persistence tests could not access the user encryption store from the sandbox. They passed when run with Windows user encryption access; no encryption bypass was added. An initial new retrieval fixture was below the existing minimum transcript length; the fixture was corrected. The Android compiler was initially denied by the sandbox and was rerun with access to the installed NDK. A new asynchronous Tauri command return-type error was corrected and checked again on both targets.

## Warnings and untested areas

- Windows native speech linkage still reports LNK4098 (conflicting CRT defaults), as in the baseline. It was not suppressed.
- Android retains the existing unused log-directory variable warning.
- Ignored tests require model assets or live services. Model-backed sample-audio tests were not rerun: source packages exclude model weights, and some existing model directories were inaccessible. Historical voice checks are preserved separately and are not current acceptance evidence.
- No signed installer, release APK, Gradle packaging, model-pack staging or store validation was performed in this pass.
- Real microphone/speaker, wake phrase, interruption, lifecycle/permissions, model-output quality, 4 GB/8 GB peak memory, sustained performance and energy measurements remain for user testing.
- Android-native generative inference and token streaming remain unimplemented; see the implementation notes. Offline analyzer/retrieval/voice functionality is the candidate's Android core path.

Source manifests and archive checksums identify the deliverables. Baseline file hashes are rechecked during packaging to verify the original sources remain unchanged.
