import { Match, Switch } from "solid-js";

import SwaggerPanel from "./api/SwaggerPanel";
import Page from "./config/page";

const DocsPanel = (props) => {
  return (
    <Switch fallback={<DocPanel page={props.config?.page} />}>
      <Match when={props.config?.kind === Page.API_V0}>
        <SwaggerPanel />
      </Match>
    </Switch>
  );
};

const DocPanel = (props) => {
  return (
    <div class="content">
      <h1 class="title">{props.page?.heading}</h1>
      <hr />
      {props.page?.content}
      <br />
    </div>
  );
};

export default DocsPanel;
