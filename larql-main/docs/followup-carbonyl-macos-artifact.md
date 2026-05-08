# Follow-up: Carbonyl macOS artifact support

## Context

The Carbonyl lift plan (Option A + B) implemented Linux x86_64 only for v1. This ticket tracks adding macOS support (Intel and Apple Silicon).

## Work required

1. **Add macOS release URLs to CARBONYL_VERSION**
   - Carbonyl releases include `carbonyl.darwin-amd64.zip` (Intel)
   - Carbonyl releases include `carbonyl.darwin-arm64.zip` (Apple Silicon)
   - Extend format to support multiple platforms/architectures

2. **Update install script**
   - Detect host OS via `uname -s`
   - Detect architecture via `uname -m`
   - Select appropriate download URL (macOS Intel vs ARM64)
   - Verify SHA256 for the selected artifact

3. **Update CI**
   - Add macOS runner job (already exists for other tests)
   - Verify macOS artifact SHA256 in CI

4. **Update SIGWINCH handling in larql-tty-io**
   - Current SIGWINCH handler is Linux-only (`cfg(target_os = "linux")`)
   - macOS uses the same signal but may need different ioctl handling
   - Test on macOS runner

5. **Update documentation**
   - `apps/terminal-runtime/README.md`: note macOS support
   - `crates/larql-terminal-browser/ONBOARDING.md`: add macOS install instructions
   - `crates/larql-tty-io/README.md`: update platform support section

## Dependencies

- Carbonyl upstream must publish macOS releases (already does)
- CI macOS runner (already exists)
- macOS SIGWINCH ioctl compatibility testing

## Acceptance

- Fresh checkout on macOS (Intel and ARM64): `./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh` downloads the correct artifact
- SHA256 verification passes for macOS artifacts
- CI job verifies macOS artifacts on schedule
- `larql-tty-io` SIGWINCH handler works on macOS (tested via CI)
