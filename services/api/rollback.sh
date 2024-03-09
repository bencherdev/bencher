#!/bin/bash

fly deploy --config fly/fly.toml --image registry.fly.io/bencher-api@$1
