# BestBuddy maintenance

- Preserve the existing user-facing features and UI unless a requested change requires otherwise.
- Use committed Cargo and npm lockfiles. Keep build output, models, SDK paths, service keys and signing material out of Git.
- Run Rust formatting, relevant unit tests, TypeScript checking, the production frontend build and applicable platform preflight before asking for device acceptance.
- Report compilation, sample-audio testing and device acceptance separately. Do not claim voice or external services work from compilation alone.
- Preserve the supplied original source and previous release artifacts. New archives must not overwrite earlier candidates.
- Shared Rust/frontend changes need equivalent fixes in the companion platform repository.
