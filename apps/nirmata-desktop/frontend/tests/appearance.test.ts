import { beforeEach, expect, test } from "vitest";
import { applyAppearanceTheme, readAppearanceTheme } from "../appearance.js";

beforeEach(() => {
  document.body.innerHTML = '<div id="closed-root"></div>';
  localStorage.clear();
});

test("appearance validates, applies and persists the selected theme", () => {
  localStorage.setItem("nirmata.appearance.theme", "invalid");
  expect(readAppearanceTheme()).toBe("system");

  applyAppearanceTheme("dark");
  expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  expect(localStorage.getItem("nirmata.appearance.theme")).toBe("dark");
  expect(readAppearanceTheme()).toBe("dark");
});
