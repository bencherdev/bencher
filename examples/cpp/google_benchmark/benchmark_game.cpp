#include "play_game.h"
#include <benchmark/benchmark.h>
#include <iostream>

static void BENCHMARK_play_game(benchmark::State &state)
{
    for (auto _ : state)
    {
        for (int i = 1; i <= 100; i++)
        {
            play_game(i, true);
        }
    }
}

// Register the function as a benchmark
BENCHMARK(BENCHMARK_play_game);

// Run the benchmark
BENCHMARK_MAIN();
