import fbf


def test_fizz_buzz_fibonacci(benchmark):
    result = benchmark(fbf.fizz_buzz_fibonacci)
    assert result == 123
