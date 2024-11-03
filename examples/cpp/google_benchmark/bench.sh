g++ -std=c++11 -isystem benchmark/include -Lbenchmark/build/src -lbenchmark -lpthread play_game.cpp benchmark_game.cpp -o benchmark_game
./benchmark_game --benchmark_format=json
