package main

import (
	"dagger.io/dagger"
	"dagger.io/dagger/core"
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
		bencher_cli: #AddHello & {
			dir: client.filesystem."./bencher".read.contents
		}
	}
}

// Write a greeting to a file, and add it to a directory
#AddHello: {
	// The input directory
	dir: dagger.#FS

	// The name of the person to greet
	name: string | *"world"

	write: core.#WriteFile & {
		input:    dir
		path:     "hello-\(name).txt"
		contents: "hello, \(name)!"
	}

	// The directory with greeting message added
	result: write.output
}
