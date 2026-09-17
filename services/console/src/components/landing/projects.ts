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
			name: "Microsoft snmalloc",
			about: "Message passing based allocator",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/microsoft.png",
			slug: "snmalloc",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=dc041689-5eaf-4b70-b1c8-9da23594c030&heads=3caf3a2d-f6aa-4e58-b7de-92219e9b4616&testbeds=c7457113-7c66-42a7-ac72-ee4e74d9f300&benchmarks=e6fdc724-f1e4-477e-81eb-19185eaef02d%2Cf99f94fb-805f-4338-a5ce-13d261f91afc%2C24eb178e-d9d4-45d7-90ec-1c82dfb58ea2&measures=44c8d1f2-5a3e-4573-bc7d-79f80461103c&start_time=1781395200000&end_time=1789430400000&clear=true&tab=benchmarks&x_axis=version",
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
			name: "conda",
			about: "A system-level, binary package and environment manager",
			logo: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/conda.svg",
			slug: "conda-tdj8rt90",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=67634f05-6858-4e0b-8c77-9222afaa78a3&heads=3075025a-cf38-4fbc-851c-fcbf223d35ee&testbeds=d32fdfd6-9631-4a16-b71c-da1e838eaf0f&benchmarks=fbbe288f-2b29-4395-977a-1254429d3ca5%2C08b608e7-8ebb-41ea-a579-9d812b05135c%2C0bad22f1-5340-45fd-9229-7f6cd114c48d&measures=d40e34c0-83a2-4f3e-926c-3f5870705694&start_time=1781395200000&end_time=1789430400000&upper_boundary=true&clear=true&tab=benchmarks",
		},
		{
			name: "Firezone",
			about: "Blazing-fast remote access",
			logo: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/firezone.svg",
			slug: "firezone-1l75jv1z",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=455d0566-7315-404c-93e1-877e2732cc97&heads=455d0566-7315-404c-93e1-877e2732cc97&testbeds=258ac4fc-9cc9-4281-a45b-666cd8264371&benchmarks=f0008569-a7be-4f17-8f53-37abe7cbc31b%2Cb35d8c57-795d-4eac-9105-6911d81320a7%2Cdbebadcd-b0cf-4abd-a4f8-7274f064f6da&measures=a777fd3c-5f6c-4884-82e8-39cd8b2d30f1&start_time=1781395200000&end_time=1789430400000&clear=true&tab=benchmarks",
		},
		{
			name: "pnpm",
			about: "Fast, disk space efficient package manager",
			logo: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/pnpm.svg",
			slug: "pnpm",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=b06c9054-8025-4f4d-973a-36e92a38ba01&heads=751ebac5-6f89-4ed7-889d-74b2b27c8bc8&testbeds=e76d4447-6105-4421-bd5f-092ab7013510&benchmarks=d9e5dbb5-fb8b-42b4-b5bd-6c3afc491616%2Ccbb1df50-ee25-4018-9aa3-9dd41b4d125f%2C94a7b145-0268-4341-904c-6caa15e85686&measures=d54343e2-b40b-4dec-a120-2fc6cb336362&start_time=1781395200000&end_time=1789430400000&clear=true&tab=benchmarks",
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
			name: "clap",
			about: "A full featured, fast Command Line Argument Parser for Rust",
			logo: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/case-study/clap.png",
			slug: "clap-rs-clap",
			query:
				"key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&plots_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1&plots_page=1&branches=b920383c-b9ee-4bd6-94ea-8d101b55286a&heads=5eeccfee-4fdd-405a-8554-90cd945ee1c1&testbeds=551ebdbf-b50a-4813-9064-286d2e66888f&benchmarks=b0a8ca01-4418-485e-9446-81d2a9c62774&measures=04ff075b-dc09-4c77-909a-634352fd5b02&end_time=1767052800000&lower_boundary=false&upper_boundary=false&clear=true&start_time=1748908800000&tab=branches&branches_search=master",
		},
	],
];
