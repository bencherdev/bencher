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
            <a href="#" class="dropdown-item is-active">
              Default Project
            </a>
            <a href="#" class="dropdown-item">
              TODO List projects
            </a>
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
          <a>Reports</a>
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
          <a>Account</a>
        </li>
        <li>
          <a>Settings</a>
        </li>
      </ul>
    </aside>
  );
};

export default ConsoleMenu;
