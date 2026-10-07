// Numbers as people read them.

/** A size in decimal units, as file managers show it: 9.9 KB, 10 KB, 1.0 MB. */
export function formatBytes(bytes: number) {
  if (bytes < 1000) return bytes === 1 ? "1 byte" : `${bytes} bytes`;
  const units = ["KB", "MB", "GB", "TB"];
  // One decimal below 10, judged after rounding, so 9,960 bytes reads 10 KB.
  const shown = (value: number) => {
    const tenths = Math.round(value * 10) / 10;
    return tenths < 10 ? tenths.toFixed(1) : Math.round(value).toFixed(0);
  };
  let value = bytes / 1000;
  let unit = 0;
  // Compare the rounded value, so 999,999 bytes reads 1.0 MB, not 1000 KB.
  while (Number(shown(value)) >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return `${shown(value)} ${units[unit]}`;
}
