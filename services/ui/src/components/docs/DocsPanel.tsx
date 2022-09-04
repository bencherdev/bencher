import { createEffect, createSignal, lazy, Match, Switch } from "solid-js";

import SwaggerUI from "swagger-ui";
import swagger from "./api/swagger.json";
import Example from "./Example.mdx";

const DocsPanel = (props) => {
  return (
    <Switch fallback={<p>Unknown docs path: {props.pathname()} </p>}>
      <Match when={props.page === false}>
        <>
          <div id="swagger" />
          <SwaggerPage />
        </>
      </Match>
      <Match when={props.page === true}>
        <Example />
      </Match>
    </Switch>
  );
};

const SwaggerPage = (props) => {
  createEffect(() => {
    SwaggerUI({
      dom_id: "#swagger",
      spec: swagger,
      layout: "BaseLayout",
    });
  });

  return <></>;
};

export default DocsPanel;
