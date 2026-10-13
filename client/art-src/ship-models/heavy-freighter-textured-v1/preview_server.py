"""Serve the offline viewer, with a loopback-only PNG capture endpoint."""
from http.server import ThreadingHTTPServer,SimpleHTTPRequestHandler
from pathlib import Path
import argparse
import os

ROOT=Path(__file__).resolve().parent
parser=argparse.ArgumentParser()
parser.add_argument('--port',type=int,default=8774)
args=parser.parse_args()
os.chdir(ROOT)
(ROOT/'previews').mkdir(exist_ok=True)
class Handler(SimpleHTTPRequestHandler):
    def do_POST(self):
        name=self.path.removeprefix('/capture/')
        if not self.path.startswith('/capture/') or name not in {
            'perspective.png','top.png','bottom.png','front.png','back.png',
            'side.png','front-quarter.png','bottom-quarter.png'
        }:
            self.send_error(404);return
        try:length=int(self.headers.get('Content-Length',0))
        except ValueError:self.send_error(400);return
        if not 0<length<20_000_000:
            self.send_error(413);return
        data=self.rfile.read(length)
        if not data.startswith(b'\x89PNG\r\n\x1a\n'):
            self.send_error(415);return
        (ROOT/'previews'/name).write_bytes(data)
        self.send_response(200);self.end_headers();self.wfile.write(b'OK')
print(f'Heavy Freighter preview: http://127.0.0.1:{args.port}/viewer.html',flush=True)
ThreadingHTTPServer(('127.0.0.1',args.port),Handler).serve_forever()
