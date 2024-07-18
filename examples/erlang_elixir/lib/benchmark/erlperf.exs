[
  ~c"fib:recursive(20).",
  ~c"fib:optimized(20)."
]
|> Enum.concat(['-d', '100', '-r', 'full'])
|> :erlperf_cli.main()
