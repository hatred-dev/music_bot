FROM alpine:edge
RUN apk update
RUN apk add --no-cache youtube-dl-2021.04.07-r0
RUN apk add --no-cache opus
RUN apk add --no-cache ffmpeg
COPY target/release/bot /
CMD ["bash", "-c","echo $HOME"]