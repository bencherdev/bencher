/** What in a row keeps a click for itself: its controls, and a tooltip drawn inside it. */
const OWN = "a, button, input, label, select, textarea, [role=tooltip]";

/** Whether a click is the row's own: off the row's controls, and not the end of a drag that selected text. */
export const rowClick = (event: MouseEvent) =>
	!(event.target instanceof Element && event.target.closest(OWN)) &&
	!getSelection()?.toString();
