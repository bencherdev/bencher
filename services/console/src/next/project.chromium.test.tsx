import { Show, createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import type { Api } from "./api";
import { leafOf } from "./leaf";
import { type Entity, ProjectContext, useEntityTitle } from "./project";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const Page = (props: { kind: string; name: () => string | undefined }) => {
	useEntityTitle(props.kind, props.name);
	return null;
};

// Kills a title that keeps a page's entity after it leaves, and a leaving page
// that clears the entity the next page already named.
test("a page names its entity while it shows, and the next page keeps its own", () => {
	const [entity, setEntity] = createSignal<Entity>();
	const [name, setName] = createSignal<string>();
	const [alert, setAlert] = createSignal(true);
	const [run, setRun] = createSignal(false);
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<ProjectContext.Provider
				value={{ api: {} as Api, slug: () => "hashbrown", entity: setEntity }}
			>
				<Show when={alert()}>
					<Page kind="Alert" name={name} />
				</Show>
				<Show when={run()}>
					<Page kind="Run" name={() => "main"} />
				</Show>
			</ProjectContext.Provider>
		),
		root,
	);
	expect(entity()).toBeUndefined();
	setName(leafOf("my_crate::module_000::bench_fn_name_0002"));
	expect(entity()).toEqual({ kind: "Alert", name: "bench_fn_name_0002" });
	setRun(true);
	setAlert(false);
	expect(entity()).toEqual({ kind: "Run", name: "main" });
	setRun(false);
	expect(entity()).toBeUndefined();
});

// Kills a name that ends in its separator read as empty.
test("a name with no last segment reads whole", () => {
	expect(leafOf("feature/")).toBe("feature/");
});
