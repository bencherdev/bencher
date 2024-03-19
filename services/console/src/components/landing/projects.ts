interface Project {
	name: string;
	about: string;
	logo: string;
	slug: string;
	query: string;
}

const caseStudy = (project: string, slug: string) => {
	const notifyKind = "alert";
	const notifyText = `Learn more about continuous benchmarking for the ${project} project.`;
	const notifyTimeout = 2147483647;
	const notifyLinkUrl = `https://bencher.dev/learn/case-study/${slug}/`;
	const notifyLinkText = "Read the case study";
	return `notify_kind=${notifyKind}&notify_text=${encodeURIComponent(
		notifyText,
	)}&notify_timeout=${notifyTimeout}&notify_link_url=${encodeURIComponent(
		notifyLinkUrl,
	)}&notify_link_text=${encodeURIComponent(notifyLinkText)}`;
};

export const PROJECTS: Project[][] = [
	[
		{
			name: "Rustls",
			about: "A modern TLS library written in Rust",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/rustls.png",
			slug: "rustls-821705769",
			query: `key=true&reports_per_page=8&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&clear=true&tab=branches&measures=013468de-9c37-4605-b363-aebbbf63268d&branches=28fae530-2b53-4482-acd4-47e16030d54f&testbeds=62ed31c3-8a58-479c-b828-52521ed67bee&benchmarks=bd25f73c-b2b9-4188-91b4-f632287c0a1b%2C8d443816-7a23-40a1-a54c-59de911eb517%2C42edb37f-ca91-4984-8835-445514575c85&start_time=1704067200000&${caseStudy(
				"Rustls",
				"rustls",
			)}`,
		},
		{
			name: "Diesel",
			about: "A safe, extensible ORM and Query Builder for Rust",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/diesel.svg",
			slug: "diesel",
			query:
				"key=true&reports_per_page=8&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&clear=true&tab=benchmarks&measures=2d3bd4cd-c4d4-4aa1-9e60-47e51e2b9dde&branches=bf9a5209-6524-45e3-af26-b8f98eee3bad&testbeds=4e5c3c90-920c-4741-8cf7-aaed4e16e9a5&benchmarks=5dfa78a5-7785-4d33-a336-aab5fff43372%2Cf65ec533-abf5-443e-a0d8-e4a583c5779e%2C0c1bcad9-2100-4170-9bc7-96a3b89071b9%2Ccee41d01-30db-4acc-8727-0d0b4ccbe216%2C6d23685f-e082-4913-8c22-14311030d130",
		},
		{
			name: "Poolifier",
			about: "Fast and small Node.js worker_threads and cluster worker pool",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/poolifier.png",
			slug: "poolifier",
			query:
				"key=true&reports_per_page=8&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&clear=true&tab=branches&branches=977f91aa-2157-4e5b-a4dc-e1d8c3ece8af&testbeds=12203dc4-c6e4-439b-bb2b-a5d4e227e4f5&measures=73517df3-f327-4853-9546-a8b61381b5e2&benchmarks=2515bbd1-81c8-4ab2-8746-135c6fa638b6%2Cf96b89da-378e-42a4-bc16-2034c1e16b3a%2Cdc1c353d-1da9-4940-af1f-d0cbdef98b03%2Cbe79f393-70f3-4a94-b377-f7b80e345461&start_time=1704067200000&benchmarks_search=FixedClusterPool+with+FAIR_SHARE",
		},
	],
	[
		{
			name: "Hydra Database",
			about:
				"Column-oriented Postgres. Add scalable analytics to your project in minutes.",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/hydra-db.svg",
			slug: "hydra-postgres",
			query:
				"key=true&reports_per_page=8&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=6&clear=true&tab=branches&measures=c20a9c30-e20a-45b7-bba5-4a6e940f951f&branches=e6bcbe0c-210d-4ab1-8fe4-5d9498800980&testbeds=1d3283b3-3e52-4dd0-a018-fb90c9361a2e&benchmarks=b31c3185-9701-4576-9fd7-288aea5cc7e4%2Cc4efd5bb-f4c4-4b75-9137-f2a841c04cfe%2C6e050650-ad8a-4043-b62c-a39e0e202bfe%2Cec575db9-3c10-4122-af8f-a062be36a198",
		},
		{
			name: "GreptimeDB",
			about:
				"An open-source, cloud-native, distributed time-series database with PromQL/SQL/Python supported.",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/greptimedb.svg",
			slug: "greptimedb",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&report=8dcbda4a-c239-4a9e-8399-4fc699f775b4&branches=3b46708f-b87f-4f52-b1bb-1d9cc7bfee2d&testbeds=6d3be02f-9efe-4e47-8a5d-e389c228172d&benchmarks=da5c8cbe-9aef-431e-9168-11ef0821c8db%2Cbb7ce469-5c34-4a69-ab2f-d9769ca5be2a&measures=a2f1689d-44d5-4d5e-863f-47d285cedf97&start_time=1707524593000&end_time=1710116593000&clear=true",
		},
		{
			name: "Disney+ Hotstar",
			about:
				"India's largest premium streaming platform with more than 100000 hours of content in 17 languages.",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/disney-hotstar.png",
			slug: "hotstar",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&report=dbab4284-e7fb-48a6-8c6b-051a98957079&branches=d6949a6e-ce6b-457b-bb69-9ab4d62eae71&testbeds=45bf0733-efb6-4d8a-8abc-a98099f3f4f2&benchmarks=a95083a3-b57c-429c-ab57-6569b27530e1%2C4f02a4e8-0158-49b9-bf78-b5977c2ce9fb%2C29a32a7a-244e-4d13-ad87-1a4b81e77428%2C2c760c7d-b860-47d0-9b9e-f60ab6204705%2C7b643e87-25c3-442a-8060-8aa1625661fc&measures=5ed5ccea-b820-42df-8974-704d3284a5c9&start_time=1698066511000&end_time=1700658511000&clear=true&range=version",
		},
	],
	[
		{
			name: "K Framework",
			about:
				"A framework for defining programming languages and their semantics",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/k-framework.png",
			slug: "k-framework",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&report=503f3fbc-3266-4389-b87e-8c6a7f7f6240&branches=f7830a8b-198d-4ac5-b5f2-23b8026b0a4f&testbeds=d9eea46c-dd6c-4d0e-a830-30581a4e4446&benchmarks=29feeefd-7ac2-4aca-9b7b-ac95826f2a41&measures=8ad04853-f0fd-410e-b075-104ae5162c82&start_time=1707828269000&end_time=1710420305000&clear=true&tab=benchmarks",
		},
		{
			name: "raft",
			about: "Asynchronous C implementation of the Raft consensus protocol",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/raft.png",
			slug: "raft",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&report=96e8a26e-1084-4de2-8ee1-d1da172141ce&branches=eeae6157-d12a-4c11-b900-9b30a9a7d497&testbeds=64e77684-ceb1-4eba-8e0b-f81f0864a608&benchmarks=58c2071b-67bc-421b-83f2-d896eb52b712%2Ce35dfc29-bfef-432d-b167-060dbbfc42ab&measures=76ecf81a-972d-4e1a-8e80-a052f2cbaf74&start_time=1707921099000&end_time=1710513101000&clear=true&tab=branches",
		},
		{
			name: "Stratum",
			about:
				"A next-generation bitcoin mining protocol designed to enhance the efficiency, security, flexibility and decentralization.",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/stratum.svg",
			slug: "stratum-v2-sri",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=3&branches_page=1&testbeds_page=1&benchmarks_page=1&branches=9c79faed-80db-4d0e-8100-b7dba4091898&testbeds=6e5299a9-5f8e-48db-b3e9-d0be3f592002&benchmarks=942edfe5-f73e-4744-978b-9e017ec1d25d%2Cc9249d55-f75f-41ec-891e-01c95941073e%2C9452e053-59a2-4f0d-b6d5-5301c9940445&measures=c73d96c0-22eb-4fb3-a2c9-bb9daf4b00e7&start_time=1707839782000&end_time=1710431782000&clear=true&tab=benchmarks",
		},
	],
];
