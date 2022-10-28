#!/bin/bash

until [ -d ~/.local/share/vscode-sqltools ]
do
    echo "Waiting for vscode-sqltools"
    sleep 5
done

cd ~/.local/share/vscode-sqltools
npm install sqlite3
