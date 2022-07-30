import axios from "axios";
import {
  createSignal,
  createResource,
  createEffect,
  Suspense,
  For,
} from "solid-js";

const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

const options = (token: string) => {
  return {
    url: `${BENCHER_API_URL}/v0/projects`,
    method: "get",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
  };
};

const fetchProjects = async () => {
  try {
    const token = JSON.parse(window.localStorage.getItem("user"))?.uuid;
    if (typeof token !== "string") {
      return;
    }
    let reports = await axios(options(token));
    console.log("GET");
    return reports.data;
  } catch (error) {
    console.error(error);
  }
};

const BENCHER_SEE_ALL = "bencher--see---all";

const ProjectSelect = (props) => {
  const [projects] = createResource(props.project, fetchProjects);
  const [selected, setSelected] = createSignal(BENCHER_SEE_ALL);

  const handleProject = (e) => {
    const target_slug = e?.target?.value;
    if (target_slug === BENCHER_SEE_ALL) {
      setSelected(BENCHER_SEE_ALL);
      props.handleProject({
        uuid: null,
        name: null,
        slug: null,
      });
    }
    console.log(target_slug);
    const p = projects();
    for (let i in p) {
      const project = p[i];
      const slug = project?.slug;
      if (slug === target_slug) {
        console.log("MATCH");
        setSelected(slug);
        props.handleProject(project);
        break;
      }
    }
  };

  const isSelected = (slug) => {
    return slug === selected();
  };

  const isSeeAll = () => {
    return BENCHER_SEE_ALL === selected();
  };

  return (
    <nav class="level">
      <div class="level-left">
        <div class="control">
          <button class="button is-outlined is-left">
            <span class="icon">
              <i class="fas fa-home" aria-hidden="true"></i>
            </span>
          </button>
          <div class="select">
            <select
              onChange={(e) => {
                handleProject(e);
              }}
            >
              <optgroup label="Projects">
                <For each={projects()}>
                  {(project, i) => (
                    <option
                      value={project?.slug}
                      selected={isSelected(project?.slug)}
                    >
                      {project?.name}
                    </option>
                  )}
                </For>
              </optgroup>
              <option value={BENCHER_SEE_ALL} selected={isSeeAll()}>
                See All
              </option>
            </select>
          </div>
        </div>
      </div>
    </nav>
  );
};

export default ProjectSelect;
