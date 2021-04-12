FROM alpine:edge
RUN apk update
#RUN apk add --no-cache youtude-dl
RUN apk add --no-cache opus
RUN apk add --no-cache ffmpeg
COPY target/release/bot /
CMD ["bash", "-c","echo $HOME"]