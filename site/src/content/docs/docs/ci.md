---
title: CI and Hooks
description: Using agentic-navigation-guide in automation.
---

Use `verify` in CI to prevent stale navigation guides from merging.

This example pins third-party actions to immutable commit SHAs. Update them
intentionally when upgrading those actions.

```yaml
name: Verify Navigation Guide

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - uses: actions-rust-lang/setup-rust-toolchain@46268bd060767258de96ed93c1251119784f2ab6 # v1.16.1
      - run: cargo install agentic-navigation-guide
      - run: agentic-navigation-guide verify --github-actions-check
```

The repository also uses a local verification workflow that builds the crate and runs:

```sh
cargo run --release -- verify --github-actions-check
```

For agent workflows, the same verification can run after file-writing tools so navigation drift is caught while the work is still fresh.
