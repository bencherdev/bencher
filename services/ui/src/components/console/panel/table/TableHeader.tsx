import { For, Match, Switch } from "solid-js";
import { Button } from "../../console";

const TableHeader = (props) => {
  return (
    <nav class="level">
      <div class="level-left">
        <div class="level-item">
          <h3 class="title is-3">{props.title}</h3>
        </div>
      </div>

      <div class="level-right">
        <For each={props.buttons}>
          {(button, i) => (
            <p class="level-item">
              <Switch fallback={<></>}>
                <Match when={button.kind === Button.ADD}>
                  <button
                    class="button is-outlined"
                    onClick={(e) => {
                      e.preventDefault();
                      props.handleRedirect(button.path);
                    }}
                  >
                    <span class="icon">
                      <i class="fas fa-plus" aria-hidden="true"></i>
                    </span>
                    <span>Add</span>
                  </button>
                </Match>
                <Match when={button.kind === Button.REFRESH}>
                  <button
                    class="button is-outlined"
                    // disabled={props.refresh() > 0}
                    onClick={(e) => {
                      e.preventDefault();
                      props.handleRefresh();
                    }}
                  >
                    <span class="icon">
                      <i class="fas fa-sync-alt" aria-hidden="true"></i>
                    </span>
                    <span>Refresh</span>
                  </button>
                </Match>
              </Switch>
            </p>
          )}
        </For>
      </div>
    </nav>
  );
};

export default TableHeader;
