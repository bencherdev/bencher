const licensingPage = {
	title: "License Agreement - Bencher",
	heading: "Bencher License Agreement",
	content: (
		<div>
			<p>Copyright © 2022-{new Date().getFullYear()} Pompeii LLC</p>
			<p>
				All content that resides under any directory named "plus" is licensed
				under the <a href="/legal/plus">Bencher Plus License</a>.
			</p>
			<p>
				All other content is licensed under either of{" "}
				<a href="https://opensource.org/licenses/Apache-2.0">
					Apache License, Version 2.0
				</a>{" "}
				or <a href="https://opensource.org/licenses/MIT">MIT license</a> at your
				discretion. Unless you explicitly state otherwise, any contribution
				intentionally submitted for inclusion in Bencher by you, as defined in
				the Apache-2.0 license, shall be dual licensed as above, without any
				additional terms or conditions.
			</p>
		</div>
	),
};

export default licensingPage;
