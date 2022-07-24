import { Match, Switch } from "solid-js";
import ConsoleTable from "./ConsoleTable";

const isSection = (current_location, section: string) => {
  if (
    (section === null && current_location.pathname === "/console") ||
    current_location.pathname === `/console/${section}`
  ) {
    return true;
  }
  return false;
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
        <ConsoleTable
          current_location={props.current_location}
          handleRedirect={props.handleRedirect}
        />
      </Match>
    </Switch>
  );
};

export default ConsolePanel;
