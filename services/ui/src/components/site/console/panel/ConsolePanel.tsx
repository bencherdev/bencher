import { createEffect, createSignal, Match, Switch } from "solid-js";
import TablePanel from "./table/TablePanel";
import DeckPanel from "./DeckPanel";
import { JsonNewTestbed } from "bencher_json";
import { Operation } from "../console";
import PerfPanel from "./PerfPanel";

const ConsolePanel = (props) => {
  return (
    <Switch
      fallback={
        <p>Unknown console path: {props.current_location().pathname} </p>
      }
    >
      <Match when={props.config?.operation === Operation.LIST}>
        <TablePanel
          config={props.config}
          handleRedirect={props.handleRedirect}
        />
      </Match>
      <Match when={props.config?.operation === Operation.VIEW}>
        <DeckPanel
          config={props.config}
          path_params={props.path_params}
          current_location={props.current_location}
          handleRedirect={props.handleRedirect}
        />
      </Match>
      <Match when={props.config?.operation === Operation.PERF}>
        <PerfPanel config={props.config} />
      </Match>
    </Switch>
  );
};

export default ConsolePanel;
