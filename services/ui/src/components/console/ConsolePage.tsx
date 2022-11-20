import { useLocation, useNavigate, useParams } from "solid-app-router";
import {
  createEffect,
  createMemo,
  createResource,
  createSignal,
  For,
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
  const navigate = useNavigate();
  const location = useLocation();
  const pathname = createMemo(() => location.pathname);
  const params = useParams();
  const path_params = createMemo(() => params);

  const [project] = createResource(props.project_slug, fetchProject);

  createEffect(() => {
    if (!(props.user().token && validator.isJWT(props.user().token))) {
      navigate("/auth/login");
    }

    const organization_uuid = project()?.organization;
    if (organization_uuid) {
      props.handleOrganizationSlug(organization_uuid);
    } else {
      const slug = organizationSlug(pathname);
      props.handleOrganizationSlug(slug);
    }

    const project_slug = projectSlug(pathname);
    props.handleProjectSlug(project_slug);
  });

  return (
    <section class="section">
      <div class="container">
        <div class="columns is-reverse-mobile">
          <div class="column is-narrow">
            <ConsoleMenu
              organization_slug={props.organization_slug}
              project_slug={props.project_slug}
              handleProjectSlug={props.handleProjectSlug}
            />
          </div>
          <div class="column">
            <ConsolePanel
              project_slug={props.project_slug}
              operation={props.operation}
              config={props.config}
              path_params={path_params}
              handleProjectSlug={props.handleProjectSlug}
            />
            <For each={[...Array(3).keys()]}>{(_k, _i) => <br />}</For>
            <hr />
          </div>
        </div>
      </div>
    </section>
  );
};

export default ConsolePage;
