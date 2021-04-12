FROM alpine:edge
RUN apk update
RUN apk add youtube-dl
RUN apk add opus
RUN apk add ffmpeg
WORKDIR /rust-bot
COPY target/release/bot /rust-bot
COPY config.yaml /rust-bot
RUN ls -al
RUN ./rust-bot/bot