def fizz(n):
    """v1"""

    if not n % 3:
        return 'Fizz'
    return None


def fizz_buzz(n):
    """v2"""

    response = ''
    if not n % 3:
        response += 'Fizz'
    if not n % 5:
        response += 'Buzz'
    return response if response else None


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

    def memo(n):
        if n not in pad:
            pad[n] = memo(n-1) + memo(n-2)
        return pad[n]
    return memo(n)


def test_fibonacci(benchmark):
    def fibonacci_month():
        for n in range(7, 29, 7):
            fibonacci_memo(n)
    benchmark(fibonacci_month)

def test_fun_notification(benchmark):
    def days_in_month():
        for n in range(1, 32):
            fizz_buzz_fibonacci(n)
    benchmark(days_in_month)