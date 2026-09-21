type Consumer = (visible: boolean, pixels: number) => void;
const targets = new Map<HTMLElement, Consumer>();
const buckets = [16, 24, 32, 36, 48, 64, 72, 96, 108, 128, 144, 192, 256];
let intersections: IntersectionObserver | undefined;
let appearance: MutationObserver | undefined;
let scale: MediaQueryList | undefined;
let scaleValue = 0;

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
  const container = element.closest(".result-list, .result-preview");
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
  for (const [element, consume] of targets)
    consume(visible(element), pixels(element));
  if (scaleValue !== window.devicePixelRatio) watchScale();
}

function watchScale() {
  scale?.removeEventListener("change", refresh);
  scaleValue = window.devicePixelRatio;
  scale = matchMedia(`(resolution: ${scaleValue}dppx)`);
  scale.addEventListener("change", refresh);
}

// Only active native avatars subscribe. All targets share one observer and
// one display-scale listener; CSS tokens define row and preview sizes.
export function observeIconDisplay(element: HTMLElement, consume: Consumer) {
  if (!targets.size) {
    intersections = new IntersectionObserver((entries) => {
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
    appearance = new MutationObserver(refresh);
    appearance.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-appearance"],
    });
    window.addEventListener("resize", refresh);
    watchScale();
  }
  targets.set(element, consume);
  intersections!.observe(element);
  // A layout check permits immediate work only for an actually visible icon.
  // The shared observer handles later scrolling and clipping changes.
  consume(visible(element), pixels(element));
  return () => {
    if (targets.get(element) !== consume) return;
    targets.delete(element);
    intersections?.unobserve(element);
    if (targets.size) return;
    intersections?.disconnect();
    appearance?.disconnect();
    scale?.removeEventListener("change", refresh);
    window.removeEventListener("resize", refresh);
    intersections = undefined;
    appearance = undefined;
    scale = undefined;
  };
}
