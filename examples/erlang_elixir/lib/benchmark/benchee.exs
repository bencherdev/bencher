Benchee.run(
  %{
    "recursive(20)" => fn -> :fib.recursive(20) end,
    "optimized(20)" => fn -> :fib.optimized(20) end
  },
  memory_time: 10
)
