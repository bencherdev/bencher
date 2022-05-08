package main

import (
	"dagger.io/dagger"
	"bencher.dev/hello"
	"bencher.dev/alpine"
	"universe.dagger.io/bash"
	"universe.dagger.io/docker"
)

dagger.#Plan & {
	client: {
		filesystem: "./bencher": {
			read: {
				contents: dagger.#FS
				exclude: [
					"README.md",
				]
			}
			write: contents: actions.bencher_cli.result
		}
	}

	actions: {
		bencher_cli: hello.#AddHello & {
			dir: client.filesystem."./bencher".read.contents
		}

		deps: docker.#Build & {
			steps: [
				alpine.#Build & {
					packages: {
						bash: {}
						yarn: {}
						git: {}
					}
				},
				docker.#Copy & {
					contents: client.filesystem."./bencher".read.contents
					dest:     "/src"
				},
			]
		}

		test: bash.#Run & {
			input:   deps.output
			workdir: "/src"
			script: contents: #"""
				exit 0
				"""#
		}

	}
}
