import { For, createMemo, createSignal } from "solid-js";
import { StatisticKind } from "../../../types/bencher";
import {
	validBoundary,
	validCdfBoundary,
	validIqrBoundary,
	validPercentageBoundary,
	validSampleSize,
	validU32,
} from "../../../util/valid";
import type { FieldConfig, FieldHandler, FieldValue } from "../Field";
import FieldKind from "../kind";
import { createStore } from "solid-js/store";
import Field from "../Field";

export type InputValue = string | number | null | undefined;

export interface Props {
	value: FieldValue;
	valid: undefined | null | boolean;
	config: FieldConfig;
	handleField: FieldHandler;
}

export interface StatisticConfig {
	icon: string;
	type: string;
	placeholder?: string;
	value: InputValue;
	disabled?: boolean;
	help?: string;
	validate: (value: InputValue) => boolean;
}

const STATISTIC_FIELDS = {
	test: {
		icon: "fas fa-vial",
	},
	static_lower_boundary: {
		type: "input",
		placeholder: "50",
		icon: "fas fa-arrow-down",
		help: "Must be any finite floating point number",
		validate: validBoundary,
	},
	static_upper_boundary: {
		type: "input",
		placeholder: "100",
		icon: "fas fa-arrow-up",
		help: "Must be any finite floating point number",
		validate: validBoundary,
	},
	percentage_lower_boundary: {
		type: "input",
		placeholder: "0.50",
		icon: "fas fa-arrow-down",
		help: "Must be any percentage greater than or equal to zero in decimal form",
		validate: validPercentageBoundary,
	},
	percentage_upper_boundary: {
		type: "input",
		placeholder: "0.50",
		icon: "fas fa-arrow-up",
		help: "Must be any percentage greater than or equal to zero in decimal form",
		validate: validPercentageBoundary,
	},
	cdf_lower_boundary: {
		type: "input",
		placeholder: "0.98",
		icon: "fas fa-arrow-down",
		help: "Must be between 0.50 and 1.00 (lower is stricter; higher is looser)",
		validate: validCdfBoundary,
	},
	cdf_upper_boundary: {
		type: "input",
		placeholder: "0.98",
		icon: "fas fa-arrow-up",
		help: "Must be between 0.50 and 1.00 (lower is stricter; higher is looser)",
		validate: validCdfBoundary,
	},
	iqr_lower_boundary: {
		type: "input",
		placeholder: "3.0",
		icon: "fas fa-arrow-down",
		help: "Must be any multiplier greater than or equal to zero",
		validate: validIqrBoundary,
	},
	iqr_upper_boundary: {
		type: "input",
		placeholder: "3.0",
		icon: "fas fa-arrow-up",
		help: "Must be any multiplier greater than or equal to zero",
		validate: validIqrBoundary,
	},
	min_sample_size: {
		type: "number",
		placeholder: "30",
		icon: "fas fa-cube",
		help: "Must be an integer greater than or equal to 2",
		validate: validSampleSize,
	},
	max_sample_size: {
		type: "number",
		placeholder: "30",
		icon: "fas fa-cubes",
		help: "Must be an integer greater than or equal to 2",
		validate: validSampleSize,
	},
	window: {
		type: "number",
		placeholder: "525600",
		icon: "fas fa-calendar-week",
		help: "Must be an integer greater than zero",
		validate: validU32,
	},
};

const testValue = (selected: StatisticKind) => {
	return {
		selected,
		options: [
			{
				value: StatisticKind.Static,
				option: "Static",
			},
			{
				value: StatisticKind.Percentage,
				option: "Percentage",
			},
			{
				value: StatisticKind.ZScore,
				option: "z-score",
			},
			{
				value: StatisticKind.TTest,
				option: "t-test",
			},
			{
				value: StatisticKind.LogNormal,
				option: "Log Normal",
			},
			{
				value: StatisticKind.Iqr,
				option: "Interquartile Range (IQR)",
			},
			{
				value: StatisticKind.DeltaIqr,
				option: "Delta Interquartile Range (ΔIQR)",
			},
		],
	};
};

const testSelectConfig = (statistic: StatisticKind) => {
	return {
		kind: FieldKind.SELECT,
		label: (
			<div class="level is-mobile">
				<div class="level-left">
					<p class="level-item">Significance Test</p>
					<a
						class="level-item"
						href={`https://bencher.dev/docs/explanation/thresholds/#${testFragment(
							statistic,
						)}`}
						// biome-ignore lint/a11y/noBlankTarget: <explanation>
						target="_blank"
						title="Open documentation in new tab"
					>
						<span class="icon">
							<i class="fas fa-book-open" aria-hidden="true" />
						</span>
					</a>
				</div>
			</div>
		),
		key: "test",
		value: testValue(statistic),
		validate: false,
		config: STATISTIC_FIELDS.test,
	};
};

const testFragment = (statistic: StatisticKind) => {
	switch (statistic) {
		case StatisticKind.Static:
			return "static-thresholds";
		case StatisticKind.Percentage:
			return "percentage-thresholds";
		case StatisticKind.ZScore:
			return "z-score-thresholds";
		case StatisticKind.TTest:
			return "t-test-thresholds";
		case StatisticKind.LogNormal:
			return "log-normal-thresholds";
		case StatisticKind.Iqr:
			return "iqr-thresholds";
		case StatisticKind.DeltaIqr:
			return "delta-iqr-thresholds";
	}
};

const cdfConfig = (statistic: StatisticKind) => {
	return [
		testSelectConfig(statistic),
		{
			kind: FieldKind.NUMBER,
			label: "Lower Boundary",
			key: "lower_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.cdf_lower_boundary,
		},
		{
			kind: FieldKind.NUMBER,
			label: "Upper Boundary",
			key: "upper_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.cdf_upper_boundary,
		},
		...SAMPLE_SIZE,
	];
};

const iqrConfig = (statistic: StatisticKind) => {
	return [
		testSelectConfig(statistic),
		{
			kind: FieldKind.NUMBER,
			label: "Lower Boundary",
			key: "lower_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.iqr_lower_boundary,
		},
		{
			kind: FieldKind.NUMBER,
			label: "Upper Boundary",
			key: "upper_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.iqr_upper_boundary,
		},
		...SAMPLE_SIZE,
	];
};

const SAMPLE_SIZE = [
	{
		kind: FieldKind.NUMBER,
		label: "Minimum Sample Size",
		key: "min_sample_size",
		value: "",
		valid: true,
		validate: true,
		nullable: true,
		config: STATISTIC_FIELDS.min_sample_size,
	},
	{
		kind: FieldKind.NUMBER,
		label: "Maximum Sample Size",
		key: "max_sample_size",
		value: "",
		valid: true,
		validate: true,
		nullable: true,
		config: STATISTIC_FIELDS.max_sample_size,
	},
	{
		kind: FieldKind.NUMBER,
		label: "Window Size (seconds)",
		key: "window",
		value: "",
		valid: true,
		validate: true,
		nullable: true,
		config: STATISTIC_FIELDS.window,
	},
];

const FIELDS = {
	[StatisticKind.Static]: [
		testSelectConfig(StatisticKind.Static),
		{
			kind: FieldKind.NUMBER,
			label: "Lower Boundary",
			key: "lower_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.static_lower_boundary,
		},
		{
			kind: FieldKind.NUMBER,
			label: "Upper Boundary",
			key: "upper_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.static_upper_boundary,
		},
	],
	[StatisticKind.Percentage]: [
		testSelectConfig(StatisticKind.Percentage),
		{
			kind: FieldKind.NUMBER,
			label: "Lower Boundary",
			key: "lower_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.percentage_lower_boundary,
		},
		{
			kind: FieldKind.NUMBER,
			label: "Upper Boundary",
			key: "upper_boundary",
			value: "",
			valid: true,
			validate: true,
			nullable: true,
			config: STATISTIC_FIELDS.percentage_upper_boundary,
		},
		...SAMPLE_SIZE,
	],
	[StatisticKind.ZScore]: cdfConfig(StatisticKind.ZScore),
	[StatisticKind.TTest]: cdfConfig(StatisticKind.TTest),
	[StatisticKind.LogNormal]: cdfConfig(StatisticKind.LogNormal),
	[StatisticKind.Iqr]: iqrConfig(StatisticKind.Iqr),
	[StatisticKind.DeltaIqr]: iqrConfig(StatisticKind.DeltaIqr),
};

const initForm = (fields) => {
	let newForm = {};
	fields.forEach((field) => {
		if (field.key) {
			newForm[field.key] = {
				kind: field.kind,
				label: field.label,
				value: field.value,
				valid: field.valid,
				validate: field.validate,
				nullable: field.nullable,
			};
		}
	});
	return newForm;
};

const Statistic = (props: Props) => {
	const [test, setTest] = createSignal(StatisticKind.TTest);
	const fields = createMemo(() => FIELDS[test()]);

	const [form, setForm] = createStore(initForm(fields()));

	const serializeForm = () => {
		const data: Record<string, undefined | number | string> = {};
		for (const key of Object.keys(form)) {
			const value = form?.[key]?.value;
			switch (form?.[key]?.kind) {
				case FieldKind.SELECT:
					if (form?.[key]?.nullable && !value?.selected) {
						continue;
					}
					data[key] = value?.selected;
					break;
				case FieldKind.NUMBER:
					if (form?.[key]?.nullable && !value) {
						continue;
					}
					data[key] = Number(value);
					break;
				default:
					if (form?.[key]?.nullable && !value) {
						continue;
					}
					if (typeof value === "string") {
						data[key] = value.trim();
					} else {
						data[key] = value;
					}
			}
		}
		return data;
	};

	const handleField = (key: string, value: FieldValue, valid: boolean) => {
		if (key && form?.[key]) {
			if (key === "test") {
				setTest(value?.selected);
				setForm(initForm(FIELDS[value?.selected]));
				props.handleField(serializeForm());
				return;
			}

			if (form?.[key]?.nullable && !value) {
				value = null;
				valid = true;
			}

			setForm({
				...form,
				[key]: {
					...form?.[key],
					value,
					valid,
				},
			});

			props.handleField(serializeForm());
		}
	};

	return (
		<>
			<For each={fields()}>
				{(field, _i) => (
					<Field
						kind={field?.kind}
						label={form?.[field?.key]?.label}
						fieldKey={field?.key}
						value={form?.[field?.key]?.value}
						valid={form?.[field?.key]?.valid}
						config={field?.config}
						handleField={handleField}
					/>
				)}
			</For>
		</>
	);
};

export default Statistic;
