import type { JSX } from "solid-js";
import { onCleanup, onMount, splitProps } from "solid-js";
import Icon from "./Icon";

export interface MenuProps extends JSX.HTMLAttributes<HTMLDivElement> {
	/** The menu's accessible name. */
	label: string;
	/** The control that opened the menu: focus returns to it, and a press on it is not outside. */
	anchor: HTMLElement | undefined;
	onClose: () => void;
	/** Above the items and outside the menu role, such as a search box. */
	header?: JSX.Element;
	/** `end` lines the menu up with its anchor's right edge. */
	align?: "start" | "end";
}

const ITEMS = '[role^="menuitem"]:not([disabled])';

/**
 * A menu anchored under the control that opened it; place both in an element
 * with `position: relative`. It takes the focus when it opens (the search box,
 * else the checked item, else the first) and scrolls itself into view, the
 * arrow keys, Home, and End move through its items, and Escape, Tab, or a press
 * outside close it. In a Sheet it opens in the flow, below its control.
 */
const Menu = (props: MenuProps) => {
	const [local, rest] = splitProps(props, [
		"label",
		"anchor",
		"onClose",
		"header",
		"align",
		"class",
		"children",
	]);
	let panel: HTMLDivElement | undefined;
	let list: HTMLDivElement | undefined;
	const items = () => [...(list?.querySelectorAll<HTMLElement>(ITEMS) ?? [])];

	const onKeyDown = (event: KeyboardEvent) => {
		const all = items();
		const at = all.indexOf(document.activeElement as HTMLElement);
		const focus = (index: number) => {
			event.preventDefault();
			all[(index + all.length) % all.length]?.focus();
		};
		switch (event.key) {
			case "Escape":
				event.preventDefault();
				local.onClose();
				return;
			case "Tab":
				local.onClose();
				return;
			case "ArrowDown":
				return focus(at + 1);
			case "ArrowUp":
				return focus(at < 0 ? -1 : at - 1);
			case "Home":
				return at < 0 ? undefined : focus(0);
			case "End":
				return at < 0 ? undefined : focus(-1);
		}
	};

	onMount(() => {
		// Delegated, so the search box in `header` steps into the items too.
		panel?.addEventListener("keydown", onKeyDown);
		const first =
			panel?.querySelector<HTMLElement>("input") ??
			list?.querySelector<HTMLElement>('[aria-checked="true"]') ??
			items()[0];
		first?.focus();
		panel?.scrollIntoView({ block: "nearest" });
		const outside = (event: PointerEvent) => {
			const target = event.target as Node;
			if (!(panel?.contains(target) || local.anchor?.contains(target))) {
				local.onClose();
			}
		};
		document.addEventListener("pointerdown", outside);
		onCleanup(() => document.removeEventListener("pointerdown", outside));
	});
	// Closed by a choice or a key, the focus would otherwise fall to the page.
	onCleanup(() => {
		if (panel?.contains(document.activeElement)) {
			local.anchor?.focus();
		}
	});

	return (
		<div
			ref={panel}
			class={local.class === undefined ? "ui-menu" : `ui-menu ${local.class}`}
			data-align={local.align ?? "start"}
			{...rest}
		>
			{local.header}
			<div ref={list} role="menu" aria-label={local.label} class="ui-menu-list">
				{local.children}
			</div>
		</div>
	);
};

export default Menu;

export interface MenuItemRadioProps
	extends Omit<JSX.ButtonHTMLAttributes<HTMLButtonElement>, "onSelect"> {
	checked: boolean;
	onSelect: () => void;
}

/** One choice in a Menu, marked with a check when it is the current one. */
export const MenuItemRadio = (props: MenuItemRadioProps) => {
	const [local, rest] = splitProps(props, [
		"checked",
		"onSelect",
		"class",
		"children",
	]);
	return (
		<button
			type="button"
			role="menuitemradio"
			aria-checked={local.checked}
			tabindex={-1}
			class={
				local.class === undefined
					? "ui-menu-item"
					: `ui-menu-item ${local.class}`
			}
			onClick={() => local.onSelect()}
			{...rest}
		>
			<Icon name="check" class="ui-menu-check" />
			{local.children}
		</button>
	);
};
