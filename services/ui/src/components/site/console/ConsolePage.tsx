import axios from "axios";
import { createSignal, createMemo, createResource } from "solid-js";

import ConsoleMenu from "./menu/ConsoleMenu";
import ConsolePanel from "./panel/ConsolePanel";

const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

const options = (token: string, slug: string) => {
  return {
    url: `${BENCHER_API_URL}/v0/projects/${slug}`,
    method: "get",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
  };
};

const fetchProject = async (slug) => {
  try {
    const token = JSON.parse(window.localStorage.getItem("user"))?.uuid;
    if (typeof token !== "string") {
      return;
    }
    const resp = await axios(options(token, slug));
    const data = resp?.data;
    console.log(data);
    return data;
  } catch (error) {
    console.error(error);
  }
};

interface Project {
  uuid: string;
  name: string;
  slug: string;
}

const ConsolePage = (props) => {
  const [project_slug, setProjectSlug] = createSignal<String>(null);
  const [project] = createResource(project_slug, fetchProject);

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
              current_location={props.current_location}
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
