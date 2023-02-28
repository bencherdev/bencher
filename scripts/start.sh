#!/bin/bash

./scripts/githooks.sh

sudo apt-get update -q
sudo apt-get install -yq netcat-openbsd sqlite3

sudo apt-get install -y gconf-service libasound2 libatk1.0-0 libcairo2 libcups2 libfontconfig1 libgdk-pixbuf2.0-0 libgtk-3-0 libnspr4 libpango-1.0-0 libxss1 fonts-liberation libappindicator1 libnss3 lsb-release xdg-utils

curl -L https://fly.io/install.sh | sh
echo "export FLYCTL_INSTALL=\"/home/gitpod/.fly\"" >> $HOME/.bash_profile
echo "export PATH=\"/home/gitpod/.fly/bin:$PATH\"" >> $HOME/.bash_profile

# cd ~/.local/share/vscode-sqltools
# npm install sqlite3@5.1.1
# cd -

source ~/.bash_profile
