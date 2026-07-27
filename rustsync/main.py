"""
RustSync APK 入口 —— 启动 Rust 后端（8023）并托管 WebView 页面（8024）。
"""
import os
import sys
import time
import threading
import subprocess
import asyncio

_app_dir = os.path.dirname(os.path.abspath(__file__))
os.chdir(_app_dir)
os.makedirs('data', exist_ok=True)
os.makedirs('data/log', exist_ok=True)

# 检测 CPU ABI
def _detect_abi():
    try:
        from jnius import autoclass
        abi = autoclass('android.os.Build').CPU_ABI
        if abi:
            return abi
    except Exception:
        pass
    try:
        import platform
        if platform.machine() in ('aarch64', 'arm64'):
            return 'arm64-v8a'
    except Exception:
        pass
    return 'arm64-v8a'

_ABI = _detect_abi()
_RUST_BINARY = os.path.join(_app_dir, 'bin', _ABI, 'rustsync_server')
BUSINESS_PORT = 8023
WEBVIEW_PORT = 8024

HTML = """<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no">
<title>RustSync</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
html,body{height:100%;overflow:hidden}
iframe{width:100%;height:100%;border:0;background:#fff}
</style>
</head>
<body>
<iframe id="app" src="http://127.0.0.1:""" + str(BUSINESS_PORT) + """/"></iframe>
<script>
(function(){
  var n=0;
  function push(){n+=1;history.pushState({i:n},'','#stay'+n)}
  push();push();push();
  window.addEventListener('popstate',function(){push()})
  window.addEventListener('pageshow',function(e){if(e.persisted){push();push()}})
})()
</script>
</body>
</html>"""

from tornado.web import Application, RequestHandler

class MainHandler(RequestHandler):
    def get(self):
        self.set_header('Content-Type', 'text/html; charset=utf-8')
        self.write(HTML)

_rust_proc = None

def _start_rust():
    global _rust_proc
    if not os.path.exists(_RUST_BINARY):
        print(f'[ERROR] Rust binary not found: {_RUST_BINARY}', file=sys.stderr)
        return False
    os.chmod(_RUST_BINARY, 0o755)
    env = os.environ.copy()
    env['RUSTSYNC_PORT'] = str(BUSINESS_PORT)
    env['RUSTSYNC_PASSWORD'] = 'admin'
    env['TZ'] = 'Asia/Shanghai'
    try:
        _rust_proc = subprocess.Popen(
            [_RUST_BINARY], env=env,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            cwd=_app_dir,
        )
        print(f'[INFO] Rust backend started (PID={_rust_proc.pid}, ABI={_ABI})')
        return True
    except Exception as e:
        print(f'[ERROR] Failed to start Rust backend: {e}', file=sys.stderr)
        return False

def _monitor_rust():
    while True:
        if _rust_proc is None:
            time.sleep(2)
            _start_rust()
            continue
        ret = _rust_proc.poll()
        if ret is not None:
            print(f'[WARN] Rust backend exited (code={ret}), restarting...')
            time.sleep(3)
            _start_rust()
        else:
            time.sleep(1)

async def main():
    global _RUST_BINARY
    print(f'[INFO] RustSync APK starting (ABI={_ABI})')
    alt = os.path.join(_app_dir, 'rustsync_server')
    if not os.path.exists(_RUST_BINARY) and os.path.exists(alt):
        _RUST_BINARY = alt

    if not _start_rust():
        print('[FATAL] Cannot start Rust backend', file=sys.stderr)
        sys.exit(1)

    threading.Thread(target=_monitor_rust, daemon=True).start()

    app = Application([(r"/", MainHandler)])
    from tornado.httpserver import HTTPServer
    from tornado.netutil import bind_sockets
    sockets = bind_sockets(WEBVIEW_PORT, address='127.0.0.1')
    HTTPServer(app).add_sockets(sockets)
    print(f'[INFO] WebView page: http://127.0.0.1:{WEBVIEW_PORT}/')
    await asyncio.Event().wait()

if __name__ == "__main__":
    asyncio.run(main())