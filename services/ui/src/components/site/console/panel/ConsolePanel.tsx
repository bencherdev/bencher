import { createEffect, createSignal, Match, Switch } from "solid-js";
import TablePanel from "./TablePanel";
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
      <Match when={props.operation === Operation.LIST}>
        <TablePanel
          current_location={props.current_location}
          handleRedirect={props.handleRedirect}
        />
      </Match>
      <Match when={props.operation === Operation.VIEW}>
        <DeckPanel
          current_location={props.current_location}
          handleRedirect={props.handleRedirect}
          handleProjectSlug={props.handleProjectSlug}
        />
      </Match>
      <Match when={props.operation === Operation.PERF}>
        <PerfPanel />
      </Match>
    </Switch>
  );
};

export default ConsolePanel;
