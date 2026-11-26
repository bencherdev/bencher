import { type ParameterSet, parameterEntries } from "./row";

type Row = { text: string; flag: boolean };

/** The export line and the command it runs, each command row but the last continued. */
export const commandOf = (rows: readonly Row[]) => {
	const code = rows.map(({ text, flag }, index) => ({
		text: index === 0 || index === rows.length - 1 ? text : `${text} \\`,
		flag,
	}));
	return { text: code.map(({ text }) => text).join("\n"), code };
};

// Spaced for reading, with keys in the API's order.
export const json = (parameters: ParameterSet) =>
	`{${parameterEntries(parameters)
		.map(([key, value]) => `${JSON.stringify(key)}: ${JSON.stringify(value)}`)
		.join(", ")}}`;

const SHELL_SAFE = /^[A-Za-z0-9_@%+=:,./-]+$/;

export const quote = (value: string) =>
	SHELL_SAFE.test(value) ? value : `'${value.replaceAll("'", `'\\''`)}'`;
