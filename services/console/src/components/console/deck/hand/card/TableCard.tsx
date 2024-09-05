import type { Params } from "astro";
import type { JsonReport } from "../../../../../types/bencher";
import { createMemo, type Resource } from "solid-js";

export interface Props {
	isConsole?: boolean;
	apiUrl: string;
	params: Params;
	value: Resource<JsonReport>;
}

const TableCard = (props: Props) => {
	const benchmarkUrls = createMemo(() =>
		BenchmarkUrls.new(props.isConsole, props.value() as JsonReport),
	);

	return (
		<div class="columns is-centered" style="margin-top: 2rem">
			<div class="column is-two-thirds">
				<div class="table-container">
					<table class="table is-bordered is-fullwidth">
						<thead>
							<tr>
								<th>Benchmark</th>
								<th>Latency</th>
								<th>
									Latency Results
									<br />
									nanoseconds (ns) | (Δ%)
								</th>
								<th>
									Latency Upper Boundary
									<br />
									nanoseconds (ns) | (%)
								</th>
							</tr>
						</thead>
						<tbody>
							<tr>
								<td>Adapter::Json</td>
								<td>
									🚨 (
									<a href="https://bencher.dev/perf/bencher?branches=bdcbbf3c-9073-4006-b194-b11aff2f94c1&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=e93b3d71-8499-4fae-bb7c-4e540b775714&measures=4358146b-b647-4869-9d24-bd22bb0c49b5&start_time=1699143413000&end_time=1701735487000&upper_boundary=true">
										view plot
									</a>{" "}
									|{" "}
									<a href="https://bencher.dev/perf/bencher/alerts/91ee27a7-2aee-41fe-b037-80b786f26cd5">
										view alert
									</a>
									)
								</td>
								<td>3445.600 (+1.52%)</td>
								<td>3362.079 (102.48%)</td>
							</tr>
							<tr>
								<td>Adapter::Magic (JSON)</td>
								<td>
									✅ (
									<a href="https://bencher.dev/perf/bencher?branches=bdcbbf3c-9073-4006-b194-b11aff2f94c1&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=3bfd5887-83ec-4e62-8690-02855a38fbc9&measures=4358146b-b647-4869-9d24-bd22bb0c49b5&start_time=1699143413000&end_time=1701735487000&upper_boundary=true">
										view plot
									</a>
									)
								</td>
								<td>3431.400 (+0.69%)</td>
								<td>3596.950 (95.40%)</td>
							</tr>
							<tr>
								<td>Adapter::Magic (Rust)</td>
								<td>
									✅ (
									<a href="https://bencher.dev/perf/bencher?branches=bdcbbf3c-9073-4006-b194-b11aff2f94c1&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=3525f177-fc8f-4a92-bd2f-dda7c4e15699&measures=4358146b-b647-4869-9d24-bd22bb0c49b5&start_time=1699143413000&end_time=1701735487000&upper_boundary=true">
										view plot
									</a>
									)
								</td>
								<td>22095.000 (-0.83%)</td>
								<td>24732.801 (89.33%)</td>
							</tr>
							<tr>
								<td>Adapter::Rust</td>
								<td>
									✅ (
									<a href="https://bencher.dev/perf/bencher?branches=bdcbbf3c-9073-4006-b194-b11aff2f94c1&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=5655ed2a-3e45-4622-bdbd-39cdd9837af8&measures=4358146b-b647-4869-9d24-bd22bb0c49b5&start_time=1699143413000&end_time=1701735487000&upper_boundary=true">
										view plot
									</a>
									)
								</td>
								<td>2305.700 (-2.76%)</td>
								<td>2500.499 (92.21%)</td>
							</tr>
							<tr>
								<td>Adapter::RustBench</td>
								<td>
									✅ (
									<a href="https://bencher.dev/perf/bencher?branches=bdcbbf3c-9073-4006-b194-b11aff2f94c1&testbeds=0d991aac-b241-493a-8b0f-8d41419455d2&benchmarks=1db23e93-f909-40aa-bf42-838cc7ae05f5&measures=4358146b-b647-4869-9d24-bd22bb0c49b5&start_time=1699143413000&end_time=1701735487000&upper_boundary=true">
										view plot
									</a>
									)
								</td>
								<td>2299.900 (-3.11%)</td>
								<td>2503.419 (91.87%)</td>
							</tr>
						</tbody>
					</table>
				</div>
			</div>
		</div>
	);
};

interface Benchmark {
	name: string;
	slug: string;
}

interface Measure {
	name: string;
	slug: string;
	units: string;
}

interface MeasureData {
	url: string;
	value: number;
	boundary?: Boundary;
}

interface Boundary {
	baseline?: number;
	lower_limit?: number;
	upper_limit?: number;
}

class BenchmarkUrls {
	urls: Map<Benchmark, Map<Measure, MeasureData>>;

	constructor(urls: Map<Benchmark, Map<Measure, MeasureData>>) {
		this.urls = urls;
	}

	static new(
		isConsole: undefined | boolean,
		jsonReport: JsonReport,
	): BenchmarkUrls {
		const urls = new Map<Benchmark, Map<Measure, MeasureData>>();
		const iteration = jsonReport?.results?.[0];
		if (!iteration) {
			return new BenchmarkUrls(urls);
		}

		if (iteration) {
			for (const result of iteration) {
				const measure = {
					name: result.measure.name,
					slug: result.measure.slug,
					units: result.measure.units,
				};

				for (const benchmarkMetric of result.benchmarks) {
					const benchmark: Benchmark = {
						name: benchmarkMetric.name,
						slug: benchmarkMetric.slug,
					};

					if (!urls.has(benchmark)) {
						urls.set(benchmark, new Map<Measure, MeasureData>());
					}

					// biome-ignore lint/style/noNonNullAssertion: just checked for existence
					const benchmarkUrls = urls.get(benchmark)!;
					const boundary = benchmarkMetric.boundary;

					const data: MeasureData = {
						url: isConsole ? "/console" : "/perf",
						value: benchmarkMetric.metric.value,
						boundary,
					};

					benchmarkUrls.set(measure, data);
				}
			}
		}

		return new BenchmarkUrls(urls);
	}
}

export default TableCard;
