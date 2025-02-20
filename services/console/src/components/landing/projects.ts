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
			name: "Microsoft CCF",
			about:
				"A framework for building a new category of secure, highly available, and performant applications",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/microsoft.png",
			slug: "ccf",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=d5004f0a-5dbe-42bb-a821-1f55704d6ec2&testbeds=1e6f6a27-eb58-4f16-8d01-0148fbaed70e&benchmarks=3bae8305-29e0-4e5f-8157-01f8f471b408&measures=bc9fb376-9a85-478a-97fd-ebd7703c9663&start_time=1715185355000&end_time=1717777355000&clear=true&tab=benchmarks",
		},
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
			query: `key=true&reports_per_page=8&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&clear=true&tab=benchmarks&measures=2d3bd4cd-c4d4-4aa1-9e60-47e51e2b9dde&branches=bf9a5209-6524-45e3-af26-b8f98eee3bad&testbeds=4e5c3c90-920c-4741-8cf7-aaed4e16e9a5&benchmarks=5dfa78a5-7785-4d33-a336-aab5fff43372%2Cf65ec533-abf5-443e-a0d8-e4a583c5779e%2C0c1bcad9-2100-4170-9bc7-96a3b89071b9%2Ccee41d01-30db-4acc-8727-0d0b4ccbe216%2C6d23685f-e082-4913-8c22-14311030d130&${caseStudy(
				"Diesel",
				"diesel",
			)}`,
		},
	],
	[
		{
			name: "Hydra Database",
			about:
				"Column-oriented Postgres. Add scalable analytics to your project in minutes",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/hydra-db.svg",
			slug: "hydra-postgres",
			query:
				"key=true&reports_per_page=8&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=6&clear=true&tab=branches&measures=c20a9c30-e20a-45b7-bba5-4a6e940f951f&branches=e6bcbe0c-210d-4ab1-8fe4-5d9498800980&testbeds=1d3283b3-3e52-4dd0-a018-fb90c9361a2e&benchmarks=b31c3185-9701-4576-9fd7-288aea5cc7e4%2Cc4efd5bb-f4c4-4b75-9137-f2a841c04cfe%2C6e050650-ad8a-4043-b62c-a39e0e202bfe%2Cec575db9-3c10-4122-af8f-a062be36a198",
		},
		{
			name: "GreptimeDB",
			about:
				"An open-source, cloud-native, distributed time-series database with PromQL/SQL/Python supported",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/greptimedb.svg",
			slug: "greptimedb",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&report=8dcbda4a-c239-4a9e-8399-4fc699f775b4&branches=3b46708f-b87f-4f52-b1bb-1d9cc7bfee2d&testbeds=6d3be02f-9efe-4e47-8a5d-e389c228172d&benchmarks=da5c8cbe-9aef-431e-9168-11ef0821c8db%2Cbb7ce469-5c34-4a69-ab2f-d9769ca5be2a&measures=a2f1689d-44d5-4d5e-863f-47d285cedf97&start_time=1707524593000&end_time=1710116593000&clear=true",
		},
		{
			name: "Tailcall",
			about: "A high-performance GraphQL Platform",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/tailcall.svg",
			slug: "tailcall",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&branches=3646cfed-fd77-417e-b8d5-90eab450e855&testbeds=5823e8f8-162f-4a86-862d-3ed9b3415a75&benchmarks=5022fcf2-e392-4dc6-8b62-cb2da9a6e36a%2Cd1499469-f2dc-4b38-91ba-83ecf11ce678%2C851fc472-d9d7-42b8-ba91-b0f90e3c9909%2Cdbea7f22-5076-4a91-a83e-bb2cadddb069&measures=d6846b7a-7a7a-4e2e-91a1-131232a131e3&start_time=1710981217000&end_time=1713573818000&clear=true&upper_boundary=false&range=version&tab=branches",
		},
	],
	[
		{
			name: "Servo",
			about:
				"The embeddable, independent, memory-safe, modular, parallel web rendering engine",
			logo: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/servo.svg",
			slug: "servo",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=52e1e9bb-959c-4171-a53d-e06bd694a6c1&heads=3dbe3681-11b1-4e30-b482-4ee72dc0960c&testbeds=d742c702-3842-4108-9d0c-2db74e57599a&measures=678e4118-c8a5-494d-8799-08abc3021cd5&start_time=1734048000000&end_time=1735236203000&lower_boundary=false&upper_boundary=false&clear=true&tab=benchmarks&benchmarks=c4da10d8-9539-4943-95ca-5e08df0cd6f9&benchmarks_search=servo",
		},
		{
			name: "Ratatui",
			about: "A Rust library for cooking up Terminal User Interfaces (TUIs)",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/ratatui.png",
			slug: "ratatui-org",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=95ce51f3-9a78-41e8-8700-562f11680798&testbeds=0615b230-cbf8-4ea6-8e2e-616c282b102a&measures=b917dd68-60ef-41c6-8ce9-2164eba4f46b&start_time=1720841447000&end_time=1723434422000&clear=true&tab=benchmarks&branches_search=main&benchmarks_search=barchart%2F&benchmarks=5695514c-6501-44a4-9a43-9de69078be9c%2C7bada371-e16a-475b-9424-af842fd2dd70%2Cadb521a6-df19-4ee9-af93-e783b69a4dc0&upper_boundary=false&lower_boundary=false",
		},
		{
			name: "Wire",
			about:
				"The most secure platform for messaging, audio, and video calls, based on edge computing and zero knowledge architecture",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/wire.svg",
			slug: "core-crypto-mmbtki3h",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=cd6b82fc-bbfb-4680-afa6-ab88ca62a1ef&testbeds=7f837718-cf29-423f-bd13-2b516ec88cda&measures=c1f87d1c-d949-4bf4-8b76-eb782e882d0e&start_time=1719668529000&end_time=1722261285000&clear=true&tab=benchmarks&benchmarks_search=6010&benchmarks=a4cefec8-6548-4e20-a7c1-75456b7ea925%2C0c73af64-460b-4082-a73b-77e3a980606d",
		},
	],
];
