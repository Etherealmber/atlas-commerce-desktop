"""Fictional local HTTP health endpoint for disposable Windows installer tests."""
import http.server
import pathlib
import sys


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != '/api/health':
            self.send_error(404)
            return
        body = b'{"success":true,"data":{"ok":true,"d1":true,"r2":false}}'
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


with http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler) as server:
    pathlib.Path(sys.argv[1]).write_text(str(server.server_address[1]), encoding='ascii')
    server.serve_forever()
