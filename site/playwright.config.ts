import { defineConfig, devices } from "@playwright/test";
import { siteConfig } from "./src/site.config.mjs";

const basePath: string = siteConfig.site.basePath;
const normalizedBasePath = basePath === "/" ? "" : basePath;
const testPort = process.env.SITE_TEST_PORT ?? "4321";
const localSiteOrigin = `http://127.0.0.1:${testPort}`;
const localSiteUrl = `${localSiteOrigin}${normalizedBasePath}/`;
const dotReporter = ["dot"] as const;
const htmlReporter = ["html", { open: "never" }] as const;
const listReporter = ["list"] as const;

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  timeout: 30_000,
  expect: {
    timeout: 5_000,
  },
  reporter: process.env.CI
    ? [dotReporter, htmlReporter]
    : [listReporter, htmlReporter],
  use: {
    baseURL: localSiteOrigin,
    trace: "on-first-retry",
  },
  webServer: {
    command: `npm run dev -- --host 127.0.0.1 --port ${testPort}`,
    url: localSiteUrl,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    // Astro 7 auto-detects AI-agent shells (Claude Code, Cursor, etc. via `am-i-vibing`)
    // and daemonizes `astro dev` into a detached background process, which makes
    // Playwright's webServer supervisor report "exited early". Setting this marker keeps
    // the dev server in the foreground everywhere. CI and plain local shells already run
    // in the foreground, so this only affects runs launched from inside an agent.
    env: { ASTRO_DEV_BACKGROUND: "1" },
  },
  projects: [
    {
      name: "mobile",
      use: {
        browserName: "chromium",
        viewport: { width: 390, height: 844 },
        deviceScaleFactor: 3,
        isMobile: true,
        hasTouch: true,
      },
    },
    {
      name: "tablet",
      use: {
        browserName: "chromium",
        viewport: { width: 820, height: 1180 },
        deviceScaleFactor: 2,
        isMobile: true,
        hasTouch: true,
      },
    },
    {
      name: "desktop",
      use: {
        browserName: "chromium",
        ...devices["Desktop Chrome"],
        viewport: { width: 1440, height: 1000 },
      },
    },
  ],
});
