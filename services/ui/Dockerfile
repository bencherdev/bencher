# https://hub.docker.com/_/node/
FROM node:lts-bullseye-slim as builder

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

RUN npm run build

# https://hub.docker.com/_/nginx
FROM nginx:mainline-alpine
COPY --from=builder /usr/src/ui/dist /usr/share/nginx/html

RUN rm -rf /etc/nginx/conf.d
COPY conf /etc/nginx

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]