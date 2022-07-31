import { createEffect, createSignal, Match, Switch } from "solid-js";
import TablePanel from "./table/TablePanel";
import DeckPanel from "./deck/DeckPanel";
import { JsonNewTestbed } from "bencher_json";
import { Operation } from "../console";
import PerfPanel from "./perf/PerfPanel";
import PosterPanel from "./poster/PosterPanel";

const ConsolePanel = (props) => {
  return (
    <Switch fallback={<p>Unknown console path: {props.pathname()} </p>}>
      <Match when={props.config?.operation === Operation.LIST}>
        <TablePanel
          config={props.config}
          pathname={props.pathname}
          handleRedirect={props.handleRedirect}
        />
      </Match>
      <Match when={props.config?.operation === Operation.ADD}>
        <PosterPanel
          config={props.config}
          pathname={props.pathname}
          handleRedirect={props.handleRedirect}
        />
      </Match>
      <Match when={props.config?.operation === Operation.VIEW}>
        <DeckPanel
          config={props.config}
          path_params={props.path_params}
          pathname={props.pathname}
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
