# https://hub.docker.com/_/node/
FROM node:18.1.0-bullseye

# Set working directory
WORKDIR /usr/src/ui
COPY package-lock.json package-lock.json
COPY package.json package.json

RUN npm install

COPY public public
COPY src src
COPY .env.development .env.development
COPY index.html index.html
COPY tsconfig.json tsconfig.json
COPY vite.config.ts vite.config.ts

CMD ["npm", "run", "host"]