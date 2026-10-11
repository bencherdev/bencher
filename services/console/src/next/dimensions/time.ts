const DAY: Intl.DateTimeFormatOptions = { month: "short", day: "numeric" };

/** The day, with the year when it is not this year's. */
export const shortDay = (time: number | string, now = Date.now()) => {
	const date = new Date(time);
	const thisYear = date.getFullYear() === new Date(now).getFullYear();
	return new Intl.DateTimeFormat(
		"en-US",
		thisYear ? DAY : { ...DAY, year: "numeric" },
	).format(date);
};

/** The day and the 24 hour time, with the year when it is not this year's. */
export const shortDate = (time: number | string, now = Date.now()) => {
	const date = new Date(time);
	const clock = new Intl.DateTimeFormat("en-US", {
		hour: "2-digit",
		minute: "2-digit",
		hourCycle: "h23",
	}).format(date);
	return `${shortDay(date.getTime(), now)}, ${clock}`;
};
