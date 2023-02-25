def fizz(n):
    """v1"""

    if not n % 3:
        return 'Fizz'
    return None


def run_v1():
    """v1"""

    return run(fizz)


def fizz_buzz(n):
    """v2"""

    response = ''
    if not n % 3:
        response += 'Fizz'
    if not n % 5:
        response += 'Buzz'
    return response if response else None


def run_v2():
    """v2"""

    return run(fizz_buzz)


def fizz_buzz_fibonacci(n):
    """v3"""

    if not n % 7:
        return fibonacci(n)

    response = ''
    if not n % 3:
        response += 'Fizz'
    if not n % 5:
        response += 'Buzz'
    return response if response else None


def fibonacci(n):
    """v3"""

    if n < 2:
        return n
    else:
        return fibonacci(n-1) + fibonacci(n-2)


def run_v3():
    """v3"""

    return run(fizz_buzz_fibonacci)


def fizz_buzz_fibonacci_memo(n):
    """v4"""

    if not n % 7:
        return fibonacci_memo(n)

    response = ''
    if not n % 3:
        response += 'Fizz'
    if not n % 5:
        response += 'Buzz'
    return response if response else None


def fibonacci_memo(n):
    """v4"""

    pad = {0: 0, 1: 1}

    def func(n):
        if n not in pad:
            pad[n] = func(n-1) + func(n-2)
        return pad[n]
    return func(n)


def run_v4():
    """v4"""

    return run(fizz_buzz_fibonacci_memo)


def run():
    result = ''
    for n in range(0, 31):
        result += f'{fizz_buzz_fibonacci(n)}'
    return result


print(run_v4())
