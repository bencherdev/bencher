# TODO move to multistage build
# https://hub.docker.com/r/swaggerapi/swagger-ui
FROM swaggerapi/swagger-ui:v4.11.1

COPY swagger.json /schema/swagger.json
ENV SWAGGER_JSON /schema/swagger.json
ENV BASE_URL /v0