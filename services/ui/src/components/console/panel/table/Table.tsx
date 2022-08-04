import { For, Switch, Match } from "solid-js";
import { Row } from "../../config/types";

const Table = (props) => {
  const handleRowButton = (event, datum) => {
    event.preventDefault();
    props.handleRedirect(props.config?.row?.path(props.pathname(), datum));
  };

  return (
    <div class="pricing-table is-horizontal">
      <For each={props.table_data()}>
        {(datum, i) => (
          <div class="pricing-plan is-primary">
            <div class="plan-header">{datum[props.config?.row?.key]}</div>
            <div class="plan-items">
              <For each={props.config?.row?.items}>
                {(item, i) => (
                  <div class="plan-item">
                    <Switch fallback="-">
                      <Match when={item.kind === Row.TEXT}>
                        {datum[item.key]}
                      </Match>
                      <Match when={item.kind === Row.BOOL}>
                        {item.text}: {datum[item.key] ? "true" : "false"}
                      </Match>
                    </Switch>
                  </div>
                )}
              </For>
            </div>
            <div class="plan-footer">
              <button
                class="button is-fullwidth"
                onClick={(e) => handleRowButton(e, datum)}
              >
                View
              </button>
            </div>
          </div>
        )}
      </For>
    </div>
  );
};

export default Table;
