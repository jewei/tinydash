import { createSignal, For, onCleanup, onMount } from "solid-js";
import { render } from "solid-js/web";
import { appearances, isAppearance, type Appearance } from "../src/appearance";
import "./review.css";

const details = {
  light: {
    tag: "A warm, open workspace",
    description:
      "Cream surfaces, peach selection, and a detail panel for the item you choose. Based on the supplied Canvas design.",
    features: ["Two-column layout", "Caprasimo headings", "Orange actions"],
  },
  dark: {
    tag: "The same Canvas, after dark",
    description:
      "Warm charcoal and olive surfaces keep the same structure, with clear peach highlights and soft contrast.",
    features: ["Warm dark palette", "Olive detail panel", "Clear selection"],
  },
  compact: {
    tag: "Everything in one list",
    description:
      "A single column with shorter rows and round selection controls. More results fit in the same space.",
    features: [
      "Single-column layout",
      "Compact rows",
      "Full keyboard controls",
    ],
  },
};

function DesignReview() {
  const [selected, setSelected] = createSignal<Appearance>("light");
  const [feedback, setFeedback] = createSignal("");
  let preview!: HTMLIFrameElement;
  const current = () => appearances.find((item) => item.id === selected())!;
  const previewUrl = () => `./launcher.html?appearance=${selected()}`;

  function onMessage(event: MessageEvent) {
    if (
      event.origin !== window.location.origin ||
      event.source !== preview.contentWindow
    )
      return;
    if (
      event.data?.type === "tinydash:preview-action" &&
      typeof event.data.message === "string"
    ) {
      setFeedback(event.data.message);
    }
    if (
      event.data?.type === "tinydash:preview-appearance" &&
      isAppearance(event.data.appearance)
    ) {
      setSelected(event.data.appearance);
    }
  }

  onMount(() => window.addEventListener("message", onMessage));
  onCleanup(() => window.removeEventListener("message", onMessage));

  return (
    <div class="review-page">
      <header class="review-header">
        <a class="review-brand" href="/" aria-label="TinyDash home">
          <span class="review-mark" aria-hidden="true">
            <i />
            <i />
          </span>
          TinyDash
        </a>
        <span class="review-label">Design preview</span>
      </header>

      <main>
        <div class="review-intro">
          <div>
            <p class="review-eyebrow">THE CANVAS COLLECTION</p>
            <h1>A little less searching.</h1>
          </div>
          <p class="review-lede">
            Light, dark, or a little more compact.
            <br /> Choose a look and try the launcher below.
          </p>
        </div>

        <div class="review-layout">
          <aside class="variant-picker" aria-label="Design variants">
            <For each={appearances}>
              {(item, index) => (
                <button
                  class="variant-choice"
                  classList={{ "is-selected": selected() === item.id }}
                  aria-pressed={selected() === item.id}
                  onClick={() => {
                    setSelected(item.id);
                    setFeedback("");
                  }}
                >
                  <span class="variant-topline">
                    <span class="variant-number">0{index() + 1}</span>
                    <span
                      class="variant-swatches"
                      data-palette={item.id}
                      aria-hidden="true"
                    >
                      <i />
                      <i />
                      <i />
                    </span>
                    <span class="variant-radio" aria-hidden="true" />
                  </span>
                  <span class="variant-name">{item.label}</span>
                  <span class="variant-description">{item.description}</span>
                </button>
              )}
            </For>
            <p class="picker-note">
              Available in the desktop app under
              <br /> <strong>Actions → Appearance</strong>.
            </p>
          </aside>

          <section class="preview-section" aria-label="Live launcher preview">
            <div class="preview-heading">
              <span>
                <span class="live-indicator" /> Interactive preview
              </span>
              <a href={previewUrl()} target="_blank" rel="noreferrer">
                Open full size <span aria-hidden="true">↗</span>
              </a>
            </div>
            <iframe
              ref={preview}
              class="launcher-preview"
              classList={{ "compact-preview": selected() === "compact" }}
              src={previewUrl()}
              title={`${current().label} launcher with sample data`}
            />
            <div class="preview-caption">
              <div>
                <h2>{details[selected()].tag}</h2>
                <p>{details[selected()].description}</p>
              </div>
              <ul aria-label="Variant details">
                <For each={details[selected()].features}>
                  {(feature) => <li>{feature}</li>}
                </For>
              </ul>
            </div>
          </section>
        </div>
      </main>

      <footer class="review-footer">
        <p>
          Try <kbd>Safari</kbd>, <kbd>:coffee</kbd>, or <kbd>12 * 8</kbd>. Use{" "}
          <kbd>↑</kbd> <kbd>↓</kbd> to move through results.
        </p>
        <p class="preview-disclaimer" role="status">
          {feedback() ||
            "Sample data only. Preview actions do not affect your computer."}
        </p>
      </footer>
    </div>
  );
}

const root = document.getElementById("root");
if (root) render(() => <DesignReview />, root);
