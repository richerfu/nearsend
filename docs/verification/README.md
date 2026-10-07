# OHOS 1.2.1 verification

The current implementation uses unmodified GPUI and the local gpui-ohos
platform adapter. Start with these final reports:

- [OHOS-only Ability implementation](2026-10-07-ability-ohos-only.md)
- [Optimized gpui-ohos delivery](2026-10-06-gpui-optimized.md)
- [System Pan integration](2026-10-06-system-pan.md)
- [Phone viewport correction](2026-10-06-phone-viewport.md)
- [First accepted Pan displacement](2026-10-06-pan-latency.md)
- [Approved publishrelease APP](2026-10-06-publishrelease.md)
- [gpui-ohos PR82 branch synchronization](2026-10-06-pr82-sync.md)

Earlier dated reports record intermediate candidates and comparisons. Their
implementation details and artifact names describe those historical builds.

Small structured results are tracked under `evidence/`. Screenshots, raw logs,
device dumps and signed packages stay local and are ignored by Git. Links to
those local files are intended for reviewing the verification workspace.

The host gesture regressions can be repeated with:

```sh
cargo test --manifest-path scripts/ohos-touch-tests/Cargo.toml --locked
cargo clippy --manifest-path scripts/ohos-touch-tests/Cargo.toml --locked --all-targets --no-deps -- -D warnings
```

Use `scripts/verify_ohos_scroll.py` for simulator checks and
`scripts/measure_ohos_cpu.py` for process CPU sampling. Both expect `hdc` on PATH.
Simulator screen coordinates are recorded for the tested layouts; adjust them
when the floating window or display size changes.
