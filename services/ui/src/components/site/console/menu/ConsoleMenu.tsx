import { Link } from "solid-app-router";

const ConsoleMenu = (props) => {
  return (
    <aside class="menu">
      <div class="menu-label dropdown project-menu">
        <div class="dropdown-trigger">
          <button
            class="button"
            aria-haspopup="true"
            aria-controls="dropdown-menu"
            onClick={() => {
              let project_menu = document.querySelector(".project-menu");
              project_menu.classList.toggle("is-active");
            }}
          >
            <span>Default Project</span>
            <span class="icon is-small">
              <i class="fas fa-angle-down" aria-hidden="true"></i>
            </span>
          </button>
        </div>
        <div class="dropdown-menu" id="dropdown-menu" role="menu">
          <div class="dropdown-content">
            <Link
              class="dropdown-item"
              href="/console/projects/default_project_uuid"
            >
              Default Project
            </Link>
            <hr class="dropdown-divider" />
            <Link class="dropdown-item" href="/console/projects">
              View All
            </Link>
          </div>
        </div>
      </div>
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
