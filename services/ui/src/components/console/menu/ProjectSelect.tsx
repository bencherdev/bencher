import axios from "axios";
import {
  createSignal,
  createResource,
  createEffect,
  Suspense,
  For,
} from "solid-js";

const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;
const BENCHER_ALL_PROJECTS = "--bencher--all---projects--";

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
    const resp = await axios(options(token));
    let data = resp?.data;
    data.push({
      name: "All Projects",
      slug: BENCHER_ALL_PROJECTS,
    });
    return data;
  } catch (error) {
    console.error(error);
  }
};

const ProjectSelect = (props) => {
  const getSelected = () => {
    const slug = props.project_slug();
    console.log(slug);
    if (slug === null) {
      return BENCHER_ALL_PROJECTS;
    } else {
      return slug;
    }
  };

  const [selected, setSelected] = createSignal(getSelected());
  const [projects] = createResource(selected, fetchProjects);

  createEffect(() => {
    const slug = props.project_slug();
    if (slug === null) {
      setSelected(BENCHER_ALL_PROJECTS);
    } else {
      setSelected(slug);
    }
  });

  const handleSelectedRedirect = () => {
    props.handleRedirect(`/console/projects/${selected()}/perf`);
  };

  const handleInput = (e) => {
    const target_slug = e.currentTarget.value;
    console.log(target_slug);
    if (target_slug === BENCHER_ALL_PROJECTS) {
      setSelected(target_slug);
      props.handleProjectSlug(null);
      props.handleRedirect("/console/projects");
      return;
    }

    const p = projects();
    for (let i in p) {
      const project = p[i];
      const slug = project?.slug;
      if (slug === target_slug) {
        props.handleProjectSlug(slug);
        handleSelectedRedirect();
        break;
      }
    }
  };

  const isSelected = (slug) => {
    return slug === selected();
  };

  const isAllProjects = () => {
    return BENCHER_ALL_PROJECTS === selected();
  };

  return (
    <nav class="level">
      <div class="level-left">
        <div class="control">
          {selected() !== BENCHER_ALL_PROJECTS && (
            <button
              class="button is-outlined"
              onClick={(e) => {
                e.preventDefault();
                handleSelectedRedirect();
              }}
            >
              <span class="icon">
                <i class="fas fa-home" aria-hidden="true"></i>
              </span>
            </button>
          )}
          <div class="select">
            <select onInput={(e) => handleInput(e)}>
              <For each={projects()}>
                {(project) => (
                  <option
                    value={project.slug}
                    selected={isSelected(project.slug)}
                  >
                    {project.name}
                  </option>
                )}
              </For>
            </select>
          </div>
        </div>
      </div>
    </nav>
  );
};

export default ProjectSelect;
