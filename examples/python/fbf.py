def fizz_buzz_fibonacci(n):
    response = ''
    if not n % 3:
        response += 'Fizz'
    if not n % 5:
        response += 'Buzz'
    if not n % 7:
        return fibonacci(n)
    return response


def fibonacci(n):
    if n < 2:
        return n
    fib_prev = 1
    fib = 1
    for _ in range(2, n):
        fib_prev, fib = fib, fib + fib_prev
    return fib


for n in range(0, 100):
    print(fizz_buzz_fibonacci(n))
