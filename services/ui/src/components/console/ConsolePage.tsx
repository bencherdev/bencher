import { useParams } from "solid-app-router";
import { createEffect, createMemo, createSignal } from "solid-js";
import ConsoleMenu from "./menu/ConsoleMenu";
import ConsolePanel from "./panel/ConsolePanel";

export const projectSlug = (pathname) => {
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
  const [count, setCount] = createSignal(0);

  const params = useParams();
  const path_params = createMemo(() => params);
  console.log(path_params());

  createEffect(() => {
    const slug = projectSlug(props.pathname);
    props.handleProjectSlug(slug);
  });

  setInterval(() => {
    if (props.user()?.uuid === null) {
      setCount(count() + 1);
      if (count() === 2) {
        props.handleRedirect("/auth/login");
      }
    } else if (count() !== 0) {
      setCount(0);
    }
  }, 1000);

  return (
    <section class="section">
      <div class="container">
        <div class="columns is-reverse-mobile">
          <div class="column is-one-fifth">
            <ConsoleMenu
              project_slug={props.project_slug}
              handleRedirect={props.handleRedirect}
              handleProjectSlug={props.handleProjectSlug}
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
              handleProjectSlug={props.handleProjectSlug}
            />
          </div>
        </div>
      </div>
    </section>
  );
};

export default ConsolePage;
