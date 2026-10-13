"""Serve the portable viewer; optional local-only render capture for visual QA."""
from http.server import ThreadingHTTPServer, SimpleHTTPRequestHandler
from pathlib import Path
import os

ROOT=Path(__file__).resolve().parent
os.chdir(ROOT)
class Handler(SimpleHTTPRequestHandler):
    def do_POST(self):
        name=self.path.removeprefix('/capture/')
        if self.path.startswith('/capture/') and name in {'perspective.png','top.png','bottom.png','front.png','side.png','front-quarter.png','bottom-quarter.png'}:
            length=int(self.headers.get('Content-Length',0))
            if not 0<length<20_000_000:
                self.send_error(413)
                return
            data=self.rfile.read(length)
            if not data.startswith(b'\x89PNG\r\n\x1a\n'):
                self.send_error(415)
                return
            (ROOT/'previews'/name).write_bytes(data)
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'OK')
        else:self.send_error(404)
ThreadingHTTPServer(('127.0.0.1',8768),Handler).serve_forever()
