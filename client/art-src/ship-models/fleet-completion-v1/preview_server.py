"""Loopback reference/model gallery with bounded per-model capture routes."""
from http.server import ThreadingHTTPServer,SimpleHTTPRequestHandler
from pathlib import Path
from scaffold import SPECS,MODELS

VIEWS={'perspective.png','top.png','bottom.png','front.png','back.png','side.png','front-quarter.png','bottom-quarter.png'}
class Handler(SimpleHTTPRequestHandler):
    def __init__(self,*args,**kwargs):super().__init__(*args,directory=str(MODELS),**kwargs)
    def do_POST(self):
        fields=self.path.split('/')
        if len(fields)!=4 or fields[1]!='capture' or fields[2] not in SPECS or fields[3] not in VIEWS:
            self.send_error(404);return
        try:n=int(self.headers.get('Content-Length','0'))
        except ValueError:self.send_error(400);return
        if not 0<n<20_000_000:self.send_error(413);return
        data=self.rfile.read(n)
        if not data.startswith(b'\x89PNG\r\n\x1a\n'):self.send_error(415);return
        folder=MODELS/(fields[2]+'-textured-v1')/'previews';folder.mkdir(exist_ok=True)
        (folder/fields[3]).write_bytes(data)
        self.send_response(200);self.end_headers();self.wfile.write(b'OK')

print('Fleet reference and model previews: http://127.0.0.1:8778/',flush=True)
ThreadingHTTPServer(('127.0.0.1',8778),Handler).serve_forever()
