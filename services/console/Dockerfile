# Build Stage
# builder.Dockerfile
FROM bencher-ui-builder as builder

# https://hub.docker.com/_/node
FROM node:lts-alpine
COPY --from=builder /usr/src/services/ui/dist /usr/src/services/ui/dist
COPY --from=builder /usr/src/services/ui/package.json /usr/src/services/ui/package.json

WORKDIR /usr/src/services/ui
# https://github.com/withastro/astro/issues/7247#issuecomment-1576200139
RUN npm install -P

ENV HOST=0.0.0.0
ENV PORT=3000
EXPOSE 3000

CMD node ./dist/server/entry.mjs
