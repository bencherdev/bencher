package main

import (
	"dagger.io/dagger"
	"bencher.dev/hello"
	"bencher.dev/rust"
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
		}
	}

	actions: {
		bencher_cli: hello.#AddHello & {
			dir: client.filesystem."./bencher".read.contents
		}

		deps: docker.#Build & {
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

		test: bash.#Run & {
			input:   deps.output
			workdir: "/src/bencher"
			script: contents: #"""
				cargo test
				"""#
		}

	}
}
