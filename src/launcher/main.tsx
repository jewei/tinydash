import { render } from "solid-js/web";

import "../styles/base.css";
import "./launcher.css";
import { Launcher } from "./Launcher";

render(() => <Launcher />, document.getElementById("root")!);
