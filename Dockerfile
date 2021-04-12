FROM ubuntu:hirsute
RUN apt update
RUN apt install -y youtube-dl libopusfile-dev libopus0 ffmpeg
WORKDIR /rust-bot
COPY target/release/bot .
COPY config.yaml .
CMD ["bot"]