package main

import (
	"dagger.io/dagger"
	"bencher.dev/hello"
	"bencher.dev/dagger/rust"
	"bencher.dev/envoy"
	"universe.dagger.io/bash"
	"universe.dagger.io/docker"
)

dagger.#Plan & {
	client: {
		filesystem: {
			"./bencher": {
				read: {
					contents: dagger.#FS
					exclude: [
						"README.md",
					]
				}
				write: contents: actions.bencher_cli.result
			}
			"./reports": {
				read: {
					contents: dagger.#FS
				}
			}
			"./envoy": {
				read: {
					contents: dagger.#FS
					exclude: [
						"envoy.cue",
					]
				}
			}
		}
	}

	actions: {
		bencher_cli: hello.#AddHello & {
			dir: client.filesystem."./bencher".read.contents
		}

		docker_bencher_cli: docker.#Build & {
			steps: [
				rust.#Build & {
					packages: {
						bash: {}
						git: {}
					}
				},
				docker.#Copy & {
					contents: client.filesystem."./bencher".read.contents
					dest:     "/src/bencher"
				},
				docker.#Copy & {
					contents: client.filesystem."./reports".read.contents
					dest:     "/src/reports"
				},
			]
		}

		docker_envoy: docker.#Build & {
			steps: [
				envoy.#Build & {},
				docker.#Copy & {
					contents: client.filesystem."./envoy".read.contents
					dest:     "/"
				},
			]
		}

		test: bash.#Run & {
			input:   docker_bencher_cli.output
			workdir: "/src/bencher"
			script: contents: #"""
				cargo test
				"""#
		}

	}
}
