import { render } from "solid-js/web";
import App from "./App";
import "./styles/app.css";

const root = document.getElementById("root");
if (root) render(() => <App />, root);
