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
			name: "Google Sedpack",
			about: "Scalable and efficient data packing for ML models",
			logo: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/google.svg",
			slug: "sedpack",
			query:
				"branches=e27f4617-5c19-4a91-a3f5-ca006bde2dd8&heads=e0f3701a-7886-4317-bf5c-ff04e2d0ccd1&testbeds=c83cc96a-a3b8-4c8e-88d3-d86c49caa12e&benchmarks=2fed029b-b64d-40ac-9d37-e4582ac6ad6b%2C7c8dfdfe-cc70-4928-8d09-841d7864984b&measures=37d645e6-8e9a-4731-8f16-28f12c22bd1c&upper_boundary=true&end_time=1754265600000&key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=3&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&start_time=1741996800000&lower_boundary=false&upper_value=false&lower_value=false&tab=branches&clear=true&branches_search=main",
		},
		{
			name: "GitLab Git",
			about: "Git is a fast, scalable, distributed revision control system",
			logo: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/gitlab.svg",
			slug: "git",
			query:
				"lower_value=false&upper_value=false&lower_boundary=false&upper_boundary=true&x_axis=date_time&branches=595859eb-071c-48e9-97cf-195e0a3d6ed1&testbeds=02dcb8ad-6873-494c-aabc-9a6237601308&benchmarks=5e5c6ae1-ec8e-4c25-b27d-dcf773d33a51%2C0eb509fd-c4a8-45f3-baca-2e7e4a89b0e8&measures=63dafffb-98c4-4c27-ba43-7112cae627fc&tab=plots&plots_search=0d7f6186-f80a-4fbe-9022-75b6caf5164e&key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&end_time=1745971200000&start_time=1740787200000&utm_medium=share&utm_source=bencher&utm_content=img&utm_campaign=perf%2Bimg&utm_term=git",
		},
	],
	[
		{
			name: "Servo",
			about:
				"The embeddable, independent, memory-safe, modular, parallel web rendering engine",
			logo: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/servo-tlf.svg",
			slug: "servo",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=52e1e9bb-959c-4171-a53d-e06bd694a6c1&heads=3dbe3681-11b1-4e30-b482-4ee72dc0960c&testbeds=d742c702-3842-4108-9d0c-2db74e57599a&measures=678e4118-c8a5-494d-8799-08abc3021cd5&start_time=1734048000000&end_time=1735236203000&lower_boundary=false&upper_boundary=false&clear=true&tab=benchmarks&benchmarks=c4da10d8-9539-4943-95ca-5e08df0cd6f9&benchmarks_search=servo",
		},
		{
			name: "Mozilla Neqo",
			about: "The Mozilla Firefox implementation of QUIC in Rust",
			logo: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/mozilla.svg",
			slug: "neqo",
			query:
				"branches=1c3aa454-5e63-4a34-bc7e-a86c397661fe&heads=a5e4e812-c619-44d3-844e-ee795a2b26e9&testbeds=f8b47e59-8dac-4a95-aec4-5bfb9756e749&measures=8bfeb966-6e8a-4719-9705-23fe985d6e40&upper_boundary=true&start_time=1762992000000&end_time=1765152000000&key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&tab=benchmarks&benchmarks_search=decode+1048576+bytes&benchmarks=66a0e29f-9d91-4656-903e-d4c0c817387c%2C9c88c263-c57e-45b8-89c9-34e3a5f196cb%2C0258133c-8223-4b76-a76c-9e92a1a60f60&clear=true",
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
	],
	[
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
		{
			name: "clap",
			about: "A full featured, fast Command Line Argument Parser for Rust",
			logo: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/clap.png",
			slug: "clap-rs-clap",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=b920383c-b9ee-4bd6-94ea-8d101b55286a&heads=5eeccfee-4fdd-405a-8554-90cd945ee1c1&testbeds=551ebdbf-b50a-4813-9064-286d2e66888f&benchmarks=b0a8ca01-4418-485e-9446-81d2a9c62774&measures=04ff075b-dc09-4c77-909a-634352fd5b02&end_time=1767052800000&lower_boundary=false&upper_boundary=false&clear=true&start_time=1748908800000&tab=branches&branches_search=master",
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
	],
];
