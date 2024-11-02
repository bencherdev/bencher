#include <iostream>
#include <string>

int game_v0()
{
    for (int i = 1; i <= 100; i++)
    {
        if ((i % 15) == 0)
            std::cout << "FizzBuzz\n";
        else if ((i % 3) == 0)
            std::cout << "Fizz\n";
        else if ((i % 5) == 0)
            std::cout << "Buzz\n";
        else
            std::cout << i << "\n";
    }
    return 0;
}

std::string fizz_buzz(int n)
{
    if (n % 15 == 0)
    {
        return "FizzBuzz";
    }
    else if (n % 3 == 0)
    {
        return "Fizz";
    }
    else if (n % 5 == 0)
    {
        return "Buzz";
    }
    else
    {
        return std::to_string(n);
    }
}

void play_game_v1(int n, bool should_print)
{
    std::string result = fizz_buzz(n);
    if (should_print)
    {
        std::cout << result << std::endl;
    }
}

void game_v1()
{
    for (int i = 1; i <= 100; i++)
    {
        play_game_v1(i, true);
    }
}

bool is_fibonacci_number_v2(int n)
{
    for (int i = 0; i <= n; ++i)
    {
        int previous = 0, current = 1;
        while (current < i)
        {
            int next = previous + current;
            previous = current;
            current = next;
        }
        if (current == n)
        {
            return true;
        }
    }
    return false;
}

std::string fizz_buzz_fibonacci_v2(int n)
{
    if (is_fibonacci_number_v2(n))
    {
        return "Fibonacci";
    }
    else if (n % 15 == 0)
    {
        return "FizzBuzz";
    }
    else if (n % 3 == 0)
    {
        return "Fizz";
    }
    else if (n % 5 == 0)
    {
        return "Buzz";
    }
    else
    {
        return std::to_string(n);
    }
}

void play_game(int n, bool should_print)
{
    std::string result = fizz_buzz_fibonacci_v2(n);
    if (should_print)
    {
        std::cout << result << std::endl;
    }
}