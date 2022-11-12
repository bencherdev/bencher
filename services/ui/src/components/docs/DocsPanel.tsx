import { Match, Switch } from "solid-js";

import SwaggerPanel from "./api/SwaggerPanel";
import { Docs } from "./config";

const DocsPanel = (props) => {
  return (
    <Switch fallback={<p>Unknown docs path: {props.pathname()} </p>}>
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
