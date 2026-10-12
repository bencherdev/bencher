import type { JSX } from "solid-js";
import {
	children,
	createEffect,
	createUniqueId,
	on,
	onCleanup,
	onMount,
} from "solid-js";

export interface TooltipProps {
	/** What the control does, in a few words. */
	tip: JSX.Element;
	/** The one element it describes. */
	children: JSX.Element;
}

const GAP = 6;
const EDGE = 8;
/** How long a tooltip stays once the pointer leaves, so it can cross onto the tooltip. */
const GRACE_MS = 150;

/**
 * A few words beside the element it wraps, shown at once on hover and on
 * keyboard focus, never for a touch or on a phone. It stays while the element
 * is hovered or focused or the tooltip itself is hovered, and Escape hides it.
 * It describes the element and opens in the top layer, so no overflow clips it.
 */
const Tooltip = (props: TooltipProps) => {
	const id = createUniqueId();
	const anchor = children(() => props.children);
	let tip!: HTMLSpanElement;
	let hovered = false;
	let focused = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	const shown = () => tip.matches(":popover-open");
	const place = () => {
		// From the left edge, so a longer text measures at its own width.
		tip.style.left = "0";
		const at = (anchor() as HTMLElement).getBoundingClientRect();
		const { width, height } = tip.getBoundingClientRect();
		const room = document.documentElement.clientWidth - EDGE - width;
		const above = at.top - GAP - height;
		tip.style.left = `${Math.max(EDGE, Math.min(room, at.left + (at.width - width) / 2))}px`;
		tip.style.top = `${above < EDGE ? at.bottom + GAP : above}px`;
	};
	const onKey = (event: KeyboardEvent) => {
		if (event.key === "Escape") {
			hide();
		}
	};
	// While it shows, Escape hides it wherever the focus is, and a scroll or a resize places it again.
	const unlisten = () => {
		document.removeEventListener("keydown", onKey);
		removeEventListener("scroll", place, true);
		removeEventListener("resize", place);
	};
	const hide = () => {
		unlisten();
		tip.hidePopover();
	};
	const show = () => {
		clearTimeout(timer);
		if (shown() || !matchMedia("(min-width: 768px)").matches) {
			return;
		}
		tip.showPopover();
		place();
		document.addEventListener("keydown", onKey);
		addEventListener("scroll", place, { capture: true, passive: true });
		addEventListener("resize", place);
	};
	const leave = () => {
		if (!hovered && !focused) {
			timer = setTimeout(hide, GRACE_MS);
		}
	};
	createEffect(
		on(
			() => props.tip,
			() => shown() && place(),
			{ defer: true },
		),
	);
	onMount(() => {
		const target = anchor() as HTMLElement;
		const described = target.getAttribute("aria-describedby");
		target.setAttribute(
			"aria-describedby",
			described ? `${described} ${id}` : id,
		);
		onCleanup(() => {
			const rest = target
				.getAttribute("aria-describedby")
				?.split(" ")
				.filter((token) => token !== id)
				.join(" ");
			if (rest) {
				target.setAttribute("aria-describedby", rest);
			} else {
				target.removeAttribute("aria-describedby");
			}
		});
		target.addEventListener("pointerenter", (event) => {
			if (event.pointerType !== "touch") {
				hovered = true;
				show();
			}
		});
		tip.addEventListener("pointerenter", () => {
			hovered = true;
			clearTimeout(timer);
		});
		for (const element of [target, tip]) {
			element.addEventListener("pointerleave", () => {
				hovered = false;
				leave();
			});
		}
		target.addEventListener("focus", () => {
			if (target.matches(":focus-visible")) {
				focused = true;
				show();
			}
		});
		target.addEventListener("blur", () => {
			focused = false;
			leave();
		});
	});
	onCleanup(() => {
		clearTimeout(timer);
		unlisten();
	});
	return (
		<>
			{anchor()}
			<span
				ref={tip}
				id={id}
				role="tooltip"
				popover="manual"
				class="ui-tooltip"
			>
				{props.tip}
			</span>
		</>
	);
};

export default Tooltip;
