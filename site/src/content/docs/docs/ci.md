---
title: CI and Hooks
description: Using agentic-navigation-guide in automation.
---

Use `verify` in CI to prevent stale navigation guides from merging.

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
      - uses: actions/checkout@v4
      - uses: actions-rust-lang/setup-rust-toolchain@v1
      - run: cargo install agentic-navigation-guide
      - run: agentic-navigation-guide verify --github-actions-check
```

The repository also uses a local verification workflow that builds the crate and runs:

```sh
cargo run --release -- verify --github-actions-check
```

For agent workflows, the same verification can run after file-writing tools so navigation drift is caught while the work is still fresh.
