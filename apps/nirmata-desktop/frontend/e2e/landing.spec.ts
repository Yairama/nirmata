import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    let callbackId = 1;
    Object.assign(window, {
      __TAURI_INTERNALS__: {
        transformCallback(callback: (...args: unknown[]) => void) {
          const id = callbackId++;
          Object.assign(window, { [`_${id}`]: callback });
          return id;
        },
        unregisterCallback(id: number) {
          delete (window as unknown as Record<string, unknown>)[`_${id}`];
        },
        async invoke(command: string) {
          if (command === "get_current_world") return null;
          if (command === "get_ai_activity") return { busy: false, requestIds: [] };
          if (command === "list_recent_projects") return [];
          if (command === "plugin:app|version") return "0.1.0";
          if (command === "get_ai_provider_status") {
            return {
              state: "connection_unchecked",
              message: "Configuración local completa.",
              canCheckConnection: true,
              connected: false,
              credential: {
                configured: true,
                source: "session_memory",
                persistence: "session",
                secureStoreAvailable: true,
                limitation: null,
              },
            };
          }
          if (command.startsWith("plugin:event|")) return 1;
          if (command.startsWith("plugin:dialog|")) return null;
          return null;
        },
      },
    });
  });
});

test("landing is keyboard navigable, accessible and screenshotable", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 960, height: 680 });
  await page.goto("/");

  const manual = page.getByRole("button", { name: /Empezar manualmente/u });
  await expect(manual).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: /Crear una base del mundo con IA/u })).toBeFocused();

  const accessibility = await new AxeBuilder({ page }).analyze();
  const blocking = accessibility.violations.filter((violation) =>
    violation.impact === "serious" || violation.impact === "critical"
  );
  expect(blocking).toEqual([]);

  await page.screenshot({ path: testInfo.outputPath("landing-960x680.png"), fullPage: true });
});

test("themes persist, pass axe and keep responsive creation paths", async ({ page }, testInfo) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Settings" }).click();
  await page.getByRole("tab", { name: "Apariencia" }).click();
  const theme = page.getByLabel("Tema");

  for (const value of ["light", "dark", "high-contrast"]) {
    await theme.selectOption(value);
    await expect(page.locator("#closed-root")).toHaveAttribute("data-theme", value);
    const accessibility = await new AxeBuilder({ page }).include("#closed-root").analyze();
    const blocking = accessibility.violations.filter((violation) =>
      violation.id === "color-contrast"
      || violation.impact === "serious"
      || violation.impact === "critical"
    );
    expect(blocking).toEqual([]);
    await page.screenshot({ path: testInfo.outputPath(`landing-${value}-960x680.png`), fullPage: true });
  }

  await expect.poll(() => page.evaluate(() => localStorage.getItem("nirmata.appearance.theme")))
    .toBe("high-contrast");
  await page.getByRole("button", { name: "Cerrar Settings" }).click();
  await page.reload();
  await page.getByRole("button", { name: "Settings" }).click();
  await page.getByRole("tab", { name: "Apariencia" }).click();
  await expect(page.getByLabel("Tema")).toHaveValue("high-contrast");
  await page.getByRole("button", { name: "Cerrar Settings" }).click();
  await expect(page.locator(".creation-path-card").first()).toHaveCSS("border-top-width", "2px");

  await page.setViewportSize({ width: 390, height: 844 });
  for (const [index, name] of [
    /Empezar manualmente/u,
    /Crear una base del mundo con IA/u,
    /Estructurar material existente/u,
  ].entries()) {
    await page.getByRole("button", { name }).click();
    const overflow = await page.evaluate(() => Array.from(document.querySelectorAll<HTMLElement>("body *"))
      .filter((element) => {
        const box = element.getBoundingClientRect();
        return box.left < -1 || box.right > document.documentElement.clientWidth + 1;
      })
      .map((element) => ({ tag: element.tagName, className: element.className, right: element.getBoundingClientRect().right })));
    expect(overflow).toEqual([]);
    await page.screenshot({ path: testInfo.outputPath(`landing-path-${index + 1}-390x844.png`), fullPage: true });
    await page.getByRole("button", { name: "Cancelar" }).click();
  }
});

test("AI creation wizard is stepped, inert until confirmation and accessible", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page.getByRole("button", { name: /Crear una base del mundo con IA/u }).click();
  await page.getByRole("textbox", { name: "Nombre" }).fill("Aurora mineral");
  await expect(page.getByText("Paso 1 de 3")).toBeVisible();
  await expect(page.getByRole("button", { name: "Crear mundo" })).toHaveCount(0);

  const accessibility = await new AxeBuilder({ page }).include("#closed-root").analyze();
  const blocking = accessibility.violations.filter((violation) =>
    violation.impact === "serious" || violation.impact === "critical"
  );
  expect(blocking).toEqual([]);
  await page.screenshot({ path: testInfo.outputPath("wizard-ai-step-1-390x844.png"), fullPage: true });
});

test("Settings and About trap and restore focus without a world", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 960, height: 680 });
  await page.goto("/");
  const settings = page.getByRole("button", { name: "Settings" });
  await settings.click();
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await expect(dialog).toBeVisible();
  await expect(page.getByRole("tab", { name: "General" })).toBeVisible();
  const accessibility = await new AxeBuilder({ page }).include("[role=dialog]").analyze();
  expect(accessibility.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical")).toEqual([]);
  await page.screenshot({ path: testInfo.outputPath("settings-960x680.png"), fullPage: true });
  await page.keyboard.press("Escape");
  await expect(settings).toBeFocused();

  const help = page.getByRole("button", { name: "Ayuda", exact: true });
  await help.click();
  await expect(page.getByRole("dialog", { name: "Centro de ayuda" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Preguntar no es modificar" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(help).toBeFocused();

  const about = page.getByRole("button", { name: "Acerca de", exact: true });
  await about.click();
  await expect(page.getByRole("dialog", { name: "Acerca de Nirmata" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(about).toBeFocused();
});

test("reduced motion disables synthetic movement", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");
  const computed = await page.evaluate(() => {
    const probe = document.createElement("div");
    probe.style.transition = "opacity 5s";
    probe.style.animation = "synthetic-motion 5s infinite";
    probe.style.scrollBehavior = "smooth";
    document.body.append(probe);
    const style = getComputedStyle(probe);
    return {
      transitionDuration: style.transitionDuration,
      animationName: style.animationName,
      scrollBehavior: style.scrollBehavior,
    };
  });
  expect(computed.transitionDuration).toBe("0s");
  expect(computed.animationName).toBe("none");
  expect(computed.scrollBehavior).toBe("auto");
});

test("system theme follows operating-system color scheme without reload", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  await page.getByRole("button", { name: "Settings" }).click();
  await page.getByRole("tab", { name: "Apariencia" }).click();
  await page.getByLabel("Tema").selectOption("system");
  await page.getByRole("button", { name: "Cerrar Settings" }).click();
  await expect(page.locator("html")).toHaveCSS("background-color", "rgb(21, 20, 18)");

  await page.emulateMedia({ colorScheme: "light" });
  await expect(page.locator("html")).toHaveCSS("background-color", "rgb(245, 242, 234)");
});
