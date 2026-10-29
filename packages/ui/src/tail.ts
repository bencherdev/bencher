// Apart from Truncate, so a name can be split without loading the component.

/** Splits once after the last `::`, else after the last `/`; with neither, it is all tail. */
export const tailOf = (text: string): [head: string, tail: string] => {
	const at = text.lastIndexOf("::");
	const cut = at < 0 ? text.lastIndexOf("/") + 1 : at + 2;
	return [text.slice(0, cut), text.slice(cut)];
};
