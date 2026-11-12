import http from "node:http";
import net from "node:net";
import { OVERRIDES } from "./fixtures.mjs";

const [listenPort, daemonPort] = process.argv.slice(2).map(Number);
if (!listenPort || !daemonPort) {
  process.stderr.write("usage: proxy.mjs LISTEN_PORT DAEMON_PORT\n");
  process.exit(2);
}

const HOST = "127.0.0.1";

const server = http.createServer((req, res) => {
  const path = new URL(req.url, "http://x").pathname;
  const fixed = req.method === "GET" ? OVERRIDES.get(path) : undefined;
  if (fixed) {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(fixed()));
    return;
  }
  const upstream = http.request(
    { host: HOST, port: daemonPort, path: req.url, method: req.method, headers: req.headers },
    (answer) => {
      res.writeHead(answer.statusCode ?? 502, answer.headers);
      answer.pipe(res);
    },
  );
  upstream.on("error", (error) => {
    res.writeHead(502, { "content-type": "text/plain" });
    res.end(`proxy: ${error.message}`);
  });
  req.pipe(upstream);
});

server.on("upgrade", (req, socket, head) => {
  const upstream = net.connect(daemonPort, HOST, () => {
    const lines = [`${req.method} ${req.url} HTTP/1.1`];
    for (let i = 0; i < req.rawHeaders.length; i += 2) lines.push(`${req.rawHeaders[i]}: ${req.rawHeaders[i + 1]}`);
    upstream.write(`${lines.join("\r\n")}\r\n\r\n`);
    upstream.write(head);
    socket.pipe(upstream);
    upstream.pipe(socket);
  });
  upstream.on("error", () => socket.destroy());
  socket.on("error", () => upstream.destroy());
});

server.listen(listenPort, HOST, () => process.stdout.write(`proxy: ${listenPort} -> ${daemonPort}\n`));
