// Runtime and compile-time type tests for the typed `src/i18n` wrapper.

import { afterEach, expect, test } from "nsh/test";
import { setLocale, t, tDynamic } from "./index";

afterEach(() => {
  setLocale(null);
});

test("typed t resolves English, Chinese, and interpolated placeholders", () => {
  expect(t("nav.home")).toBe("Home");
  expect(t("nav.home", { locale: "zh" })).toBe("首页");
  expect(t("account.userMenu", { name: "Ada" })).toBe("User menu: Ada");
  expect(t("settings.cache.other", { count: 3, locale: "zh" })).toBe("本地已缓存 3 条记录。");

  setLocale("zh-CN");
  expect(t("settings.title")).toBe("设置");
  expect(tDynamic("nav.home")).toBe("首页");
});

// Compile-time type checks (verified by `nsh check` / `tsc`; never executed at runtime).
function _typeChecks() {
  // @ts-expect-error unknown key must be rejected at compile time
  t("nav.nonexistentKey");

  // @ts-expect-error missing required %{name} param must be rejected
  t("account.userMenu");

  // @ts-expect-error wrong param name must be rejected
  t("account.userMenu", { user: "Ada" });

  // @ts-expect-error extra params on parameterless key must be rejected
  t("nav.home", { count: 1 });
}

void _typeChecks;
