"""
RustSync 前台服务入口（运行在 :pythonservice 进程）。

启动 Rust 后端（8023）并运行日记页（Tornado 8024）。
Rust 二进制处理所有业务逻辑，Python 仅负责日志收集、进程守护和日记页。
"""
import os
import sys
import time
import logging
import threading
import subprocess
import asyncio
import warnings

warnings.filterwarnings('ignore', message='.*character detection dependency.*')

# ======================================================================
# 路径设置
# ======================================================================
_app_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if _app_dir not in sys.path:
    sys.path.insert(0, _app_dir)
os.chdir(_app_dir)
os.makedirs('data', exist_ok=True)
os.makedirs('data/log', exist_ok=True)

# ======================================================================
# 检测 CPU ABI，选择正确的 Rust 二进制
# ======================================================================
def _detect_abi():
    try:
        from jnius import autoclass
        Build = autoclass('android.os.Build')
        abi = Build.CPU_ABI
        if abi:
            return abi
    except Exception:
        pass
    try:
        import platform
        machine = platform.machine()
        if machine in ('aarch64', 'arm64'):
            return 'arm64-v8a'
    except Exception:
        pass
    return 'arm64-v8a'

_ABI = _detect_abi()
_RUST_BINARY = os.path.join(_app_dir, 'bin', _ABI, 'rustsync_server')

# ======================================================================
# 内存日志缓冲
# ======================================================================
LOG_MAX = 500
_log_lock = threading.Lock()
_log_entries = []
_log_seq = 0


def _append_log(level, msg):
    global _log_seq
    msg = msg.rstrip('\n')
    if not msg:
        return
    with _log_lock:
        _log_seq += 1
        _log_entries.append({
            'seq': _log_seq,
            'ts': time.time(),
            'level': level,
            'msg': msg,
        })
        if len(_log_entries) > LOG_MAX:
            del _log_entries[:len(_log_entries) - LOG_MAX // 2]
    if _file_fp:
        try:
            _file_fp.write(f'[{level}] {msg}\n')
            _file_fp.flush()
        except Exception:
            pass


def _file_log(level, msg):
    msg = msg.rstrip('\n')
    if not msg:
        return
    if _file_fp:
        try:
            _file_fp.write(f'[{level}] {msg}\n')
            _file_fp.flush()
        except Exception:
            pass


class MemoryLogHandler(logging.Handler):
    def emit(self, record):
        _append_log(record.levelname, record.getMessage())


class _StdoutCapture:
    def __init__(self, level):
        self._level = level
        self._buf = ''

    def write(self, text):
        self._buf += text
        while '\n' in self._buf:
            line, self._buf = self._buf.split('\n', 1)
            if line.strip():
                _append_log(self._level, line)

    def flush(self):
        if self._buf.strip():
            _append_log(self._level, self._buf)
        self._buf = ''


# ======================================================================
# 文件日志后备
# ======================================================================
_file_fp = None
_log_paths = []
try:
    from jnius import autoclass
    _ctx = (getattr(autoclass('org.kivy.android.PythonActivity'), 'mActivity', None)
            or getattr(autoclass('org.kivy.android.PythonService'), 'mService', None))
    _d = _ctx.getExternalFilesDir(None) if _ctx is not None else None
    if _d is not None:
        _log_paths.append(os.path.join(str(_d.getAbsolutePath()), 'rustsync_debug.log'))
except Exception:
    pass
_log_paths.append('/storage/emulated/0/Android/data/com.github.rustsync/files/rustsync_debug.log')
_log_paths.append(os.path.join(os.getcwd(), 'debug.log'))
for _log_path in _log_paths:
    try:
        _file_fp = open(_log_path, 'a', buffering=1)
        break
    except Exception:
        pass


class _FileLogHandler(logging.Handler):
    def emit(self, record):
        if _file_fp:
            try:
                _file_fp.write(f'[{record.levelname}] {record.getMessage()}\n')
            except Exception:
                pass


# ======================================================================
# 安装日志收集
# ======================================================================
_file_log('INFO', '=== RustSync 服务进程启动 ===')
_file_log('INFO', f'Python: {sys.version}')
_file_log('INFO', f'app_dir: {_app_dir}')
_file_log('INFO', f'cwd: {os.getcwd()}')
_file_log('INFO', f'ABI: {_ABI}')
_file_log('INFO', f'Rust binary: {_RUST_BINARY}')

_logger = logging.getLogger()
_logger.addHandler(MemoryLogHandler())
_logger.addHandler(_FileLogHandler())

logging.getLogger('tornado.access').setLevel(logging.WARNING)

sys.stdout = _StdoutCapture('INFO')
sys.stderr = _StdoutCapture('ERROR')


def _safe_exit(code=0):
    _append_log('CRITICAL', f'服务进程退出 (code={code})')
    if _file_fp:
        try:
            _file_fp.flush()
        except Exception:
            pass
    os._exit(code)


sys.exit = _safe_exit


def _excepthook(exc_type, exc_value, exc_tb):
    import traceback
    tb = ''.join(traceback.format_exception(exc_type, exc_value, exc_tb))
    _append_log('ERROR', f'未捕获异常:\n{tb}')
    if _file_fp:
        try:
            _file_fp.write(tb)
            _file_fp.flush()
        except Exception:
            pass


sys.excepthook = _excepthook


# ======================================================================
# 日记页 Tornado 应用（8024，仅 127.0.0.1）
# ======================================================================
from tornado.web import Application, RequestHandler

_LOG_PAGE_HTML = """<!DOCTYPE html>
<html lang="zh">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no">
<title>RustSync Android</title>
<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
html, body { height: 100%; overflow: hidden; }
body {
  background: #1e1e1e;
  color: #d4d4d4;
  font-family: 'Courier New', Consolas, monospace;
  font-size: 12px;
  display: flex;
  flex-direction: column;
}
button { letter-spacing: 0; }
#tabs {
  flex: 0 0 44px;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  background: #181818;
  border-bottom: 1px solid #3c3c3c;
}
.tab {
  min-width: 0;
  appearance: none;
  background: transparent;
  color: #a8a8a8;
  border: 0;
  border-bottom: 2px solid transparent;
  font-family: system-ui, sans-serif;
  font-size: 14px;
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
}
.tab.active {
  background: #252526;
  color: #ffffff;
  border-bottom-color: #4ec9b0;
  font-weight: 600;
}
.tab:active { background: #303030; }
.tab:focus { outline: none; }
.view {
  flex: 1 1 auto;
  min-height: 0;
  display: none;
}
.view.active {
  display: flex;
  flex-direction: column;
}
#bar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: #252526;
  border-bottom: 1px solid #3c3c3c;
}
#bar .title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: bold;
  font-size: 13px;
  flex: 1;
}
#bar button {
  flex-shrink: 0;
  background: #3a3a3a;
  color: #d4d4d4;
  border: 1px solid #3c3c3c;
  border-radius: 3px;
  padding: 5px 12px;
  font-size: 11px;
  cursor: pointer;
}
#bar button:active { background: #505050; }
#status {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #4ec9b0;
  flex-shrink: 0;
}
#status.offline { background: #f44747; }
#log {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 6px 10px;
  -webkit-overflow-scrolling: touch;
}
.line {
  white-space: pre-wrap;
  word-break: break-all;
  padding: 1px 0;
  line-height: 1.6;
}
.line .ts { color: #858585; }
.line .lv { font-weight: bold; margin: 0 4px; }
.lv-DEBUG .lv { color: #569cd6; }
.lv-INFO .lv { color: #4ec9b0; }
.lv-WARNING .lv { color: #dcdcaa; }
.lv-ERROR { color: #f44747; }
.lv-CRITICAL { color: #569cd6; }
.lv-ERROR .lv { color: #f44747; }
.lv-CRITICAL .lv { color: #569cd6; }
#viewWeb { background: #ffffff; }
#businessFrame {
  flex: 1;
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
  border: 0;
  background: #ffffff;
}
</style>
</head>
<body>
<nav id="tabs" role="tablist" aria-label="RustSync 页面">
  <button type="button" id="tabLogs" class="tab active" aria-selected="true" aria-controls="viewLogs" role="tab">日志</button>
  <button type="button" id="tabWeb" class="tab" aria-selected="false" aria-controls="viewWeb" role="tab">网页</button>
</nav>
<section id="viewLogs" class="view active" aria-labelledby="tabLogs" role="tabpanel">
  <div id="bar">
    <span id="status"></span>
    <span class="title">RustSync 运行日志</span>
    <button type="button" id="btnClear">清空</button>
  </div>
  <div id="log"></div>
</section>
<section id="viewWeb" class="view" aria-labelledby="tabWeb" role="tabpanel" hidden>
  <iframe id="businessFrame" title="RustSync 网页" data-src="http://127.0.0.1:8023/"></iframe>
</section>
<script>
var lastSeq = 0;
var logEl = document.getElementById('log');
var statusEl = document.getElementById('status');
var tabLogs = document.getElementById('tabLogs');
var tabWeb = document.getElementById('tabWeb');
var viewLogs = document.getElementById('viewLogs');
var viewWeb = document.getElementById('viewWeb');
var businessFrame = document.getElementById('businessFrame');

function selectView(name) {
  var showWeb = name === 'web';
  tabLogs.classList.toggle('active', !showWeb);
  tabWeb.classList.toggle('active', showWeb);
  tabLogs.setAttribute('aria-selected', String(!showWeb));
  tabWeb.setAttribute('aria-selected', String(showWeb));
  viewLogs.classList.toggle('active', !showWeb);
  viewWeb.classList.toggle('active', showWeb);
  viewLogs.hidden = showWeb;
  viewWeb.hidden = !showWeb;
  if (name === 'web' && !businessFrame.getAttribute('src')) {
    businessFrame.setAttribute('src', businessFrame.dataset.src);
  }
}

tabLogs.onclick = function() { selectView('logs'); };
tabWeb.onclick = function() { selectView('web'); };

document.getElementById('btnClear').onclick = function() {
  logEl.innerHTML = '';
};

function nearBottom() {
  return logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight < 80;
}

function escapeHtml(s) {
  return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
}

function poll() {
  fetch('/__log__?since=' + lastSeq)
    .then(function(r) { return r.json(); })
    .then(function(data) {
      statusEl.classList.remove('offline');
      if (data.last_seq) lastSeq = data.last_seq;
      if (!data.entries || data.entries.length === 0) return;
      var scroll = nearBottom();
      data.entries.forEach(function(e) {
        var d = new Date(e.ts * 1000);
        var ts = d.getHours() + ':' +
                 String(d.getMinutes()).padStart(2,'0') + ':' +
                 String(d.getSeconds()).padStart(2,'0');
        var line = document.createElement('div');
        line.className = 'line lv-' + e.level;
        line.innerHTML = '<span class="ts">[' + ts + ']</span>' +
                         '<span class="lv">' + e.level + '</span>' +
                         escapeHtml(e.msg);
        logEl.appendChild(line);
      });
      while (logEl.children.length > 2000) {
        logEl.removeChild(logEl.firstChild);
      }
      if (scroll) logEl.scrollTop = logEl.scrollHeight;
    })
    .catch(function() { statusEl.classList.add('offline'); })
    .finally(function() { setTimeout(poll, 1500); });
}

(function() {
  var n = 0;
  function push() {
    n += 1;
    history.pushState({ i: n }, '', '#stay' + n);
  }
  push(); push(); push();
  window.addEventListener('popstate', function() { push(); });
  window.addEventListener('pageshow', function(e) { if (e.persisted) { push(); push(); } });
})();

poll();
</script>
</body>
</html>"""


class LogIndexHandler(RequestHandler):
    def get(self):
        self.set_header('Content-Type', 'text/html; charset=utf-8')
        self.write(_LOG_PAGE_HTML)


class LogDataHandler(RequestHandler):
    def get(self):
        since = int(self.get_argument('since', 0))
        with _log_lock:
            entries = [e for e in _log_entries if e['seq'] > since]
            last_seq = _log_seq
        self.set_header('Content-Type', 'application/json')
        self.write({'entries': entries, 'last_seq': last_seq})


def make_log_app():
    return Application([
        (r"/__log__", LogDataHandler),
        (r"/", LogIndexHandler),
    ])


# ======================================================================
# Rust 二进制进程管理
# ======================================================================
_rust_process = None
LOG_PORT = 8024
BUSINESS_PORT = 8023


def _start_rust():
    """启动 Rust 后端进程"""
    global _rust_process
    if not os.path.exists(_RUST_BINARY):
        _file_log('ERROR', f'Rust 二进制不存在: {_RUST_BINARY}')
        _append_log('ERROR', f'Rust 二进制不存在: {_RUST_BINARY}')
        return False

    # 确保可执行
    os.chmod(_RUST_BINARY, 0o755)

    env = os.environ.copy()
    env['RUSTSYNC_PORT'] = str(BUSINESS_PORT)
    env['RUSTSYNC_DATA_DIR'] = os.path.join(_app_dir, 'data')
    env['RUSTSYNC_LOG_DIR'] = os.path.join(_app_dir, 'data', 'log')
    env['RUSTSYNC_DB_PATH'] = os.path.join(_app_dir, 'data', 'rustsync.db')

    try:
        _rust_process = subprocess.Popen(
            [_RUST_BINARY],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            cwd=_app_dir,
        )
        _file_log('INFO', f'Rust 后端已启动 (PID={_rust_process.pid})')
        _append_log('INFO', f'Rust 后端已启动 (PID={_rust_process.pid}, ABI={_ABI})')
        return True
    except Exception as e:
        _file_log('ERROR', f'启动 Rust 后端失败: {e}')
        _append_log('ERROR', f'启动 Rust 后端失败: {e}')
        return False


def _read_rust_output():
    """读取 Rust 进程输出并记录到日志"""
    if _rust_process is None:
        return
    try:
        while True:
            line = _rust_process.stdout.readline()
            if not line:
                break
            try:
                text = line.decode('utf-8', errors='replace').rstrip('\n\r')
            except Exception:
                text = str(line)
            if text:
                _append_log('INFO', text)
    except Exception:
        pass


def _monitor_rust():
    """监控 Rust 进程，崩溃时自动重启"""
    while True:
        if _rust_process is None:
            time.sleep(2)
            if not _start_rust():
                time.sleep(5)
            continue
        ret = _rust_process.poll()
        if ret is not None:
            _file_log('WARNING', f'Rust 后端已退出 (code={ret})，3秒后重启...')
            _append_log('WARNING', f'Rust 后端已退出 (code={ret})，3秒后重启...')
            _rust_process = None
            time.sleep(3)
        else:
            time.sleep(1)


async def main():
    _file_log('INFO', '服务进程正在初始化...')

    # 确保 Rust 二进制目录存在
    if not os.path.exists(_RUST_BINARY):
        _file_log('ERROR', f'Rust 二进制不存在: {_RUST_BINARY}')
        _append_log('ERROR', f'Rust 二进制不存在: {_RUST_BINARY}')
        # 尝试备用路径
        alt_binary = os.path.join(_app_dir, 'rustsync_server')
        if os.path.exists(alt_binary):
            global _RUST_BINARY
            _RUST_BINARY = alt_binary
            _file_log('INFO', f'使用备用路径: {_RUST_BINARY}')

    # 启动 Rust 后端
    if not _start_rust():
        _file_log('CRITICAL', '无法启动 Rust 后端')
        _safe_exit(1)
        return

    # 启动 Rust 进程守护线程
    threading.Thread(target=_monitor_rust, daemon=True).start()

    # 启动 Rust 输出读取线程
    threading.Thread(target=_read_rust_output, daemon=True).start()

    # 日记页 Tornado（8024，仅 127.0.0.1）
    from tornado.httpserver import HTTPServer
    from tornado.netutil import bind_sockets

    log_app = make_log_app()
    sockets = bind_sockets(LOG_PORT, address='127.0.0.1')
    server = HTTPServer(log_app)
    server.add_sockets(sockets)
    _file_log('INFO', f'日记页已启动: http://127.0.0.1:{LOG_PORT}/')
    _append_log('INFO', f'日记页已启动: http://127.0.0.1:{LOG_PORT}/')

    _append_log('CRITICAL',
                f'RustSync 启动成功 (Rust 模式, ABI={_ABI}) '
                f'| 业务: http://127.0.0.1:{BUSINESS_PORT}/ '
                f'| 日记: http://127.0.0.1:{LOG_PORT}/')

    await asyncio.Event().wait()


try:
    asyncio.run(main())
except Exception as e:
    import traceback
    tb = traceback.format_exc()
    _append_log('ERROR', f'服务进程启动失败:\n{tb}')
    if _file_fp:
        try:
            _file_fp.write(tb)
            _file_fp.flush()
        except Exception:
            pass
    os._exit(1)