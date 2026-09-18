import { render } from "solid-js/web";
import App from "./App";
import Settings from "./Settings";
import "./styles/app.css";
import "./styles/refined.css";

const root = document.getElementById("root");
if (root)
  render(
    () =>
      new URLSearchParams(window.location.search).get("view") === "settings" ? (
        <Settings />
      ) : (
        <App />
      ),
    root,
  );
