#!/usr/bin/env python3
import argparse
import socket
import ssl
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass

    def do_HEAD(self):
        self.do_GET()

    def do_GET(self):
        self.connection.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        if self.path == "/drop":
            self.close_connection = True
            return
        if self.path == "/slow":
            time.sleep(0.8)
        if self.path == "/interim":
            self.wfile.write(b"HTTP/1.1 100 Continue\r\n\r\n")
        body = b"abc\x00def"
        status = 500 if self.path == "/error" else 304 if self.path == "/304" else 200
        if self.path == "/empty":
            body = b""
        if self.path.startswith("/bench"):
            parsed = urllib.parse.urlparse(self.path)
            query = urllib.parse.parse_qs(parsed.query)
            size = min(int(query.get("size", ["16384"])[0]), 4 * 1024 * 1024)
            unit = '{"key":"值0123456789abcdef","items":[1,2,3],'.encode()
            body = (unit * (size // len(unit) + 2))[:size]
            self.send_response(200)
            self.send_header("Content-Type", "application/json; charset=UTF-8")
            self.send_header("Connection", "close")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            try:
                self.wfile.write(body)
            except (BrokenPipeError, ConnectionResetError, ssl.SSLError):
                pass
            finally:
                self.close_connection = True
            return
        self.send_response(status)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Connection", "close")
        if self.path == "/chunked":
            self.send_header("Transfer-Encoding", "chunked")
        else:
            self.send_header("Content-Length", str(12 if self.path == "/drip" else len(body)))
        self.end_headers()
        try:
            if self.command == "HEAD" or status == 304:
                return
            if self.path == "/chunked":
                self.wfile.write(b"7\r\n" + body + b"\r\n0\r\n\r\n")
            elif self.path == "/drip":
                for _ in range(12):
                    self.wfile.write(b"x")
                    self.wfile.flush()
                    time.sleep(0.05)
            else:
                self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError, ssl.SSLError):
            pass
        finally:
            self.close_connection = True


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--cert", required=True)
    parser.add_argument("--key", required=True)
    parser.add_argument("--port", type=int, default=18474)
    args = parser.parse_args()
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.maximum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(args.cert, args.key)
    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    print("TLS fixture listening on 127.0.0.1:" + str(args.port), flush=True)
    server.serve_forever()
