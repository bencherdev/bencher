import { createSignal } from "solid-js";
import { THEME_KEY, THEME_TOGGLE_ID, Theme, ThemeId, getTheme } from "./theme";
import { BENCHER_WORDMARK, BENCHER_WORDMARK_DARK, BENCHER_WORDMARK_FOOTER_ID, BENCHER_WORDMARK_ID } from "../../../util/ext";

const ThemeToggle = () => {
  const [theme, setTheme] = createSignal(Theme.Light);
	setInterval(() => {
		const newTheme = getTheme();
		if (theme() != newTheme) {
			setTheme(newTheme);
		}
	}, 100);


  const themeTitle = () => {
    switch (theme()) {
      case Theme.Light:
        return "Toggle dark mode";
      case Theme.Dark:
        return "Toggle light mode";
    }
  }

  const sunTheme = () => {
    switch (theme()) {
      case Theme.Light:
        return "has-text-primary";
      case Theme.Dark:
        return "has-text-dark";
    }
  }

  const moonTheme = () => {
    switch (theme()) {
      case Theme.Light:
        return "has-text-light";
      case Theme.Dark:
        return "has-text-primary";
    }
  }

  const handleTheme = () => {
    const wordmark = document.getElementById(BENCHER_WORDMARK_ID);
    const wordmarkFooter = document.getElementById(BENCHER_WORDMARK_FOOTER_ID);
    const newTheme = (() => {switch(theme()) {
      case Theme.Light:
        wordmark ? wordmark.src = BENCHER_WORDMARK_DARK : null;
        wordmarkFooter ? wordmarkFooter.src = BENCHER_WORDMARK_DARK : null;
        return setTheme(Theme.Dark);
      case Theme.Dark:
        wordmark ? wordmark.src = BENCHER_WORDMARK : null;
        wordmarkFooter ? wordmarkFooter.src = BENCHER_WORDMARK : null;
        return setTheme(Theme.Light);
    }})();
    document.documentElement.setAttribute("data-theme", newTheme);
    window.localStorage.setItem(THEME_KEY, newTheme);
  }

  return (
    <button id={THEME_TOGGLE_ID} class="button" title={themeTitle()} onClick={(e) => {
      e.preventDefault();
      handleTheme();
    }}>
      <span id={ThemeId.Light} class={`icon ${sunTheme()}`}>
        <i class="fas fa-sun" aria-hidden="true" />
      </span>
      <span id={ThemeId.Dark} class={`icon ${moonTheme()}`}>
        <i class="far fa-moon" aria-hidden="true" />
      </span>
    </button>
  );
}

export default ThemeToggle;
