import * as Sentry from "@sentry/astro";
import { debounce } from "@solid-primitives/scheduled";
import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import { Portal } from "solid-js/web";
import {
	type Language,
	defaultLang,
	search as searchLabel,
	searchDevStub,
	searchEmpty,
	searchError,
	searchLoading,
	searchNoResults,
	searchPlaceholder,
} from "../../../i18n/ui";
import type { PagefindApi, PagefindResultData } from "./pagefind";

export interface Props {
	lang?: undefined | Language;
}

const PAGEFIND_URL = "/pagefind/pagefind.js";
const DEBOUNCE_MS = 150;
const MAX_RESULTS = 10;

const DocsSearch = (props: Props) => {
	const isDev = import.meta.env.DEV;
	const lang = (): Language => props.lang ?? defaultLang;

	const [isOpen, setIsOpen] = createSignal(false);
	const [query, setQuery] = createSignal("");
	const [results, setResults] = createSignal<PagefindResultData[]>([]);
	const [loading, setLoading] = createSignal(false);
	const [errored, setErrored] = createSignal(false);
	const [selected, setSelected] = createSignal(0);
	const [pagefind, setPagefind] = createSignal<PagefindApi | null>(null);
	let inputRef: HTMLInputElement | undefined;
	let triggerRef: HTMLButtonElement | undefined;

	const loadPagefind = async (): Promise<PagefindApi | null> => {
		const cached = pagefind();
		if (cached) {
			return cached;
		}
		if (isDev) {
			return null;
		}
		try {
			// Assign to a variable to prevent Vite from trying to resolve the
			// runtime-served /pagefind/pagefind.js at build time.
			const url = PAGEFIND_URL;
			const mod = (await import(/* @vite-ignore */ url)) as PagefindApi;
			setPagefind(mod);
			return mod;
		} catch (err) {
			Sentry.captureException(err);
			setErrored(true);
			return null;
		}
	};

	const open = () => {
		setIsOpen(true);
		setErrored(false);
		document.documentElement.classList.add("is-clipped");
		void loadPagefind();
		requestAnimationFrame(() => inputRef?.focus());
	};

	const close = () => {
		setIsOpen(false);
		setQuery("");
		setResults([]);
		setSelected(0);
		document.documentElement.classList.remove("is-clipped");
		requestAnimationFrame(() => triggerRef?.focus());
	};

	const runSearch = async (q: string) => {
		const trimmed = q.trim();
		if (!trimmed) {
			setResults([]);
			setLoading(false);
			return;
		}
		const pf = pagefind();
		if (!pf) {
			setLoading(false);
			return;
		}
		try {
			const response = await pf.search(trimmed, {
				filters: { lang: [lang()] },
			});
			const data = await Promise.all(
				response.results.slice(0, MAX_RESULTS).map((r) => r.data()),
			);
			setResults(data);
			setSelected(0);
		} catch (err) {
			Sentry.captureException(err);
			setErrored(true);
		} finally {
			setLoading(false);
		}
	};

	const debouncedSearch = debounce(runSearch, DEBOUNCE_MS);
	onCleanup(() => debouncedSearch.clear());

	const onInput = (event: InputEvent & { currentTarget: HTMLInputElement }) => {
		const value = event.currentTarget.value;
		setQuery(value);
		setErrored(false);
		if (!value.trim()) {
			setResults([]);
			setLoading(false);
			return;
		}
		setLoading(true);
		debouncedSearch(value);
	};

	const onGlobalKey = (event: KeyboardEvent) => {
		if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
			event.preventDefault();
			if (isOpen()) {
				close();
			} else {
				open();
			}
		}
	};

	const onModalKey = (event: KeyboardEvent) => {
		if (event.key === "Escape") {
			event.preventDefault();
			close();
			return;
		}
		if (event.key === "ArrowDown") {
			event.preventDefault();
			const len = results().length;
			if (len > 0) {
				setSelected((s) => (s + 1) % len);
			}
			return;
		}
		if (event.key === "ArrowUp") {
			event.preventDefault();
			const len = results().length;
			if (len > 0) {
				setSelected((s) => (s - 1 + len) % len);
			}
			return;
		}
		if (event.key === "Enter") {
			const picked = results()[selected()];
			if (picked) {
				event.preventDefault();
				window.location.href = picked.url;
			}
		}
	};

	onMount(() => {
		window.addEventListener("keydown", onGlobalKey);
		onCleanup(() => window.removeEventListener("keydown", onGlobalKey));
	});

	return (
		<>
			<div class="menu-label" style="margin-bottom: 0.5rem;">
				<button
					ref={triggerRef}
					type="button"
					class="button is-small is-fullwidth"
					onClick={open}
					title={searchLabel(lang())}
					aria-label={searchLabel(lang())}
				>
					<span class="icon is-small">
						<i class="fas fa-search" />
					</span>
				</button>
			</div>
			<Portal>
			<Show when={isOpen()}>
				<div class="modal is-active" onKeyDown={onModalKey}>
					<div
						class="modal-background"
						onMouseDown={(event) => {
							event.preventDefault();
							close();
						}}
					/>
					<div class="modal-card">
						<header class="modal-card-head">
							<p class="modal-card-title">{searchLabel(lang())}</p>
							<button
								class="delete"
								type="button"
								aria-label="close"
								onMouseDown={(event) => {
									event.preventDefault();
									close();
								}}
							/>
						</header>
						<section class="modal-card-body">
							<div class="field">
								<div class="control has-icons-left">
									<input
										ref={inputRef}
										class="input"
										type="search"
										autofocus
										placeholder={searchPlaceholder(lang())}
										value={query()}
										onInput={onInput}
									/>
									<span class="icon is-small is-left">
										<i class="fas fa-search" />
									</span>
								</div>
							</div>
							<Show when={isDev}>
								<div class="notification is-warning is-light mt-4">
									{searchDevStub(lang())}
								</div>
							</Show>
							<Show when={!isDev && errored()}>
								<p class="has-text-danger mt-4">{searchError(lang())}</p>
							</Show>
							<Show when={!isDev && !errored() && !query().trim()}>
								<p class="has-text-grey mt-4">{searchEmpty(lang())}</p>
							</Show>
							<Show
								when={
									!isDev && !errored() && query().trim() !== "" && loading()
								}
							>
								<p class="has-text-grey mt-4">{searchLoading(lang())}</p>
							</Show>
							<Show
								when={
									!isDev &&
									!errored() &&
									query().trim() !== "" &&
									!loading() &&
									results().length === 0
								}
							>
								<p class="has-text-grey mt-4">
									{searchNoResults(lang(), query().trim())}
								</p>
							</Show>
							<Show when={!isDev && results().length > 0}>
								<ul class="menu-list mt-4 docs-search-results">
									<For each={results()}>
										{(result, index) => (
											<li>
												<a
													href={result.url}
													class={selected() === index() ? "is-active" : ""}
													onMouseEnter={() => setSelected(index())}
												>
													<strong>{result.meta?.title ?? result.url}</strong>
													<p class="is-size-7" innerHTML={result.excerpt} />
												</a>
											</li>
										)}
									</For>
								</ul>
							</Show>
						</section>
					</div>
				</div>
			</Show>
			</Portal>
		</>
	);
};

export default DocsSearch;
