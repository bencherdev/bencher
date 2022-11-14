import { useParams } from "solid-app-router";
import {
  createEffect,
  createMemo,
  createResource,
  createSignal,
} from "solid-js";
import { BENCHER_API_URL, getToken } from "../site/util";
import ConsoleMenu from "./menu/ConsoleMenu";
import ConsolePanel from "./panel/ConsolePanel";
import validator from "validator";
import axios from "axios";

export const organizationSlug = (pathname) => {
  const path = pathname().split("/");
  if (
    path.length < 5 ||
    path[0] ||
    path[1] !== "console" ||
    path[2] !== "organizations" ||
    !path[3] ||
    !path[4]
  ) {
    return null;
  }
  return path[3];
};

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

const options = (token: string, project_slug: string) => {
  return {
    url: `${BENCHER_API_URL()}/v0/projects/${project_slug}`,
    method: "GET",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
  };
};

const fetchProject = async (project_slug: string) => {
  try {
    const token = getToken();
    if (token && !validator.isJWT(token)) {
      return null;
    }

    const resp = await axios(options(token, project_slug));
    return resp?.data;
  } catch (error) {
    console.error(error);
    return null;
  }
};

const ConsolePage = (props) => {
  const [count, setCount] = createSignal(0);

  const params = useParams();
  const path_params = createMemo(() => params);

  const [project] = createResource(props.project_slug, fetchProject);

  createEffect(() => {
    const organization_uuid = project()?.organization;
    if (organization_uuid) {
      props.handleOrganizationSlug(organization_uuid);
    } else {
      const slug = organizationSlug(props.pathname);
      props.handleOrganizationSlug(slug);
    }
  });

  createEffect(() => {
    const slug = projectSlug(props.pathname);
    props.handleProjectSlug(slug);
  });

  setInterval(() => {
    if (props.user()?.token === null) {
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
          <div class="column is-narrow">
            <ConsoleMenu
              organization_slug={props.organization_slug}
              project_slug={props.project_slug}
              handleRedirect={props.handleRedirect}
              handleProjectSlug={props.handleProjectSlug}
            />
          </div>
          <div class="column">
            <ConsolePanel
              project_slug={props.project_slug}
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
