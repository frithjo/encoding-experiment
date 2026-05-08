# Follow-up: Carbonyl aarch64 artifact support

## Context

The Carbonyl lift plan (Option A + B) implemented Linux x86_64 only for v1. This ticket tracks adding aarch64 (ARM64) support.

## Work required

1. **Add aarch64 release URL to CARBONYL_VERSION**
   - Extend the format to support multiple platforms, or add a separate file like `CARBONYL_VERSION.aarch64`
   - Carbonyl releases include `carbonyl.linux-arm64.zip` for ARM64

2. **Update install script**
   - Detect host architecture via `uname -m`
   - Select appropriate download URL based on architecture
   - Verify SHA256 for the selected artifact

3. **Update CI**
   - Add aarch64 runner job (e.g., `ubuntu-latest-arm64` or equivalent)
   - Verify aarch64 artifact SHA256 in CI

4. **Update documentation**
   - `apps/terminal-runtime/README.md`: note aarch64 support
   - `crates/larql-terminal-browser/ONBOARDING.md`: add aarch64 install instructions

## Dependencies

- Carbonyl upstream must publish aarch64 releases
- CI runner with aarch64 support (GitHub Actions ARM runners are in beta)

## Acceptance

- Fresh checkout on aarch64 Linux: `./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh` downloads the correct ARM64 artifact
- SHA256 verification passes for aarch64 artifact
- CI job verifies aarch64 artifact on schedule
