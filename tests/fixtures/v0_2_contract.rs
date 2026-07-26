pub(super) const CASES: &[ContractCase] = &[
    ContractCase {
        id: "marker-bare",
        source: "<agentic-navigation-guide>\n- Cargo.toml\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "Cargo.toml",
            }]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "Cargo.toml",
            }]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "marker-ignore-unquoted",
        source: "<agentic-navigation-guide ignore=true>\n- example.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: true,
            items: Some(&[]),
        },
        current: ExpectedResult::Accept {
            ignore: true,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "example.txt",
            }]),
        },
        pending_issue: Some(39),
    },
    ContractCase {
        id: "marker-ignore-quoted",
        source: "<agentic-navigation-guide   ignore = \"true\"  >\n- example.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: true,
            items: Some(&[]),
        },
        current: ExpectedResult::Accept {
            ignore: true,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "example.txt",
            }]),
        },
        pending_issue: Some(39),
    },
    ContractCase {
        id: "marker-outer-whitespace",
        source: "  <agentic-navigation-guide>\n- example.txt\n  </agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "example.txt",
            }]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "example.txt",
            }]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "marker-concatenated-attribute",
        source: "<agentic-navigation-guideignore=true>\n- missing.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: true,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "missing.txt",
            }]),
        },
        pending_issue: Some(38),
    },
    ContractCase {
        id: "marker-unknown-attribute",
        source: "<agentic-navigation-guide mode=example>\n- example.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "example.txt",
            }]),
        },
        pending_issue: Some(38),
    },
    ContractCase {
        id: "marker-duplicate-attribute",
        source: "<agentic-navigation-guide ignore=true ignore=true>\n- example.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: true,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "example.txt",
            }]),
        },
        pending_issue: Some(38),
    },
    ContractCase {
        id: "marker-false-attribute",
        source: "<agentic-navigation-guide ignore=false>\n- example.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "example.txt",
            }]),
        },
        pending_issue: Some(38),
    },
    ContractCase {
        id: "marker-space-without-attribute",
        source: "<agentic-navigation-guide >\n- example.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "example.txt",
            }]),
        },
        pending_issue: Some(38),
    },
    ContractCase {
        id: "marker-unterminated-quote",
        source: "<agentic-navigation-guide ignore=\"true>\n- example.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "example.txt",
            }]),
        },
        pending_issue: Some(38),
    },
    ContractCase {
        id: "marker-closing-attribute",
        source: "<agentic-navigation-guide>\n- example.txt\n</agentic-navigation-guide ignore=true>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "marker-two-blocks",
        source: "<agentic-navigation-guide>\n- first.txt\n</agentic-navigation-guide>\n<agentic-navigation-guide>\n- second.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "body-prologue-epilogue",
        source: "# Navigation\n<agentic-navigation-guide>\n- src/\n  - main.rs # Entry point\n</agentic-navigation-guide>\nAdditional Markdown.",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "src",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "src/main.rs",
                },
            ]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "src",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "src/main.rs",
                },
            ]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "body-empty",
        source: "<agentic-navigation-guide>\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "body-blank-line",
        source: "<agentic-navigation-guide>\n- first.txt\n\n- second.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "body-non-list",
        source: "<agentic-navigation-guide>\nnot a list item\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "body-extra-list-space",
        source: "<agentic-navigation-guide>\n-  leading-space.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "leading-space.txt",
            }]),
        },
        pending_issue: Some(40),
    },
    ContractCase {
        id: "body-tab-after-dash",
        source: "<agentic-navigation-guide>\n-\texample.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "example.txt",
            }]),
        },
        pending_issue: Some(40),
    },
    ContractCase {
        id: "indent-two-spaces",
        source: "<agentic-navigation-guide>\n- src/\n  - cli/\n    - check.rs\n  - lib.rs\n- Cargo.toml\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "src",
                },
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "src/cli",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "src/cli/check.rs",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "src/lib.rs",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "Cargo.toml",
                },
            ]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "src",
                },
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "src/cli",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "src/cli/check.rs",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "src/lib.rs",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "Cargo.toml",
                },
            ]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "indent-four-spaces",
        source: "<agentic-navigation-guide>\n- src/\n    - main.rs\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "src",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "src/main.rs",
                },
            ]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "src",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "src/main.rs",
                },
            ]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "indent-sixteen-spaces",
        source: "<agentic-navigation-guide>\n- src/\n                - main.rs\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "src",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "src/main.rs",
                },
            ]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "src",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "src/main.rs",
                },
            ]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "indent-seventeen-spaces",
        source: "<agentic-navigation-guide>\n- src/\n                 - main.rs\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "indent-child-under-file",
        source: "<agentic-navigation-guide>\n- a/\n- b\n  - c\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "indent-skipped-depth",
        source: "<agentic-navigation-guide>\n- a/\n  - b/\n      - c\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "indent-tab",
        source: "<agentic-navigation-guide>\n- src/\n\t- main.rs\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "path-comment-escaped-hash",
        source: "<agentic-navigation-guide>\n- docs/issue\\#123.md # ticket #123\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "docs/issue#123.md",
            }]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "docs/issue#123.md",
            }]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "path-unicode-interior-space",
        source: "<agentic-navigation-guide>\n- docs/Guía rápida.md\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "docs/Guía rápida.md",
            }]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "docs/Guía rápida.md",
            }]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "path-quoted-sensitive",
        source: "<agentic-navigation-guide>\n- \" report#draft[final], \\\"copy\\\" \\\\ \"\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: " report#draft[final], \"copy\" \\ ",
            }]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "\" report",
            }]),
        },
        pending_issue: Some(41),
    },
    ContractCase {
        id: "path-quoted-ellipsis",
        source: "<agentic-navigation-guide>\n- \"...\"\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "...",
            }]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "\"...\"",
            }]),
        },
        pending_issue: Some(41),
    },
    ContractCase {
        id: "path-bare-nested-ellipsis",
        source: "<agentic-navigation-guide>\n- src/.../file\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "src/.../file",
            }]),
        },
        pending_issue: Some(41),
    },
    ContractCase {
        id: "path-quoted-nested-ellipsis",
        source: "<agentic-navigation-guide>\n- \"src/.../file\"\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "src/.../file",
            }]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "\"src/.../file\"",
            }]),
        },
        pending_issue: Some(41),
    },
    ContractCase {
        id: "path-quoted-directory",
        source: "<agentic-navigation-guide>\n- \"src\"/\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::Directory,
                path: "src",
            }]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::Directory,
                path: "\"src\"",
            }]),
        },
        pending_issue: Some(41),
    },
    ContractCase {
        id: "path-quoted-trailing-separator",
        source: "<agentic-navigation-guide>\n- \"src/\"\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "\"src/\"",
            }]),
        },
        pending_issue: Some(41),
    },
    ContractCase {
        id: "path-repeated-internal-separator",
        source: "<agentic-navigation-guide>\n- foo//bar\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "path-repeated-trailing-separator",
        source: "<agentic-navigation-guide>\n- foo///\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::Directory,
                path: "foo",
            }]),
        },
        pending_issue: Some(40),
    },
    ContractCase {
        id: "path-dot-component",
        source: "<agentic-navigation-guide>\n- src/./main.rs\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "path-parent-component",
        source: "<agentic-navigation-guide>\n- src/../secret.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "path-posix-absolute",
        source: "<agentic-navigation-guide>\n- /etc/passwd\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "path-windows-prefix",
        source: "<agentic-navigation-guide>\n- C:/Windows/System32\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "C:/Windows/System32",
            }]),
        },
        pending_issue: Some(40),
    },
    ContractCase {
        id: "path-nested-drive-looking-component",
        source: "<agentic-navigation-guide>\n- dir/C:notes\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "dir/C:notes",
            }]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "dir/C:notes",
            }]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "path-unknown-escape",
        source: "<agentic-navigation-guide>\n- bad\\q.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "badq.txt",
            }]),
        },
        pending_issue: Some(41),
    },
    ContractCase {
        id: "path-empty-quoted",
        source: "<agentic-navigation-guide>\n- \"\"\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "\"\"",
            }]),
        },
        pending_issue: Some(41),
    },
    ContractCase {
        id: "path-unmatched-closing-bracket",
        source: "<agentic-navigation-guide>\n- bad]name.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "bad]name.txt",
            }]),
        },
        pending_issue: Some(40),
    },
    ContractCase {
        id: "path-duplicate-decoded",
        source: "<agentic-navigation-guide>\n- report\\#draft\n- report\\#draft\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "report#draft",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "report#draft",
                },
            ]),
        },
        pending_issue: Some(40),
    },
    ContractCase {
        id: "choice-simple",
        source: "<agentic-navigation-guide>\n- FooCoordinator[.h, .cpp] # paired implementation\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "FooCoordinator.h",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "FooCoordinator.cpp",
                },
            ]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "FooCoordinator.h",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "FooCoordinator.cpp",
                },
            ]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "choice-empty-alternative",
        source: "<agentic-navigation-guide>\n- Config[, .local].json\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "Config.json",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "Config.local.json",
                },
            ]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "Config.json",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "Config.local.json",
                },
            ]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "choice-quoted-whitespace",
        source: "<agentic-navigation-guide>\n- x[\" foo \"]y\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "x foo y",
            }]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "xfooy",
            }]),
        },
        pending_issue: Some(40),
    },
    ContractCase {
        id: "choice-escaped-comma",
        source: "<agentic-navigation-guide>\n- data[plain, \\,comma].txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "dataplain.txt",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "data,comma.txt",
                },
            ]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "dataplain.txt",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "data,comma.txt",
                },
            ]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "choice-escaped-hash-comment",
        source: "<agentic-navigation-guide>\n- x[\"a\\#b\", c]y # inherited\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "xa#by",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "xcy",
                },
            ]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "xa#by",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "xcy",
                },
            ]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "choice-all-empty",
        source: "<agentic-navigation-guide>\n- Config[,]\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "choice-single-alternative",
        source: "<agentic-navigation-guide>\n- File[.rs]\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[ExpectedItem {
                kind: ItemKind::File,
                path: "File.rs",
            }]),
        },
        pending_issue: Some(40),
    },
    ContractCase {
        id: "choice-unclosed",
        source: "<agentic-navigation-guide>\n- File[.h, .rs\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "choice-multiple-lists",
        source: "<agentic-navigation-guide>\n- File[One, Two][.h, .rs]\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "choice-duplicate-expansion",
        source: "<agentic-navigation-guide>\n- File[.rs, .rs]\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "File.rs",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "File.rs",
                },
            ]),
        },
        pending_issue: Some(40),
    },
    ContractCase {
        id: "choice-directory-result",
        source: "<agentic-navigation-guide>\n- module[One, Two]/\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "moduleOne",
                },
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "moduleTwo",
                },
            ]),
        },
        pending_issue: Some(40),
    },
    ContractCase {
        id: "choice-different-parents",
        source: "<agentic-navigation-guide>\n- x[a/b, c/d]y\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "xa/by",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "xc/dy",
                },
            ]),
        },
        pending_issue: Some(40),
    },
    ContractCase {
        id: "placeholder-forms",
        source: "<agentic-navigation-guide>\n- src/\n  - ...\n  - main.rs\n  - ... # future modules\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "src",
                },
                ExpectedItem {
                    kind: ItemKind::Placeholder,
                    path: "src/...",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "src/main.rs",
                },
                ExpectedItem {
                    kind: ItemKind::Placeholder,
                    path: "src/...",
                },
            ]),
        },
        current: ExpectedResult::Accept {
            ignore: false,
            items: Some(&[
                ExpectedItem {
                    kind: ItemKind::Directory,
                    path: "src",
                },
                ExpectedItem {
                    kind: ItemKind::Placeholder,
                    path: "src/...",
                },
                ExpectedItem {
                    kind: ItemKind::File,
                    path: "src/main.rs",
                },
                ExpectedItem {
                    kind: ItemKind::Placeholder,
                    path: "src/...",
                },
            ]),
        },
        pending_issue: None,
    },
    ContractCase {
        id: "placeholder-adjacent",
        source: "<agentic-navigation-guide>\n- src/\n  - ...\n  - ... # future modules\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "placeholder-child",
        source: "<agentic-navigation-guide>\n- ...\n  - child.txt\n</agentic-navigation-guide>",
        normative: ExpectedResult::Reject,
        current: ExpectedResult::Reject,
        pending_issue: None,
    },
    ContractCase {
        id: "ignore-opaque-body",
        source: "<agentic-navigation-guide ignore=true>\nthis is deliberately not a list\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: true,
            items: Some(&[]),
        },
        current: ExpectedResult::Reject,
        pending_issue: Some(39),
    },
    ContractCase {
        id: "ignore-empty-body",
        source: "<agentic-navigation-guide ignore=\"true\">\n</agentic-navigation-guide>",
        normative: ExpectedResult::Accept {
            ignore: true,
            items: Some(&[]),
        },
        current: ExpectedResult::Reject,
        pending_issue: Some(39),
    },
];
