import { expect, test } from "@playwright/test";
import { nativeResponseError } from "./native/protocol";

test("native callback failures retain their error instead of HTTP OK", () => {
  expect(
    nativeResponseError(true, "OK", { error: "Search timing timed out" }),
  ).toBe("Search timing timed out");
  expect(
    nativeResponseError(false, "Internal Server Error", {
      error: "javascript error",
      message: "The native callback failed",
    }),
  ).toBe("The native callback failed");
  expect(nativeResponseError(false, "Bad Gateway", null)).toBe("Bad Gateway");
});

test("successful native values are not classified as failures", () => {
  for (const value of [null, false, 0, "", { visible: false }, { error: "" }]) {
    expect(nativeResponseError(true, "OK", value)).toBeNull();
  }
});
