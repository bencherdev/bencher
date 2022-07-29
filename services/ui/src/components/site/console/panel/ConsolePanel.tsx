import { createEffect, createSignal, Match, Switch } from "solid-js";
import TablePanel from "./TablePanel";
import DeckPanel from "./DeckPanel";
import { JsonNewTestbed } from "bencher_json";

interface Panel {
  section: string;
  operation: string;
  slug: string | null;
}

interface MatchPanel {
  section: string;
  operation: string;
  slug: boolean;
}

const projectsPath = (context: string, path: Array<string>) => {
  if (path[0]) {
    return;
  }
  if (path[1] !== context) {
    return;
  }
  if (path[2] !== "projects") {
    return;
  }
  return {
    section: Section.PROJECTS,
    operation: path[3] ? Operation.VIEW : Operation.LIST,
    slug: path[3] || null,
  };
};

const Section = {
  PROJECTS: "projects",
};

const Operation = {
  ADD: "add",
  LIST: "list",
  VIEW: "view",
  EDIT: "edit",
  DELETE: "delete",
};

const initPanel = () => {
  return {
    section: null,
    operation: null,
    slug: null,
  };
};

const handlePanel = (props, setPanel) => {
  const current_location = props.current_location();
  const pathname = current_location.pathname?.split("/");
  const panel: Panel = projectsPath("console", pathname);
  console.log(panel);
  setPanel(panel);
};

const ConsolePanel = (props) => {
  const [pathname, setPathname] = createSignal<string | null>();
  const [panel, setPanel] = createSignal<Panel | null>();

  // props.handleTitle("Bencher Console - Track Your Benchmarks");

  createEffect(() => {
    if (pathname() !== props.current_location().pathname) {
      setPathname(props.current_location().pathname);
      handlePanel(props, setPanel);
    }
  });

  const matchPanel = (match_panel: MatchPanel) => {
    const p: Panel = panel();
    if (p === null) {
      return false;
    } else if (
      p?.section === match_panel?.section &&
      p?.operation === match_panel?.operation &&
      ((p?.slug === null && !match_panel?.slug) ||
        (typeof p?.slug === "string" && match_panel?.slug))
    ) {
      return true;
    } else {
      return false;
    }
  };

  return (
    <Switch
      fallback={
        <p>
          Unknown console path: {props.current_location().pathname} for:{" "}
          {panel()}
        </p>
      }
    >
      <Match
        when={matchPanel({
          section: Section.PROJECTS,
          operation: Operation.LIST,
          slug: false,
        })}
      >
        <TablePanel
          current_location={props.current_location}
          handleRedirect={props.handleRedirect}
        />
      </Match>
      <Match
        when={matchPanel({
          section: Section.PROJECTS,
          operation: Operation.VIEW,
          slug: true,
        })}
      >
        <DeckPanel
          current_location={props.current_location}
          handleRedirect={props.handleRedirect}
        />
      </Match>
    </Switch>
  );
};

export default ConsolePanel;
