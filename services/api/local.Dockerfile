# Build Stage
# builder.Dockerfile
FROM bencher-api-builder as builder

# RUN ls -l /usr/lib && exit 1
# RUN find / -name libfontconfig.so.1 && exit 1
# TODO move these all to a folder like /usr/lib/bencher_libs
# COPY --from=builder /usr/lib/bencher_libs /usr/lib

# Bundle Stage
# https://github.com/GoogleContainerTools/distroless/blob/main/cc/README.md
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /usr/include/fontconfig /usr/include/fontconfig
ARG ARCH=aarch64-linux-gnu
COPY --from=builder /usr/lib/${ARCH}/libfontconfig.so.1 /usr/lib/libfontconfig.so.1
COPY --from=builder /usr/lib/${ARCH}/libfreetype.so.6 /usr/lib/libfreetype.so.6
COPY --from=builder /usr/lib/${ARCH}/libexpat.so.1 /usr/lib/libexpat.so.1
COPY --from=builder /usr/lib/${ARCH}/libz.so.1 /usr/lib/libz.so.1
COPY --from=builder /usr/lib/${ARCH}/libpng16.so.16 /usr/lib/libpng16.so.16
COPY --from=builder /usr/lib/${ARCH}/libbrotlidec.so.1 /usr/lib/libbrotlidec.so.1
COPY --from=builder /usr/lib/${ARCH}/libbrotlicommon.so.1 /usr/lib/libbrotlicommon.so.1
COPY --from=builder /usr/share/fonts /usr/share/fonts
COPY --from=builder /etc/fonts /etc/fonts

ARG TARGET
COPY --from=builder /usr/src/target/${TARGET}/debug/api /api
# COPY --from=builder /usr/src/target/release/api /api
COPY --from=builder /data /data

ENV PORT 61016

CMD ["/api"]
