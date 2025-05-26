# Stream proxy

Simple reverse proxy for AceStream and HLS streams. The idea is to serve a single stream as a website.

Requires a running AceStream instance, do this

## With Docker

Edit `docker-compose.yml` in the rproxy service, mount the `/app/private` directory to your directory with the certificates.

Example:

```yaml
- /root/.acme.sh/example.com_ecc/:/app/private # Ensure that these arent symlinks
```

Edit `Rocket.toml`

- Set `ace_base_url = "http://ace:6878"`
- Set the tls paths relative to the mountpoint in `docker-compose.yml`

Then run:

```bash
docker compose up -d
```

## Run Local

Run the script to grab the latest version of hls.js

`./scripts/grab-js.sh`

edit `Rocket.toml`

### Run Dev

```bash
cargo run # Port 8999
```

### Run Prod

Install acme.sh, grab a certificate. This example using acme.sh requires port 80 to be open.

```bash
curl https://get.acme.sh | sh -s email=email@example.com
source ~/.acme.sh/acme.sh.env
acme.sh --set-default-ca --server letsencrypt
acme.sh --issue --standalone -d stream.example.com --fullchain-file private/fullchain.cer --key-file private/cert.key
```

```bash
cargo run --release # Port 443, requires certificates
```

### Running AceStream

Run an instance of AceStream, I prefer to do this with docker.

```bash
docker run -t -p 6878:6878 ghcr.io/martinbjeldbak/acestream-http-proxy
```

You can also run install it with the official instructions, though its a bit limited due to using an old version of Python, and Ubuntu.

## Limitations

- So far its only designed to reverse proxy one stream
- The web player will either not work or have no audio if the stream audio is AC3 encoded
  - In theory you can use [transcode_ac3](https://wiki.acestream.media/Engine_HTTP_API) to ensure that AC3 gets transcoded, but I havent had it work in testing.
- No chromecast or airplay support yet
