import { Link } from "solid-app-router";

import ProjectSelect from "./ProjectSelect";

const ConsoleMenu = (props) => {
  const getProjectPath = (section: string) => {
    return `/console/projects/${props.project_slug()}/${section}`;
  };

  return (
    <aside class="menu">
      <div class="menu-label">
        <ProjectSelect
          project_slug={props?.project_slug}
          handleRedirect={props?.handleRedirect}
          handleProjectSlug={props?.handleProjectSlug}
        />
      </div>
      {typeof props.project_slug() === "string" && (
        <>
          <p class="menu-label">Project</p>
          <ul class="menu-list">
            <li>
              <Link href={getProjectPath("reports")}>Reports</Link>
            </li>
            <li>
              <Link href={getProjectPath("branches")}>Branches</Link>
            </li>
            <li>
              <Link href={getProjectPath("testbeds")}>Testbeds</Link>
            </li>
            <li>
              <Link href={getProjectPath("connections")}>Connections</Link>
            </li>
            <li>
              <Link href={getProjectPath("settings")}>Settings</Link>
            </li>
          </ul>
        </>
      )}
      <p class="menu-label">User</p>
      <ul class="menu-list">
        <li>
          <Link href="/console/user/account">Account</Link>
        </li>
        <li>
          <Link href="/console/user/settings">Settings</Link>
        </li>
      </ul>
    </aside>
  );
};

export default ConsoleMenu;
