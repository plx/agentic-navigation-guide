import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

type DocsPage = {
  title: string;
  description: string;
  slug: string;
  href: string;
};

const origin = `http://127.0.0.1:${process.env.SITE_TEST_PORT ?? "4321"}`;
const projectTitle = "agentic-navigation-guide";
const projectDescription =
  "agentic-navigation-guide verifies hand-written repo maps against the filesystem, so coding agents do not start from stale navigation instructions.";
const basePath: string = "/agentic-navigation-guide";
const normalizedBasePath = basePath === "/" ? "" : basePath;
// prettier-ignore
const docsPages: DocsPage[] = [
    {
      "title": "Overview",
      "description": "How agentic-navigation-guide keeps repository route maps accurate for coding agents.",
      "slug": "docs",
      "href": "docs/"
    },
    {
      "title": "Commands",
      "description": "CLI commands provided by agentic-navigation-guide.",
      "slug": "docs/commands",
      "href": "docs/commands/"
    },
    {
      "title": "Guide Format",
      "description": "Syntax rules for agentic navigation guide blocks.",
      "slug": "docs/guide-format",
      "href": "docs/guide-format/"
    },
    {
      "title": "CI and Hooks",
      "description": "Using agentic-navigation-guide in automation.",
      "slug": "docs/ci",
      "href": "docs/ci/"
    }
  ];
const pagesToCheck = ["/", ...docsPages.map((page) => page.href)];
const pagesToAudit = ["/", docsPages[0]?.href].filter(Boolean);

function sitePath(path = "/"): string {
  const cleanPath = path.startsWith("/") ? path : `/${path}`;
  return `${normalizedBasePath}${cleanPath}`;
}

function isSkippableHref(href: string): boolean {
  return (
    href === "" ||
    href.startsWith("mailto:") ||
    href.startsWith("tel:") ||
    href.startsWith("javascript:")
  );
}

test.describe("rendered site", () => {
  test("exposes core document and landmark properties", async ({ page }) => {
    await page.goto(sitePath("/"));

    expect(await page.title()).toContain(projectTitle);
    await expect(page.locator('meta[name="description"]')).toHaveAttribute(
      "content",
      projectDescription,
    );
    await expect(page.getByRole("main")).toBeVisible();
    await expect(
      page.getByRole("navigation", { name: /primary/i }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { level: 1, name: projectTitle }),
    ).toBeVisible();
    await expect(page.locator(".skip-link")).toHaveAttribute("href", "#main");
  });

  test("keeps primary pages inside the viewport", async ({ page }) => {
    for (const pagePath of pagesToCheck) {
      await page.goto(sitePath(pagePath));
      await expect(page.getByRole("main")).toBeVisible();
      const hasHorizontalOverflow = await page.evaluate(
        () => document.documentElement.scrollWidth > window.innerWidth + 1,
      );
      expect(
        hasHorizontalOverflow,
        `${pagePath} should not overflow horizontally`,
      ).toBe(false);
    }
  });

  test("manages the mobile navigation expanded state accessibly", async ({
    page,
  }) => {
    await page.goto(sitePath("/"));

    const toggle = page.locator("[data-nav-toggle]");
    if (!(await toggle.isVisible())) {
      return;
    }

    const panel = page.locator("[data-nav-panel]");
    await expect(toggle).toHaveAttribute("aria-controls", "mobile-nav");
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await expect(panel).toBeHidden();

    await toggle.click();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await expect(panel).toBeVisible();

    await page.keyboard.press("Escape");
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await expect(panel).toBeHidden();
  });

  test("validates rendered links and internal link targets", async ({
    page,
    request,
  }) => {
    const failures: string[] = [];

    for (const pagePath of pagesToCheck) {
      const response = await page.goto(sitePath(pagePath));
      expect(response?.status(), `${pagePath} should load`).toBeLessThan(400);

      const links = await page.locator("a[href]").evaluateAll((anchors) =>
        anchors.map((anchor) => ({
          href: anchor.getAttribute("href") ?? "",
          label: anchor.textContent?.trim() ?? "",
        })),
      );

      for (const link of links) {
        if (isSkippableHref(link.href)) {
          continue;
        }

        const resolved = new URL(link.href, `${origin}${sitePath(pagePath)}`);
        if (!["http:", "https:"].includes(resolved.protocol)) {
          failures.push(
            `${pagePath}: unsupported link protocol in ${link.href}`,
          );
          continue;
        }

        if (resolved.origin !== origin) {
          if (!link.label) {
            failures.push(
              `${pagePath}: external link ${link.href} has no text label`,
            );
          }
          continue;
        }

        if (
          normalizedBasePath &&
          resolved.pathname !== normalizedBasePath &&
          !resolved.pathname.startsWith(`${normalizedBasePath}/`)
        ) {
          failures.push(
            `${pagePath}: internal link escapes base path: ${link.href}`,
          );
          continue;
        }

        const targetPath = `${resolved.pathname}${resolved.search}`;
        const targetResponse = await request.get(targetPath);
        if (targetResponse.status() >= 400) {
          failures.push(
            `${pagePath}: ${link.href} returned ${targetResponse.status()}`,
          );
          continue;
        }

        if (resolved.hash) {
          await page.goto(`${targetPath}${resolved.hash}`);
          const targetExists = await page.evaluate((hash) => {
            const id = decodeURIComponent(hash.slice(1));
            return Boolean(
              document.getElementById(id) ||
              document.querySelector(`[name="${id}"]`),
            );
          }, resolved.hash);
          if (!targetExists) {
            failures.push(
              `${pagePath}: ${link.href} hash target does not exist`,
            );
          }
        }
      }
    }

    expect(failures).toEqual([]);
  });

  for (const pagePath of pagesToAudit) {
    test(`has no detectable accessibility violations on ${pagePath}`, async ({
      page,
    }) => {
      await page.goto(sitePath(pagePath));

      const results = await new AxeBuilder({ page }).analyze();
      expect(results.violations).toEqual([]);
    });
  }
});
