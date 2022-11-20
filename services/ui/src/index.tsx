/* @refresh reload */
import { render } from "solid-js/web";
import { Router } from "solid-app-router";
import { MDXProvider } from "solid-jsx";

import App from "./App";
import mdx from "./md";

render(
  () => (
    <Router>
      <MDXProvider components={{ ...mdx }}>
        <App />
      </MDXProvider>
    </Router>
  ),
  document.getElementById("root") as HTMLElement
);
