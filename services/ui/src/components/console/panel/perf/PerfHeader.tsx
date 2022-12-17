import axios from "axios";
import { createEffect, createResource } from "solid-js";
import { get_options, pageTitle } from "../../../site/util";

const PerfHeader = (props) => {
  const getProject = async (token: null | string) => {
    try {
      const url = props.config?.url(props.path_params());
      const resp = await axios(get_options(url, props.user()?.token));
      return resp.data;
    } catch (error) {
      console.error(error);
      return [];
    }
  };

  const [project_data] = createResource(props.refresh, getProject);

  createEffect(() => {
    pageTitle(project_data()?.name);
  });

  return (
    <nav class="level">
      <div class="level-left">
        <div class="level-item">
          <h3 class="title is-3">{project_data()?.name}</h3>
        </div>
      </div>

      <div class="level-right">
        <div class="level-item">
          <button
            class="button is-outlined"
            onClick={(e) => {
              e.preventDefault();
              navigator.clipboard.writeText(window.location.href);
            }}
          >
            <span class="icon">
              <i class="fas fa-link" aria-hidden="true" />
            </span>
            <span>Copy Link</span>
          </button>
        </div>
        <div class="level-item">
          <button
            class="button is-outlined"
            onClick={(e) => {
              e.preventDefault();
              props.handleRefresh();
            }}
          >
            <span class="icon">
              <i class="fas fa-sync-alt" aria-hidden="true" />
            </span>
            <span>Refresh</span>
          </button>
        </div>
      </div>
    </nav>
  );
};

export default PerfHeader;
