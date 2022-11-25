import axios from "axios";
import { createSignal, createResource, createEffect, For } from "solid-js";
import { BENCHER_API_URL, getToken, validate_jwt } from "../../site/util";
import { useNavigate } from "solid-app-router";

const BENCHER_ALL_PROJECTS = "--bencher--all---projects--";

const ProjectSelect = (props) => {
  const navigate = useNavigate();

  const options = (token: string) => {
    return {
      url: `${BENCHER_API_URL()}/v0/organizations/${props.organization_slug()}/projects`,
      method: "GET",
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
      if (!validate_jwt(token)) {
        return [all_projects];
      }

      const resp = await axios(options(token));
      let data = resp?.data;
      data.push(all_projects);

      return data;
    } catch (error) {
      console.error(error);
      return [all_projects];
    }
  };

  const getSelected = () => {
    const slug = props.project_slug();
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
      path = `/console/organizations/${props.organization_slug()}/projects`;
    } else {
      path = `/console/projects/${selected()}/perf`;
    }
    navigate(path);
  };

  const handleInput = (e) => {
    const target_slug = e.currentTarget.value;
    if (target_slug === BENCHER_ALL_PROJECTS) {
      setSelected(target_slug);
      props.handleProjectSlug(null);
      navigate(`/console/organizations/${props.organization_slug()}/projects`);
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
