interface Customer {
	name: string;
	icon: string;
	github: string;
	quote: string;
}

export const CUSTOMERS: Customer[][] = [
	[
		{
			name: "Jonathan Woollett-Light",
			icon: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/customers/JonathanWoollett-Light.jpg",
			github: "JonathanWoollett-Light",
			quote: "Bencher is like CodeCov for performance metrics.",
		},
		{
			name: "Price Clark",
			icon: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/customers/gpwclark.jpg",
			github: "gpwclark",
			quote:
				"I think I'm in heaven. Now that I'm starting to see graphs of performance over time automatically from tests I'm running in CI. It's like this whole branch of errors can be caught and noticed sooner.",
		},
	],
	[
		{
			name: "Joe Neeman",
			icon: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/customers/jneem.jpg",
			github: "jneem",
			quote:
				"95% of the time I don't want to think about my benchmarks. But when I need to, Bencher ensures that I have the detailed historical record waiting there for me. It's fire-and-forget.",
		},
		{
			name: "Jamie Wilkinson",
			icon: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/customers/jaqx0r.png",
			github: "jaqx0r",
			quote:
				"I've been looking for a public service like Bencher for about 10 years :)",
		},
	],
	[
		{
			name: "Weston Pace",
			icon: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/customers/westonpace.jpg",
			github: "westonpace",
			quote:
				"I'm happy with how quickly I was able to get Bencher configured and working.",
		},
		{
			name: "Free Ekanayaka",
			icon: "https://s3.us-east-1.amazonaws.com/public.bencher.dev/customers/freeekanayaka.jpg",
			github: "freeekanayaka",
			quote: "Bencher's main ideas and concepts are really well designed.",
		},
	],
];
