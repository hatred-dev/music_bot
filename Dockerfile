FROM alpine:edge
RUN apk update
RUN apk add bash
RUN apk add youtube-dl
RUN apk add opus
RUN apk add ffmpeg
WORKDIR /rust-bot
COPY target/release/bot .
COPY config.yaml .
COPY . ./
CMD ["exec","bot"]