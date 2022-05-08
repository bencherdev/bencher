package main

import (
	"dagger.io/dagger"
	"bencher.dev/hello"
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
	}
}
