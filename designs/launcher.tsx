// This separate Vite entry is for design review only. The desktop entry never
// imports the preview backend or sample data, and the production build omits it.
import { render } from "solid-js/web";
import "./sample-backend";
import App from "../src/App";
import { isAppearance } from "../src/appearance";
import "../src/styles/app.css";
import "../src/styles/refined.css";

const value = new URLSearchParams(window.location.search).get("appearance");
const appearance = isAppearance(value) ? value : "light";
const root = document.getElementById("root");
if (root)
  render(
    () => (
      <App
        initialAppearance={appearance}
        onAppearanceChange={(value) =>
          window.parent.postMessage(
            { type: "tinydash:preview-appearance", appearance: value },
            window.location.origin,
          )
        }
      />
    ),
    root,
  );
