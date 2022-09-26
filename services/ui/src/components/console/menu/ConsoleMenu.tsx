import { Link } from "solid-app-router";

const ConsoleMenu = (props) => {
  const getProjectPath = (section: string) => {
    return `/console/projects/${props.project_slug()}/${section}`;
  };

  return (
    <aside class="menu">
      {typeof props.project_slug() === "string" && (
        <>
          <div class="menu-label">
            <button
              class="button is-outlined is-fullwidth"
              onClick={(e) => {
                e.preventDefault();
                props.handleRedirect(
                  `/console/projects/${props.project_slug()}/perf`
                );
              }}
            >
              <span class="icon">
                <i class="fas fa-home" aria-hidden="true" />
              </span>
            </button>
          </div>
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
              <Link href={getProjectPath("thresholds")}>Thresholds</Link>
            </li>
            <li>
              <Link href={getProjectPath("alerts")}>Alerts</Link>
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
