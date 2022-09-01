import { useParams } from "solid-app-router";
import { createEffect, createMemo, createSignal } from "solid-js";
import DocsMenu from "./DocsMenu";
// import ConsolePanel from "./panel/ConsolePanel";

const DocsPage = (props) => {
  return (
    <section class="section">
      <div class="container">
        <div class="columns is-reverse-mobile">
          <div class="column is-one-fifth">
            <DocsMenu
              project_slug={props.project_slug}
              handleRedirect={props.handleRedirect}
              handleProjectSlug={props.handleProjectSlug}
            />
          </div>
          <div class="column">
            {/* <ConsolePanel
              project_slug={props.project_slug}
              operation={props.operation}
              config={props.config}
              path_params={path_params}
              pathname={props.pathname}
              handleTitle={props.handleTitle}
              handleRedirect={props.handleRedirect}
              handleProjectSlug={props.handleProjectSlug}
            /> */}
          </div>
        </div>
      </div>
    </section>
  );
};

export default DocsPage;
