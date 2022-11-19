import { createEffect } from "solid-js";
import { pageTitle } from "../../../site/util";

const PerfHeader = (props) => {
  createEffect(() => {
    pageTitle(props.config?.title);
  });

  return (
    <nav class="level">
      <div class="level-left">
        <div class="level-item">
          <h3 class="title is-3">{props.config?.title}</h3>
        </div>
      </div>

      <div class="level-right">
        <button
          class="button is-outlined"
          // disabled={props.refresh() > 0}
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
    </nav>
  );
};

export default PerfHeader;
