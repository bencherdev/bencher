import { For } from "solid-js";

interface Project {
	name: string;
	logo: string;
	slug: string;
}

const PROJECTS: Project[][] = [
	[
		{
			name: "Rustls",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/rustls.png",
			slug: "rustls-821705769",
		},
		{
			name: "K Framework",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/k-framework.png",
			slug: "k-framework",
		},
		{
			name: "Poolifier",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/poolifier.png",
			slug: "poolifier",
		},
	],
	[
		{
			name: "Hydra Database",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/hydra-db.svg",
			slug: "hydra-postgres",
		},
		{
			name: "GreptimeDB",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/greptimedb.svg",
			slug: "greptimedb",
		},
		{
			name: "Disney+ Hotstar",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/disney-hotstar.png",
			slug: "hotstar",
		},
	],
	[
		{
			name: "trace4rs",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/trace4rs.png",
			slug: "trace4rs",
		},
		{
			name: "Stratum",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/stratum.svg",
			slug: "stratum",
		},
		{
			name: "raft",
			logo: "https://s3.amazonaws.com/public.bencher.dev/case-study/raft.png",
			slug: "raft",
		},
	],
];

const Showcase = (_props: {}) => {
	return (
		<section class="section" style="margin-top: 4rem;">
			<div class="content has-text-centered">
				<h2 class="title is-2">You are in good company</h2>
				<br />
				<br />
				<div class="columns is-centered is-vcentered">
					<div class="column">
						<For each={PROJECTS}>
							{([left_project, center_project, right_project]) => (
								<div class="columns is-centered is-vcentered is-mobile">
									<ProjectLogo project={left_project as Project} />
									<br />
									<ProjectLogo project={center_project as Project} />
									<br />
									<ProjectLogo project={right_project as Project} />
								</div>
							)}
						</For>
					</div>
				</div>
			</div>
		</section>
	);
};

const ProjectLogo = (props: { project: Project }) => {
	return (
		<div class="column has-text-centered is-2">
			<a href={`https://bencher.dev/perf/${props.project.slug}`}>
				<img width="88%" src={props.project.logo} alt={props.project.name} />
			</a>
		</div>
	);
};

export default Showcase;
