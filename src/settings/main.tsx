import { render } from "solid-js/web";

import "../styles/base.css";
import "./settings.css";
import { Settings } from "./Settings";

render(() => <Settings />, document.getElementById("root")!);
