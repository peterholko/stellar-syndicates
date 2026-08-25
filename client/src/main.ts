import { mountDesktopMarkup } from "./shell/desktop/markup";

const shellRoot = document.getElementById("shell-root");
if (!shellRoot) throw new Error("missing #shell-root");
mountDesktopMarkup(shellRoot);
void import("./shell/desktop/index");
