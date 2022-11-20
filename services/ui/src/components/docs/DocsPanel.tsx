import { useLocation } from "solid-app-router";
import { createMemo, Match, Switch } from "solid-js";

import SwaggerPanel from "./api/SwaggerPanel";
import { Docs } from "./config/types";
import QuickStart from "./example.mdx";

const DocsPanel = (props) => {
  const location = useLocation();
  const pathname = createMemo(() => location.pathname);

  return (
    <Switch fallback={<p>Unknown docs path: {pathname()} </p>}>
      <Match when={props.config?.docs === Docs.QUICK_START}>
        <QuickStart />
      </Match>
      <Match when={props.config?.docs === Docs.API_V0}>
        <SwaggerPanel />
      </Match>
    </Switch>
  );
};

export default DocsPanel;
