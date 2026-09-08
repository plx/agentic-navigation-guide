// Shared metadata, navigation, and landing-page content.
// prettier-ignore
export const siteConfig = {
  "repository": {
    "owner": "plx",
    "name": "agentic-navigation-guide",
    "url": "https://github.com/plx/agentic-navigation-guide",
    "defaultBranch": "main"
  },
  "project": {
    "name": "agentic-navigation-guide",
    "title": "agentic-navigation-guide",
    "packageName": "agentic-navigation-guide-site",
    "category": "CLI",
    "tagline": "Check paths in repository guides.",
    "description": "A CLI that checks the paths in hand-maintained repository guides against the filesystem."
  },
  "site": {
    "host": "https://plx.github.io",
    "basePath": "/agentic-navigation-guide",
    "url": "https://plx.github.io/agentic-navigation-guide/",
    "dir": "site",
    "language": "en"
  },
  "theme": {
    "accent": "#a86a4a",
    "accent_2": "#3f6b57",
    "ink": "#1c1c1c",
    "surface": "#faf7f2",
    "muted": "#5a6773",
    "code": "#1a1d1b"
  },
  "landing": {
    "nav": [
      {
        "label": "Example",
        "href": "#cli"
      },
      {
        "label": "Docs",
        "href": "docs/"
      },
      {
        "label": "GitHub",
        "href": "https://github.com/plx/agentic-navigation-guide"
      }
    ],
    "footerLinks": [
      {
        "label": "GitHub",
        "href": "https://github.com/plx/agentic-navigation-guide"
      },
      {
        "label": "crates.io",
        "href": "https://crates.io/crates/agentic-navigation-guide"
      },
      {
        "label": "Full reference",
        "href": "https://github.com/plx/agentic-navigation-guide/blob/main/docs/v0.2-contract.md"
      }
    ],
    "primaryCta": {
      "label": "Read the docs",
      "href": "docs/"
    },
    "secondaryCta": {
      "label": "Source on GitHub",
      "href": "https://github.com/plx/agentic-navigation-guide"
    },
    "terminal": {
      "title": "AGENTIC_NAVIGATION_GUIDE.md",
      "copy": "agentic-navigation-guide verify",
      "lines": [
        "<agentic-navigation-guide>",
        "- src/",
        "  - parser.rs # Guide syntax",
        "  - verifier.rs # Filesystem checks",
        "- Cargo.toml",
        "</agentic-navigation-guide>",
        "",
        "$ agentic-navigation-guide verify",
        "✓ Navigation guide is valid and matches filesystem"
      ]
    }
  },
  "docs": {
    "sidebar": [
      {
        "label": "Guide",
        "items": [
          {
            "label": "Overview",
            "slug": "docs"
          },
          {
            "label": "Commands",
            "slug": "docs/commands"
          },
          {
            "label": "Guide format",
            "slug": "docs/guide-format"
          },
          {
            "label": "CI and hooks",
            "slug": "docs/ci"
          }
        ]
      }
    ],
    "pages": [
      {
        "title": "Overview",
        "description": "Install the CLI and check a small repository guide.",
        "slug": "docs",
        "href": "docs/"
      },
      {
        "title": "Commands",
        "description": "Create, print, check, and verify guides.",
        "slug": "docs/commands",
        "href": "docs/commands/"
      },
      {
        "title": "Guide format",
        "description": "Paths, comments, omissions, and filename shorthand.",
        "slug": "docs/guide-format",
        "href": "docs/guide-format/"
      },
      {
        "title": "CI and hooks",
        "description": "Run verification as a required check or local hook.",
        "slug": "docs/ci",
        "href": "docs/ci/"
      }
    ]
  }
};
