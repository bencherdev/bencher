# https://hub.docker.com/_/node/
FROM node:18.1.0-bullseye

# Set working directory
WORKDIR /usr/src/ui

# Add `/usr/src/ui/node_modules/.bin` to $PATH
ENV PATH /usr/src/ui/node_modules/.bin:$PATH

# Install and cache ui dependencies
COPY package.json /usr/src/ui/package.json
RUN npm config set unsafe-perm true
RUN npm install

# Start dev vite server
CMD ["npm", "run", "dev"]