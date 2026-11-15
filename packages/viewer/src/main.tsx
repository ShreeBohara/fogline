import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./standalone.css";
import "./styles.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("The Fogline viewer needs an element with id `root`.");
}

// The bundle sits next to the page: `fogline open` serves it there and a
// static export writes it there, so the viewer has one way to find its data.
const bundleUrl = new URL("./bundle/", document.baseURI);

createRoot(root).render(
  <StrictMode>
    <App bundleUrl={bundleUrl} />
  </StrictMode>,
);
