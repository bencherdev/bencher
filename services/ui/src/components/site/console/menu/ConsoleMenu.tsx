import { Link } from "solid-app-router";

import ProjectSelect from "./ProjectSelect";

const ConsoleMenu = (props) => {
  return (
    <aside class="menu">
      <div class="menu-label">
        <ProjectSelect
          project={props?.project}
          handleRedirect={props.handleRedirect}
          handleProject={props?.handleProject}
        />
      </div>
      {typeof props?.project()?.slug === "string" && (
        <>
          <p class="menu-label">Project</p>
          <ul class="menu-list">
            <li>
              <a>Benchmarks</a>
            </li>
            <li>
              <a>Testbeds</a>
            </li>
            <li>
              <Link href="/console/reports">Reports</Link>
            </li>
            <li>
              <a>Connections</a>
            </li>
            <li>
              <a>Settings</a>
            </li>
          </ul>
        </>
      )}
      <p class="menu-label">User</p>
      <ul class="menu-list">
        <li>
          <Link href="/console/account">Account</Link>
        </li>
        <li>
          <a>Settings</a>
        </li>
      </ul>
    </aside>
  );
};

export default ConsoleMenu;
