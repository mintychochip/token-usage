## What

<!-- What does this PR change? -->

## Why

<!-- Motivation, context, or `Fixes #N`. -->

## Testing

<!-- Commands you ran, fixtures you used, or why tests were not added. -->

```bash
cargo test --workspace
```

## Checklist

- [ ] Tests cover the shipped path (adapter/store/API), not a re-implementation
- [ ] Host wrappers still exec `token-usage-reporter` (if plugins changed)
- [ ] Living spec updated when identity, merge, or harness set changed
- [ ] `cargo fmt --all` and `cargo test --workspace` pass
