import axios from "axios";
import { createSignal, createResource, createEffect, For } from "solid-js";
import { getToken } from "../../site/util";
import validator from "validator";

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
  const all_projects = {
    name: "All Projects",
    slug: BENCHER_ALL_PROJECTS,
  };

  try {
    const token = getToken();
    if (token && !validator.isJWT(token)) {
      return [all_projects];
    }

    console.log(token);
    const resp = await axios(options(token));
    let data = resp?.data;
    data.push(all_projects);

    return data;
  } catch (error) {
    console.error(error);
    return [all_projects];
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
    let path: string;
    if (selected() === BENCHER_ALL_PROJECTS) {
      path = "/console/projects";
    } else {
      path = `/console/projects/${selected()}/perf`;
    }
    props.handleRedirect(path);
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

  return (
    <div class="select">
      <select onInput={(e) => handleInput(e)}>
        <For each={projects()}>
          {(project) => (
            <option value={project.slug} selected={project.slug === selected()}>
              {project.name}
            </option>
          )}
        </For>
      </select>
    </div>
  );
};

export default ProjectSelect;
