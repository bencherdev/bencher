import { createSignal } from "solid-js";

import ConsoleMenu from "./menu/ConsoleMenu";
import ConsolePanel from "./panel/ConsolePanel";

interface Project {
  uuid: string;
  name: string;
  slug: string;
}

const ConsolePage = (props) => {
  const [project, setProject] = createSignal<Project>();

  const handleProject = (json_project) => {
    setProject({
      uuid: json_project?.uuid,
      name: json_project?.name,
      slug: json_project?.slug,
    });
  };

  return (
    <section class="section">
      <div class="container">
        <div class="columns is-reverse-mobile">
          <div class="column is-one-fifth">
            <ConsoleMenu />
          </div>
          <div class="column">
            <ConsolePanel
              current_location={props.current_location}
              handleTitle={props.handleTitle}
              handleRedirect={props.handleRedirect}
              handleProject={handleProject}
            />
          </div>
        </div>
      </div>
    </section>
  );
};

export default ConsolePage;
