import { describe, expect, test } from "vitest";
import { render } from "solid-testing-library";
import LandingPage from "../src/components/site/pages/LandingPage";
import { Router } from "solid-app-router";
import { createSignal } from "solid-js";
import { default_user } from "../src/components/site/util";

// https://github.com/vitest-dev/vitest/tree/main/examples/solid
describe("<LandingPage />", () => {
	test("renders", () => {
		const [user, _setUser] = createSignal(default_user());
		const { container, unmount } = render(() => (
			<Router>
				<LandingPage user={user} />
			</Router>
		));
		expect(container.innerHTML).toMatchSnapshot();
		unmount();
	});
});
