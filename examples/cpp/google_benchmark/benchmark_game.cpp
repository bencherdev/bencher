#include <benchmark/benchmark.h>
#include <iostream>

void FizzBuzz()
{
    for (int i = 1; i <= 100; i++)
    {
        if ((i % 15) == 0)
            std::cout << "FizzBuzz\n";
        else if ((i % 3) == 0)
            std::cout << "Fizz\n";
        else if ((i % 5) == 0)
            std::cout << "Buzz\n";
        else
            std::cout << i << "\n";
    }
}

static void BM_FizzBuzz(benchmark::State &state)
{
    for (auto _ : state)
    {
        FizzBuzz();
    }
}

// Register the function as a benchmark
BENCHMARK(BM_FizzBuzz);

// Run the benchmark
BENCHMARK_MAIN();