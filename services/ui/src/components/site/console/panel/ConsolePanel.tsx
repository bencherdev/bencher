import { Match, Switch } from "solid-js";
import TablePanel from "./TablePanel";
import DeckPanel from "./DeckPanel";

const isSection = (current_location, section: string) => {
  if (
    (section === null && current_location.pathname === "/console") ||
    current_location.pathname === `/console/${section}`
  ) {
    return true;
  }
  return false;
};

const isSubSection = (current_location, section: string) => {
  return current_location.pathname.includes(`/console/${section}`);
};

const ConsolePanel = (props) => {
  props.handleTitle("Bencher Console - Track Your Benchmarks");

  return (
    <Switch
      fallback={
        <p>Unknown console path: {props.current_location().pathname}</p>
      }
    >
      <Match when={isSection(props.current_location(), null)}>
        TODO Project Dashboard
      </Match>
      <Match when={isSection(props.current_location(), "reports")}>
        <TablePanel
          current_location={props.current_location}
          handleRedirect={props.handleRedirect}
        />
      </Match>
      <Match when={isSubSection(props.current_location(), "reports")}>
        <DeckPanel
          current_location={props.current_location}
          handleRedirect={props.handleRedirect}
        />
      </Match>
    </Switch>
  );
};

export default ConsolePanel;
