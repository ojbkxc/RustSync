FROM dr34m/rust-sync:not-for-use-pyinstaller
COPY requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt
