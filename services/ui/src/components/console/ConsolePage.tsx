import { useParams } from "solid-app-router";
import { createMemo, createSignal } from "solid-js";
import ConsoleMenu from "./menu/ConsoleMenu";
import ConsolePanel from "./panel/ConsolePanel";

const projectSlug = (pathname) => {
  const path = pathname().split("/");
  if (
    path.length < 5 ||
    path[0] ||
    path[1] !== "console" ||
    path[2] !== "projects" ||
    !path[3]
  ) {
    return null;
  }
  return path[3];
};

const ConsolePage = (props) => {
  // The project slug can't be a resource because it isn't 100% tied to the URL
  const [project_slug, setProjectSlug] = createSignal<String>(
    projectSlug(props.pathname)
  );

  const params = useParams();
  const path_params = createMemo(() => params);
  console.log(path_params);

  return (
    <section class="section">
      <div class="container">
        <div class="columns is-reverse-mobile">
          <div class="column is-one-fifth">
            <ConsoleMenu
              project_slug={project_slug}
              handleRedirect={props.handleRedirect}
              handleProjectSlug={setProjectSlug}
            />
          </div>
          <div class="column">
            <ConsolePanel
              operation={props.operation}
              config={props.config}
              path_params={path_params}
              pathname={props.pathname}
              handleTitle={props.handleTitle}
              handleRedirect={props.handleRedirect}
              handleProjectSlug={setProjectSlug}
            />
          </div>
        </div>
      </div>
    </section>
  );
};

export default ConsolePage;
