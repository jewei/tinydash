type Consumer = (visible: boolean, pixels: number) => void;
const targets = new Map<HTMLElement, Consumer>();
// Nearby row and preview sizes share one decoded payload. Round upward so
// each image still has at least its required physical resolution.
const buckets = [16, 32, 64, 128, 256];
let intersections: IntersectionObserver | undefined;
let appearance: MutationObserver | undefined;
let scale: MediaQueryList | undefined;
let scaleValue = 0;
let refreshQueued = false;

function pixels(element: HTMLElement) {
  const size = Number.parseFloat(
    getComputedStyle(element).getPropertyValue("--icon-size"),
  );
  const required = Math.ceil(size * window.devicePixelRatio);
  return buckets.find((value) => value >= required) ?? 256;
}

function visible(element: HTMLElement) {
  const bounds = element.getBoundingClientRect();
  // These are the two scrolling containers that own application avatars.
  const container = element.closest(".result-list, .preview-content");
  const clip = container?.getBoundingClientRect();
  return (
    bounds.width > 0 &&
    bounds.height > 0 &&
    Math.min(bounds.right, clip?.right ?? innerWidth, innerWidth) >
      Math.max(bounds.left, clip?.left ?? 0, 0) &&
    Math.min(bounds.bottom, clip?.bottom ?? innerHeight, innerHeight) >
      Math.max(bounds.top, clip?.top ?? 0, 0)
  );
}

function refresh() {
  // Finish layout reads before a consumer can change an image or its classes.
  const updates = Array.from(targets, ([element, consume]) => ({
    element,
    consume,
    visible: visible(element),
    pixels: pixels(element),
  }));
  for (const update of updates)
    if (targets.get(update.element) === update.consume)
      update.consume(update.visible, update.pixels);
  if (scaleValue !== window.devicePixelRatio) watchScale();
}

function scheduleRefresh() {
  if (refreshQueued) return;
  refreshQueued = true;
  queueMicrotask(() => {
    refreshQueued = false;
    if (targets.size) refresh();
  });
}

function watchScale() {
  scale?.removeEventListener("change", scheduleRefresh);
  if (!scale || scaleValue !== window.devicePixelRatio) {
    scaleValue = window.devicePixelRatio;
    scale = matchMedia(`(resolution: ${scaleValue}dppx)`);
  }
  scale.addEventListener("change", scheduleRefresh);
}

// Only active native avatars subscribe. All targets share one observer and
// one display-scale listener; CSS tokens define row and preview sizes.
export function observeIconDisplay(element: HTMLElement, consume: Consumer) {
  if (!targets.size) {
    intersections ??= new IntersectionObserver((entries) => {
      for (const entry of entries) {
        const target = entry.target as HTMLElement;
        targets.get(target)?.(
          entry.isIntersecting &&
            entry.intersectionRect.width > 0 &&
            entry.intersectionRect.height > 0,
          pixels(target),
        );
      }
    });
    appearance ??= new MutationObserver(scheduleRefresh);
    appearance.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-appearance", "data-compact"],
    });
    window.addEventListener("resize", scheduleRefresh);
    watchScale();
  }
  targets.set(element, consume);
  intersections!.observe(element);
  // A single microtask reads the completed row layout before starting images.
  // This does not wait for an intersection event or load clipped icons.
  scheduleRefresh();
  return () => {
    if (targets.get(element) !== consume) return;
    targets.delete(element);
    intersections?.unobserve(element);
    if (targets.size) return;
    intersections?.disconnect();
    intersections?.takeRecords();
    appearance?.disconnect();
    scale?.removeEventListener("change", scheduleRefresh);
    window.removeEventListener("resize", scheduleRefresh);
    // Keep one disconnected observer of each type for the next result list.
    // Neither retains observed targets while the launcher is empty or hidden.
  };
}
