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

void game_v1()
{
    for (int i = 1; i <= 100; i++)
    {
        play_game(i, true);
    }
}

void play_game(int n, bool should_print)
{
    std::string result = fizz_buzz(n);
    if (should_print)
    {
        std::cout << result << std::endl;
    }
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

int main()
{
    game_v1();
    return 0;
}
