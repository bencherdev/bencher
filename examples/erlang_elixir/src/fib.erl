-module(fib).
-export([recursive/1, optimized/1]).

%% Recursive version
recursive(0) -> 0;
recursive(1) -> 1;
recursive(N) -> recursive(N - 1) + recursive(N - 2).

%% Tail call optimized version
optimized(N) -> optimized(N, 0, 1).

optimized(1, _A, B) -> B;
optimized(N, A, B) -> optimized(N - 1, B, A + B).
