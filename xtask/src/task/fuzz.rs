// Create an LLM backed property based fuzzer for benchmarking
// This is separate and distinct from adding property based testing to Bencher itself (https://github.com/bencherdev/bencher/issues/50).
//
// Instead, this is a literal shower thought on how to create a blackbox based benchmarking tool for libraries, say Bencher Fuzz.
//
// Tools like Lighthouse allow one to benchmark the performance of any website.
// Likewise, Flashlight does the same for mobile apps.
//
// My (naive) understanding of how property based testing works is that you give it the input values and a function for deriving the solution and then it tests the production code output based on the input and derived output.
//
// Usually the goal in benchmarking and profiling is to both make the most common paths as fast as possible but to also avoid extreme tail latencies.
// In a strongly typed function definition, I could see this working in Rust and say Haskell as a property based benchmarking approach, which in and of itself would be pretty cool (does that even exist in the wild?).
// The tool would iterate through inputs and try to find the combination of inputs that creates the absolute maximum latencies/lowest throughput etc.
// A "benchmark fuzzer" or "benchmarking fuzzer" if you will.
//
// But taking things a step further, things break down as soon as inputs are stringly typed, even in Rust let alone moving to a less strongly or untyped language.
// This is there the idea for LLMs comes in. One of the top cited use cases for LLMs + code is test generation. But what about test/mock data generation?
// This could both be inferred by the code itself, the docs, or maybe just a short description of the input data types.
// Add that along with a separate but related goal to increase latency/decrease throughput as much as possible and iterate.
// In order to actually make this both time and cost effective, the LLM would need to be local and not API gated. The key here is both the number and speed of iterations.
//
// The original question I posed was, "How do you make security folks really care about performance?"
//
// And the answer that I came up with was this tool, an LLM backed benchmarking fuzzer used offensively.
// Given the corpus of the internet and its abundant documentation of CVEs, this tool may end up grabbing some of them or coming up with some of them on their own. Either way, showing that you can effectively DOS their tool will make the security folks in the room start paying attention. That plus the buzzy-ness around LLMs and you've got yourself fodder for a conference talk!
//
// I'm not really sure how effective this will be for getting library security folks to really care, as a lot of their use cases are embedded. But as far as getting attention towards the project and speaking at conferences, this seems like a good idea.
//
// Not to be confused with benchmarking a fuzzer: https://github.com/google/fuzzbench
