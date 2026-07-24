FROM dr34m/rust-sync:not-for-use-pip-req as builder
WORKDIR /app
COPY . /app
RUN pip install --no-cache-dir -r requirements.txt \
    && pyinstaller RustSync.spec

FROM dr34m/rust-sync:not-for-use-alpine
VOLUME /app/data
WORKDIR /app
COPY --from=builder /app/dist/RustSync /app/
ENV RUSTSYNC_PORT=8023 RUSTSYNC_EXPIRES=2 RUSTSYNC_LOG_LEVEL=1 RUSTSYNC_CONSOLE_LEVEL=2 RUSTSYNC_LOG_SAVE=7 RUSTSYNC_TASK_SAVE=0 RUSTSYNC_TASK_TIMEOUT=72 TZ=Asia/Shanghai
EXPOSE 8023
CMD ["./RustSync"]
