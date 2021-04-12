FROM alpine:edge
RUN apk update
RUN apk add bash
RUN apk add youtube-dl
RUN apk add opus
RUN apk add ffmpeg
COPY target/release/bot /rust-bot
COPY config.yaml /rust-bot
ENTRYPOINT ["rust-bot"]
CMD ["bot"]