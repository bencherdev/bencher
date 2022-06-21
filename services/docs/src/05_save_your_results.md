# Save Your Benchmark Results

Now that you have the `bencher` CLI running your benchmarks and a Bencher REST API token,
you can finally save your benchmark results!

## Run a Benchmark with Credentials

The only difference between running a benchmark locally and saving the results is swapping out the `--local` flag with the `--email` and the `--token` flags. For example:

```
bencher run --email info@bencher.dev --token 123JWT --adapter rust_cargo_bench "cargo bench"
```

These new flags are:
- `--email info@bencher.dev` - The email used to create your Bencher account. This should be the same email as your GitHub account.
- `--token 123JWT` - The API token that you copied earlier. Again, it can be found at [https://bencher.dev/account/token](https://bencher.dev/account/token).

## View Your Results

After you run your benchmarks with credentials, you can now got to your [Bencher Dashboard](https://bencher.dev/dashboard) and view your results. You may notice that the project name for your results is `default`. Whenever a project name or ID is not specified, benchmark results are saved under the `default` project. Lets rerun our benchmarks, but this time specify a project name. For example:

```
bencher run --email info@bencher.dev --token 123JWT --adapter rust_cargo_bench --project my_first_project "cargo bench"
```

`--project my_first_project` sets the project name to `my_first_project`, so if you refresh your dashboard, you should now see a new project named `my_first_project` with your results. See [managing projects](managing_projects.md) for more details on on how to manage your projects.
