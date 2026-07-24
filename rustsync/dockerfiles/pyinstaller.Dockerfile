FROM dr34m/rust-sync:not-for-use-python
RUN apk update \
	&& apk add --no-cache binutils gcc zlib-dev libc-dev \
	&& pip install --upgrade pip \
	&& pip install --no-cache-dir pyinstaller