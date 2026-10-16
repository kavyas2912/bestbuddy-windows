# Device acceptance

Status: not run on a Windows PC in this cleanup session.

1. Install the candidate MSI/EXE on a clean device and launch it. Check dashboard layout, navigation and persistence after restart.
2. Review consent controls. With microphone consent off, confirm recording stays off. Grant system microphone permission and voice consent deliberately.
3. Confirm the complete model pack is available. Confirm the installer contains its own models; source-tree models must not mask missing installed files.
4. Record a five-second microphone sample using the app diagnostic. Save and replay the WAV. Record whether input frames, audible playback and transcription succeed.
5. Test a short spoken command, the composer response and TTS playback. Test Hey Buddy, silence and interruption during playback. Record the first failing stage and app logs.
6. Test reminders across midnight, a lead-day reminder and a leap-day anniversary. Restart and check that already delivered reminders do not repeat.
7. Test optional news/weather/market/music/messenger integrations only with their required network access, accounts and consent. A visible control does not prove integration success.
8. Test personal-data export and erase, then restart to confirm the result.

For Store distribution, verify valid signatures, stable hosted installer URLs and Partner Center validation before submission. PUBLISH_MS_STORE_WINDOWS.ps1 changes a remote Store draft even without -Submit; run it only when intentionally updating that draft. -Submit additionally requests publication.

Record the artifact SHA-256, OS/device, version, prerequisites and pass/fail result for each stage. Do not describe the release as accepted before this checklist has evidence.
