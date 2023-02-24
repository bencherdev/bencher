# Example Python benchmarking

def fizz(n):
    if not n % 3:
        return 'Fizz'
    return None


def fizz_buzz(n):
    response = ''
    if not n % 3:
        response += 'Fizz'
    if not n % 5:
        response += 'Buzz'
    return response if response else None


def fizz_buzz_fibonacci(n):
    if not n % 7:
        return fibonacci(n)

    response = ''
    if not n % 3:
        response += 'Fizz'
    if not n % 5:
        response += 'Buzz'
    return response if response else None


def fibonacci(n):
    if n < 2:
        return n
    else:
        return fibonacci(n-1) + fibonacci(n-2)


def fibonacci_memo():
    pad = {0: 0, 1: 1}

    def func(n):
        if n not in pad:
            pad[n] = func(n-1) + func(n-2)
        return pad[n]
    return func


for n in range(0, 42):
    print(fizz_buzz_fibonacci(n))
