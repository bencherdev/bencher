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
              <button class="button is-outlined">
                <Switch fallback={<></>}>
                  <Match when={button === Button.ADD}>
                    <span class="icon">
                      <i class="fas fa-plus" aria-hidden="true"></i>
                    </span>
                    <span>Add</span>
                  </Match>
                  <Match when={button === Button.REFRESH}>
                    <span class="icon">
                      <i class="fas fa-sync-alt" aria-hidden="true"></i>
                    </span>
                    <span>Refresh</span>
                  </Match>
                </Switch>
              </button>
            </p>
          )}
        </For>
      </div>
    </nav>
  );
};

export default TableHeader;
