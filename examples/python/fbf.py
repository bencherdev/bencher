def fizz_buzz_fibonacci(n):
    response = ''

    if not n%3:
        response += 'Fizz'
    if not n%5:
        response += 'Buzz'

   return response

for n in range(0,max):
    print fizz_buzz_fibonacci(n)