const BENCHER_UI_URL = "http://localhost:3000";

export function Navbar() {
    return (
        <nav class="navbar" role="navigation" aria-label="main navigation">
        <div class="navbar-brand">
          <a class="navbar-item" href={BENCHER_UI_URL}>
            <img src="/bencher_navbar.png" width="152" height="28"/>
          </a>
      
          <a role="button" class="navbar-burger" aria-label="menu" aria-expanded="false" data-target="bencherNavbar"
            onClick={() => {
              let toggle = document.querySelector(".navbar-burger");
              let menu = document.querySelector(".navbar-menu");
              toggle.classList.toggle("is-active");
              menu.classList.toggle("is-active");
            }} 
          >
            <span aria-hidden="true"></span>
            <span aria-hidden="true"></span>
            <span aria-hidden="true"></span>
          </a>
        </div>
      
        <div id="navbarBasicExample" class="navbar-menu">
          <div class="navbar-start">
            <a class="navbar-item" href={BENCHER_UI_URL}>
              Docs
            </a>

            <a class="navbar-item" href={BENCHER_UI_URL}>
              API
            </a>
          </div>
      
          <div class="navbar-end">
            <div class="navbar-item">
              <div class="buttons">
                <a class="button is-light" href={BENCHER_UI_URL}>
                  Log in
                </a>
                <a class="button is-primary" href={BENCHER_UI_URL}>
                  <strong>Sign up</strong>
                </a>
              </div>
            </div>
          </div>
        </div>
      </nav>
    );
  }