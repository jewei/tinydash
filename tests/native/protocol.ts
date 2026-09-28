/** Preserve errors returned by both WebDriver and our async script callbacks. */
export function nativeResponseError(
  ok: boolean,
  statusText: string,
  value: unknown,
): string | null {
  const result =
    value !== null && typeof value === "object"
      ? (value as { error?: unknown; message?: unknown })
      : undefined;
  const error = typeof result?.error === "string" ? result.error : "";
  if (ok && !error) return null;
  const message = typeof result?.message === "string" ? result.message : "";
  return message || error || statusText;
}
