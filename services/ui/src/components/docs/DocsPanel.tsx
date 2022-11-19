import { useLocation } from "solid-app-router";
import { createMemo, Match, Switch } from "solid-js";

import SwaggerPanel from "./api/SwaggerPanel";
import { Docs } from "./config";

const DocsPanel = (props) => {
  const location = useLocation();
  const pathname = createMemo(() => location.pathname);

  return (
    <Switch fallback={<p>Unknown docs path: {pathname()} </p>}>
      <Match when={props.page === Docs.API}>
        <>
          <div id="swagger" />
          <SwaggerPanel />
        </>
      </Match>
    </Switch>
  );
};

export default DocsPanel;
