import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

const fontStylesheetUrl = "https://fonts.googleapis.com/css2?family=IBM+Plex+Sans+Condensed:wght@600;700&family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;600&display=swap";

export default defineConfig({
  site: "https://plx.github.io",
  base: "/agentic-navigation-guide",
  integrations: [
    starlight({
      title: "Agentic Navigation Guide",
      description: "A Rust CLI for maintaining accurate repository navigation guides for coding agents.",
      customCss: ["./src/styles/starlight.css"],
      head: [
        { tag: "link", attrs: { rel: "preconnect", href: "https://fonts.googleapis.com" } },
        { tag: "link", attrs: { rel: "preconnect", href: "https://fonts.gstatic.com", crossorigin: "" } },
        { tag: "link", attrs: { rel: "stylesheet", href: fontStylesheetUrl } }
      ],
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/plx/agentic-navigation-guide"
        }
      ],
      editLink: {
        baseUrl: "https://github.com/plx/agentic-navigation-guide/edit/main/site/src/content/docs/"
      },
      sidebar: [
        {
          label: "Guide",
          items: [
            { label: "Overview", slug: "docs" },
            { label: "Commands", slug: "docs/commands" },
            { label: "Guide Format", slug: "docs/guide-format" },
            { label: "CI and Hooks", slug: "docs/ci" }
          ]
        }
      ]
    })
  ]
});
