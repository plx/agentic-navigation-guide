import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import { siteConfig } from "./src/site.config.mjs";

const fontStylesheetUrl =
  "https://fonts.googleapis.com/css2?family=IBM+Plex+Sans+Condensed:wght@600;700&family=Inter:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;600&display=swap";

const basePath =
  siteConfig.site.basePath === "/" ? "" : siteConfig.site.basePath;
/** @param {string} path */
const siteAsset = (path) => `${basePath}/${path.replace(/^\/+/, "")}`;

export default defineConfig({
  site: siteConfig.site.host,
  base: siteConfig.site.basePath,
  trailingSlash: "always",
  integrations: [
    starlight({
      title: siteConfig.project.title,
      description: siteConfig.project.description,
      logo: {
        src: "./src/assets/tool-mark.svg",
        alt: "",
      },
      customCss: ["./src/styles/starlight.css"],
      head: [
        {
          tag: "link",
          attrs: { rel: "preconnect", href: "https://fonts.googleapis.com" },
        },
        {
          tag: "link",
          attrs: {
            rel: "preconnect",
            href: "https://fonts.gstatic.com",
            crossorigin: "",
          },
        },
        { tag: "link", attrs: { rel: "stylesheet", href: fontStylesheetUrl } },
        {
          tag: "link",
          attrs: {
            rel: "icon",
            href: siteAsset("favicon.svg"),
            type: "image/svg+xml",
          },
        },
      ],
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: siteConfig.repository.url,
        },
      ],
      editLink: {
        baseUrl: `${siteConfig.repository.url}/edit/${siteConfig.repository.defaultBranch}/site/src/content/docs/`,
      },
      sidebar: siteConfig.docs.sidebar,
    }),
  ],
});
