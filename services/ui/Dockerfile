# https://hub.docker.com/_/node/
FROM node:lts-bullseye-slim

RUN apt-get update \
    && apt-get install -y python2 python3 make g++

# Set working directory
WORKDIR /usr/src/ui
COPY package-lock.json package-lock.json
COPY package.json package.json

RUN npm install

COPY public public
COPY src src
COPY index.html index.html
COPY tsconfig.json tsconfig.json
COPY vite.config.ts vite.config.ts

CMD ["npm", "run", "host"]