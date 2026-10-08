import { render } from "solid-js/web";

import "../styles/base.css";
import "./hud.css";
import { Hud } from "./Hud";

render(() => <Hud />, document.getElementById("root")!);
