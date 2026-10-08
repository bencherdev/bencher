import Menu from "@bencherdev/ui/Menu";
import TextInput from "@bencherdev/ui/TextInput";
import {
	For,
	Index,
	type JSX,
	Show,
	createEffect,
	createSignal,
	on,
	onCleanup,
	onMount,
} from "solid-js";

export interface BoxValue {
	label: string;
	/** Quiet text after the label, such as a measure's units. */
	detail?: string | undefined;
}

export interface Option {
	value: string;
	label: string;
	detail?: string | undefined;
}

const SEARCH_DELAY = 150;

export const Cross = () => (
	<svg
		viewBox="0 0 24 24"
		width="12"
		height="12"
		fill="none"
		stroke="currentColor"
		stroke-width="2.4"
		stroke-linecap="round"
		aria-hidden="true"
	>
		<path d="M6 6l12 12M18 6L6 18" />
	</svg>
);

/** One box of the query: its values, each with a remove control, and an add control that searches. */
const Box = (props: {
	title: string;
	/** The value's name in a control's label, such as `branch`. */
	noun: string;
	values: BoxValue[];
	hint?: string | undefined;
	/** Marks the box as the place to start. */
	start?: boolean | undefined;
	readOnly?: boolean | undefined;
	/** The box holds as many values as the query reads. */
	full?: boolean | undefined;
	onRemove: (index: number) => void;
	/** The add control's choices, read once it opens; `text` is what the reader searched. */
	options: (text: () => string) => () => Option[];
	onAdd: (option: Option) => void;
	/** The add control opened. */
	onOpen?: (() => void) | undefined;
	/** What the menu says when it has nothing to offer. */
	empty?: string | undefined;
	/** Rows of the children's own, each with a remove control. */
	rows?: number | undefined;
	children?: JSX.Element;
}) => {
	const [open, setOpen] = createSignal(false);
	let box: HTMLFieldSetElement | undefined;
	let add: HTMLButtonElement | undefined;
	// A removed row takes its focused control with it: the next row's, or Add, takes focus back.
	let refocus: number | undefined;
	const removes = () => [
		...(box?.querySelectorAll<HTMLButtonElement>(".ex-x") ?? []),
	];
	onMount(() =>
		box?.addEventListener(
			"click",
			(event) => {
				const remove =
					event.target instanceof Element
						? event.target.closest<HTMLButtonElement>(".ex-x")
						: null;
				refocus =
					remove && remove === document.activeElement
						? removes().indexOf(remove)
						: undefined;
			},
			true,
		),
	);
	createEffect(
		on(
			() => props.values.length + (props.rows ?? 0),
			() => {
				const at = refocus;
				refocus = undefined;
				if (at !== undefined) {
					(removes()[at] ?? add)?.focus();
				}
			},
			{ defer: true },
		),
	);
	return (
		<fieldset
			ref={box}
			class="ex-box"
			classList={{ "ex-start": props.start === true }}
			aria-label={props.title}
		>
			<div class="ex-eh">
				<span>{props.title}</span>
				<Show when={props.hint}>
					<span class="ex-hint" classList={{ "ex-on": props.start === true }}>
						{props.hint}
					</span>
				</Show>
			</div>
			<Index each={props.values}>
				{(value, index) => (
					<div class="ex-val">
						<span class="ex-ellip">{value().label}</span>
						<Show when={value().detail}>
							<span class="ex-k">{value().detail}</span>
						</Show>
						<Show when={!props.readOnly}>
							<button
								type="button"
								class="ex-x"
								aria-label={`Remove ${props.noun} ${value().label}`}
								onClick={() => props.onRemove(index)}
							>
								<Cross />
							</button>
						</Show>
					</div>
				)}
			</Index>
			{props.children}
			<Show when={!(props.readOnly || props.full)}>
				<span class="ex-anchor">
					<button
						ref={add}
						type="button"
						class="ex-add"
						classList={{ "ex-hot": props.start === true || open() }}
						aria-haspopup="menu"
						aria-expanded={open()}
						onClick={() => {
							if (!open()) {
								props.onOpen?.();
							}
							setOpen(!open());
						}}
					>
						<svg
							viewBox="0 0 24 24"
							width="12"
							height="12"
							fill="none"
							stroke="currentColor"
							stroke-width="2.4"
							stroke-linecap="round"
							aria-hidden="true"
						>
							<path d="M12 5v14M5 12h14" />
						</svg>
						Add {props.noun}
					</button>
					<Show when={open()}>
						<AddMenu
							noun={props.noun}
							title={props.title}
							anchor={add}
							options={props.options}
							empty={props.empty}
							onClose={() => setOpen(false)}
							onPick={(option) => {
								setOpen(false);
								props.onAdd(option);
							}}
						/>
					</Show>
				</span>
			</Show>
		</fieldset>
	);
};

export default Box;

const AddMenu = (props: {
	noun: string;
	title: string;
	anchor: HTMLElement | undefined;
	options: (text: () => string) => () => Option[];
	empty?: string | undefined;
	onClose: () => void;
	onPick: (option: Option) => void;
}) => {
	const [typed, setTyped] = createSignal("");
	const [text, setText] = createSignal("");
	let timer: ReturnType<typeof setTimeout> | undefined;
	onCleanup(() => clearTimeout(timer));
	const options = props.options(text);
	const search = `Search ${props.title.toLowerCase()}`;
	return (
		<Menu
			label={`Add ${props.noun}`}
			anchor={props.anchor}
			onClose={props.onClose}
			header={
				<TextInput
					type="search"
					size="sm"
					aria-label={search}
					placeholder={search}
					value={typed()}
					onInput={(event) => {
						const value = event.currentTarget.value;
						setTyped(value);
						clearTimeout(timer);
						timer = setTimeout(() => setText(value.trim()), SEARCH_DELAY);
					}}
				/>
			}
		>
			<For
				each={options()}
				fallback={
					<p class="ex-none">
						{text() ? `Nothing matches "${text()}".` : (props.empty ?? "")}
					</p>
				}
			>
				{(option) => (
					<button
						type="button"
						role="menuitem"
						tabindex={-1}
						class="ui-menu-item"
						onClick={() => props.onPick(option)}
					>
						<span class="ex-ellip">{option.label}</span>
						<Show when={option.detail}>
							<span class="ex-n">{option.detail}</span>
						</Show>
					</button>
				)}
			</For>
		</Menu>
	);
};
