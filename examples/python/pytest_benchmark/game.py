def game_v0():
    """v0"""
    for i in range(1, 101):
        if i % 15 == 0:
            print("FizzBuzz")
        elif i % 3 == 0:
            print("Fizz")
        elif i % 5 == 0:
            print("Buzz")
        else:
            print(i)

def play_game_v1(n, should_print):
    """v1"""
    result = fizz_buzz_v1(n)
    if should_print:
        print(result)
    return result

def fizz_buzz_v1(n):
    if n % 15 == 0:
       return "FizzBuzz"
    elif n % 3 == 0:
        return "Fizz"
    elif n % 5 == 0:
        return "Buzz"
    else:
        return str(n)

def play_game_v2(n, should_print):
    """v2"""
    result = fizz_buzz_fibonacci_v2(n)
    if should_print:
        print(result)
    return result

def fizz_buzz_fibonacci_v2(n):
    if is_fibonacci_number_v2(n):
        return "Fibonacci"
    elif n % 15 == 0:
        return "FizzBuzz"
    elif n % 3 == 0:
        return "Fizz"
    elif n % 5 == 0:
        return "Buzz"
    else:
        return str(n)

def is_fibonacci_number_v2(n):
    for i in range(n + 1):
        previous, current = 0, 1
        while current < i:
            next_value = previous + current
            previous = current
            current = next_value
        if current == n:
            return True
    return False

def play_game(n, should_print):
    result = fizz_buzz_fibonacci(n)
    if should_print:
        print(result)
    return result

def fizz_buzz_fibonacci(n):
    if is_fibonacci_number(n):
        return "Fibonacci"
    else:
        if n % 15 == 0:
            return "FizzBuzz"
        elif n % 3 == 0:
            return "Fizz"
        elif n % 5 == 0:
            return "Buzz"
        else:
            return str(n)

def is_fibonacci_number(n):
    previous, current = 0, 1
    while current < n:
        next_value = previous + current
        previous = current
        current = next_value
    return current == n

def game_v1():
    for i in range(1, 101):
        play_game_v1(i, True)

def game_v2():
    for i in range(1, 101):
        play_game_v2(i, True)

def open_world_game():
    import sys
    args = sys.argv
    if len(args) > 1 and args[1].isdigit():
        i = int(args[1])
        play_game(i, True)
    else:
        print("Please, enter a positive integer to play...")

open_world_game()

def test_game(benchmark):
    def run_game():
        for i in range(1, 101):
            play_game(i, False)
    benchmark(run_game)

def test_game_100(benchmark):
    def run_game():
        play_game(100, False)
    benchmark(run_game)

def test_game_1_000_000(benchmark):
    def run_game():
        play_game(1_000_000, False)
    benchmark(run_game)
