Profile guided optimization is on the to do list.
- https://doc.rust-lang.org/rustc/profile-guided-optimization.html
- https://blog.rust-lang.org/inside-rust/2020/11/11/exploring-pgo-for-the-rust-compiler.html
- https://github.com/Kobzol/cargo-pgo
- https://en.wikipedia.org/wiki/Profile-guided_optimization

A further extension of this would be also profile the application
with the same test load to analyze its call stack.

There are the "continuous profilers" like the Parca Agent and Graphana Pyroscope Agent:
- https://github.com/parca-dev/parca-agent
- https://github.com/grafana/pyroscope

This security tool is using a seemingly similar approach for helping to triage CVEs.

- https://www.oligo.security
- https://youtube.com/watch?v=m5Xwo715CgM
- https://youtube.com/watch?v=XAibAlw7POw&pp=ygUPb2xpZ28gc2VjdXJpdHkg
