FROM alpine:latest

RUN apk add --update --no-cache socat

# 将与写入 Flag 相关的操作放在最后面
ARG ATTACKR_FLAG
RUN echo $ATTACKR_FLAG > /flag

EXPOSE 1337

ENTRYPOINT ["socat", "tcp-l:1337,reuseaddr,fork", "exec:/bin/sh,pty,ctty,setsid,stderr,echo=0"]
