/** Where a scrolled tab row should sit so its current tab is centered. */
export const centeredScroll = ({
	scrollWidth,
	clientWidth,
	offsetLeft,
	offsetWidth,
}: {
	scrollWidth: number;
	clientWidth: number;
	offsetLeft: number;
	offsetWidth: number;
}) =>
	scrollWidth > clientWidth
		? Math.max(0, offsetLeft - (clientWidth - offsetWidth) / 2)
		: 0;

export const centerCurrentTab = (tabs: HTMLElement | undefined) => {
	const current = tabs?.querySelector<HTMLElement>('[aria-current="page"]');
	if (tabs && current) {
		const row = tabs.getBoundingClientRect();
		const tab = current.getBoundingClientRect();
		tabs.scrollLeft = centeredScroll({
			scrollWidth: tabs.scrollWidth,
			clientWidth: tabs.clientWidth,
			offsetLeft: tab.left - row.left + tabs.scrollLeft,
			offsetWidth: tab.width,
		});
	}
};
