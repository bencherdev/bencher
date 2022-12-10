import axios from "axios";
import { createEffect, createMemo, createResource } from "solid-js";
import { BENCHER_API_URL, get_options, validate_jwt } from "../site/util";

const PerfPage = (props) => {
  const url = createMemo(() => `${BENCHER_API_URL()}/v0/projects`);

  // const fetchProject = async () => {
  //   const EMPTY_OBJECT = {};
  //   try {
  //     const token = props.user()?.token;
  //     if (!validate_jwt(props.user()?.token)) {
  //       return EMPTY_OBJECT;
  //     }

  //     const resp = await axios(get_options(url(), token));
  //     return resp?.data;
  //   } catch (error) {
  //     console.error(error);
  //     return EMPTY_OBJECT;
  //   }
  // };

  // const [project] = createResource(props.project_slug, fetchProject);

  // createEffect(() => {
  //   // if (!validate_jwt(props.user()?.token)) {
  //   //   navigate("/auth/login");
  //   // }

  //   const organization_uuid = project()?.organization;
  //   if (organization_uuid) {
  //     props.handleOrganizationSlug(organization_uuid);
  //   } else {
  //     const slug = organizationSlug(pathname);
  //     props.handleOrganizationSlug(slug);
  //   }

  //   const project_slug = projectSlug(pathname);
  //   props.handleProjectSlug(project_slug);
  // });

  return (
    <section class="section">
      {/* <div class="container">
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
              user={props.user}
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
      </div> */}
    </section>
  );
};

export default PerfPage;
