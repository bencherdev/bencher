import { afterEach, expect, test } from "vitest";
import built from "../../../../../plus/api_mcp/ui/perf.html?raw";
import { fakeHost, perf, success } from "./testing";

// What a host that renders MCP Apps must apply to a page that declares no CSP.
const DEFAULT_CSP =
	"default-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; media-src 'self' data:; connect-src 'none'";

// Runs before the page's own script: it records what the CSP blocks and
// whether Inter is ready when the plot's first canvas lands, and holds back
// every font load, so a plot that does not wait for Inter draws without it.
const PROBE = `<script>
window.blocked = [];
document.addEventListener("securitypolicyviolation", (event) => {
	blocked.push(event.effectiveDirective + " " + event.blockedURI);
});
const load = FontFace.prototype.load;
FontFace.prototype.load = function () {
	return new Promise((resolve) => setTimeout(resolve, 300)).then(() => load.call(this));
};
window.interReady = () =>
	[...document.fonts].some((face) => face.family.replaceAll('"', "") === "Inter" && face.status === "loaded");
new MutationObserver(() => {
	if (window.interAtFirstDraw === undefined && document.querySelector("canvas")) {
		window.interAtFirstDraw = interReady();
	}
}).observe(document, { childList: true, subtree: true });
</script>`;

interface Probed extends Window {
	blocked: string[];
	interReady: () => boolean;
	interAtFirstDraw?: boolean;
}

let stop: (() => void) | undefined;
afterEach(() => {
	stop?.();
	stop = undefined;
	document.body.replaceChildren();
});

// Kills a page that asks for Inter from a URL, a `data:` one included, which
// the default CSP blocks, and a plot that draws before Inter is ready, whose
// text is then laid out in the next font of the stack.
test("the built page draws in Inter under the default CSP, and not before Inter is ready", async () => {
	expect(built).not.toMatch(/@font-face/);
	const frame = document.createElement("iframe");
	frame.style.cssText = "width: 1000px; height: 900px; border: 0";
	frame.srcdoc = built.replace(
		"<head>",
		`<head><meta http-equiv="Content-Security-Policy" content="${DEFAULT_CSP}">${PROBE}`,
	);
	const shown = fakeHost({ first: success(perf()) }, (message) =>
		frame.contentWindow?.postMessage(message, "*"),
	);
	const listen = (event: MessageEvent) => {
		if (event.source === frame.contentWindow) {
			shown.answer(event.data);
		}
	};
	window.addEventListener("message", listen);
	stop = () => window.removeEventListener("message", listen);
	document.body.append(frame);

	const view = () => frame.contentWindow as Probed | null;
	await expect
		.poll(
			() =>
				view()?.document.querySelector("figure")?.getAttribute("aria-label"),
			{ timeout: 10_000 },
		)
		.toBe("2 lines");
	expect(view()?.interAtFirstDraw).toBe(true);
	expect(view()?.interReady()).toBe(true);
	expect(
		view()?.blocked.filter((entry) => entry.startsWith("font-src")),
	).toEqual([]);
});
