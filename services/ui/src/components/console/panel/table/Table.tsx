import { Link } from "solid-app-router";
import { For, Switch, Match } from "solid-js";
import { Row } from "../../config/types";

const Table = (props) => {
  return (
    <>
      {props.table_data()?.length === 0 ? (
        <div class="box">
          <AddButton pathname={props.pathname} add={props.config?.add} />
        </div>
      ) : (
        <div class="pricing-table is-horizontal">
          <For each={props.table_data()}>
            {(datum, i) => (
              <div class="pricing-plan is-primary">
                <div class="plan-header">
                  <p style="overflow-wrap:break-word;">
                    {datum[props.config?.row?.key]}
                  </p>
                </div>
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
                          <Match when={item.kind === Row.SELECT}>
                            {item.value?.options.reduce((field, option) => {
                              if (datum[item.key] === option.value) {
                                return option.option;
                              } else {
                                return field;
                              }
                            }, datum[item.key])}
                          </Match>
                        </Switch>
                      </div>
                    )}
                  </For>
                </div>
                <div class="plan-footer">
                  <RowButton
                    handleRedirect={props.handleRedirect}
                    config={props.config?.row?.button}
                    pathname={props.pathname}
                    datum={datum}
                  />
                </div>
              </div>
            )}
          </For>
        </div>
      )}
    </>
  );
};

const AddButton = (props) => {
  return (
    <Link
      class="button is-primary is-fullwidth"
      href={props.add?.path(props.pathname())}
    >
      {props.add?.text}
    </Link>
  );
};

const RowButton = (props) => {
  return (
    <button
      class="button is-fullwidth"
      onClick={(e) => {
        e.preventDefault();
        props.handleRedirect(
          props.config?.path?.(props.pathname(), props.datum)
        );
      }}
    >
      {props.config?.text}
    </button>
  );
};

export default Table;
