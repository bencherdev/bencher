import { createEffect } from "solid-js";
import { pageTitle } from "../site/util";
import DocsMenu from "./DocsMenu";
import DocsPanel from "./DocsPanel";

const DocsPage = (props) => {
  createEffect(() => {
    pageTitle(props.config?.title);
  });

  return (
    <section class="section">
      <div class="container">
        <div class="columns is-reverse-mobile">
          <div class="column is-narrow">
            <DocsMenu />
          </div>
          <div class="column">
            <DocsPanel config={props.config} />
          </div>
        </div>
      </div>
    </section>
  );
};

export default DocsPage;
