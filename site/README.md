# agentic-navigation-guide site

Static Astro/Starlight site generated from `static-tool-page-template`.

## Common commands

```sh
just install
just dev
just check
just test
just build
```

The site is configured for `https://plx.github.io/agentic-navigation-guide/` with the GitHub Pages base path `/agentic-navigation-guide`.

The generated Playwright suite runs against mobile, tablet, and desktop projects.
Use `just install-browsers` once locally before `just test`.
If port 4321 is already occupied, run tests with another port:

```sh
SITE_TEST_PORT=54321 just test
```
