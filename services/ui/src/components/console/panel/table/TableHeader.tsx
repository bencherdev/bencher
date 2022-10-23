import { createEffect, createResource, For, Match, Switch } from "solid-js";
import { OrganizationPermission } from "../../../site/util";
import { Button } from "../../config/types";

const TableHeader = (props) => {
  createEffect(() => {
    const title = props.config?.title;
    if (title) {
      props.handleTitle(title);
    }
  });

  return (
    <nav class="level">
      <div class="level-left">
        <div class="level-item">
          <h3 class="title is-3">{props.config?.title}</h3>
        </div>
      </div>

      <div class="level-right">
        <For each={props.config?.buttons}>
          {(button) => (
            <TableHeaderButton
              pathname={props.pathname}
              path_params={props.path_params}
              handleRefresh={props.handleRefresh}
              handleRedirect={props.handleRedirect}
              button={button}
            />
          )}
        </For>
      </div>
    </nav>
  );
};

const TableHeaderButton = (props) => {
  const [is_allowed] = createResource(props.path_params, (path_params) => props.button.is_allowed?.(path_params));

  return (
    <p class="level-item">
      <Switch fallback={<></>}>
        <Match when={props.button.kind === Button.ADD}>
          <button
            class="button is-outlined"
            onClick={(e) => {
              e.preventDefault();
              props.handleRedirect(props.button.path(props.pathname()));
            }}
          >
            <span class="icon">
              <i class="fas fa-plus" aria-hidden="true" />
            </span>
            <span>Add</span>
          </button>
        </Match>
        <Match when={props.button.kind === Button.INVITE && is_allowed()}>
          <button
            class="button is-outlined"
            onClick={(e) => {
              e.preventDefault();
              props.handleRedirect(props.button.path(props.pathname()));
            }}
          >
            <span class="icon">
              <i class="fas fa-envelope" aria-hidden="true" />
            </span>
            <span>Invite</span>
          </button>
        </Match>
        <Match when={props.button.kind === Button.REFRESH}>
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
        </Match>
      </Switch>
    </p>
  );
};

export default TableHeader;
